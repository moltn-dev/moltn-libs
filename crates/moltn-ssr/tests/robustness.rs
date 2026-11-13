//! Robustesse du moteur SSR (voir docs/moltn-ssr.md, « Tests »).

use std::time::{Duration, Instant};

use moltn_ssr::{Bundle, EmbeddedSsr, Engine, SsrError};
use serde_json::json;

const SCENARIOS: &str = include_str!("fixtures/scenarios.js");
const TIMEOUT: Duration = Duration::from_millis(200);

/// Les mêmes tests pour chaque moteur compilé : le contrat de `JsEngine` ne dépend pas du moteur.
macro_rules! engine_tests {
    ($name:ident, $engine:expr, intl = $intl:expr) => {
        mod $name {
            use super::*;

            const ENGINE: Engine = $engine;
            const HAS_INTL: bool = $intl;

        /// Un seul worker : prouve que c'est bien le même pool qui se remet d'une erreur.
        fn ssr() -> EmbeddedSsr {
            EmbeddedSsr::builder(Bundle::from_static(SCENARIOS)).engine(ENGINE)
                .workers(1)
                .render_timeout(TIMEOUT)
                .memory_limit_mb(32)
                .build()
                .expect("le bundle de test se charge")
        }

        fn page(component: &str) -> serde_json::Value {
            json!({ "component": component, "props": { "n": 7 }, "url": "/", "version": null })
        }

        async fn still_serves(ssr: &EmbeddedSsr) {
            let out = ssr.render(&page("ok")).await.expect("le pool sert encore");
            assert_eq!(out.body, r#"<div id="app">7</div>"#);
        }

        #[tokio::test]
        async fn renders_head_and_body() {
            let out = ssr().render(&page("ok")).await.unwrap();
            assert_eq!(out.head, vec!["<title>ok</title>"]);
            assert_eq!(out.body, r#"<div id="app">7</div>"#);
        }

        #[tokio::test]
        async fn infinite_loop_times_out_then_worker_is_recreated() {
            let ssr = ssr();
            let start = Instant::now();
            let err = ssr.render(&page("loop")).await.unwrap_err();
            assert!(matches!(err, SsrError::Timeout(_)), "{err:?}");
            assert!(
                start.elapsed() < TIMEOUT * 5,
                "délai non respecté : {:?}",
                start.elapsed()
            );
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn js_exception_reports_message() {
            let ssr = ssr();
            let err = ssr.render(&page("throw")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("intentional boom")),
                "{err:?}"
            );
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn console_error_during_render_is_a_failure() {
            let err = ssr().render(&page("console-error")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("error swallowed")),
                "{err:?}"
            );
        }

        #[tokio::test]
        async fn memory_limit_then_worker_is_recreated() {
            let ssr = ssr();
            let err = ssr.render(&page("oom")).await.unwrap_err();
            assert!(matches!(err, SsrError::OutOfMemory), "{err:?}");
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn never_resolved_promise_fails_fast() {
            let start = Instant::now();
            let err = ssr().render(&page("pending")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("never resolves")),
                "{err:?}"
            );
            assert!(start.elapsed() < TIMEOUT);
        }

        #[tokio::test]
        async fn deep_recursion_is_a_js_error_not_a_crash() {
            let ssr = ssr();
            let err = ssr.render(&page("deep")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("call stack")),
                "{err:?}"
            );
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn endless_microtasks_time_out() {
            let ssr = ssr();
            let start = Instant::now();
            let err = ssr.render(&page("microtask-loop")).await.unwrap_err();
            // V8 garde les promesses d'une même purge de microtâches : selon la limite, le tas est
            // plein avant le délai. Dans les deux cas le moteur est recréé.
            assert!(
                matches!(err, SsrError::Timeout(_)) || (HAS_INTL && matches!(err, SsrError::OutOfMemory)),
                "{err:?}"
            );
            assert!(start.elapsed() < TIMEOUT * 5, "{:?}", start.elapsed());
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn late_microtasks_belong_to_their_render() {
            let ssr = ssr();
            // Un rendu est fini quand la file de microtâches est vide : l'erreur tardive est la
            // sienne, et ne fait pas échouer la requête suivante.
            let err = ssr.render(&page("late-error")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("late error")),
                "{err:?}"
            );
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn intl_formats_by_locale_only_with_v8() {
            let result = ssr().render(&page("intl")).await;
            if HAS_INTL {
                assert_eq!(result.unwrap().body, "31 oct. 2010|1\u{202f}234,5|2 éléments");
            } else {
                // QuickJS n'implémente pas ECMA-402 : la page part en rendu client.
                let err = result.unwrap_err();
                assert!(
                    matches!(&err, SsrError::Js(m) if m.contains("Intl")),
                    "{err:?}"
                );
            }
        }

        #[tokio::test]
        async fn abandoned_renders_are_skipped() {
            let ssr = ssr();
            let loop_page = page("loop");
            let start = Instant::now();
            let (busy, served) = tokio::join!(ssr.render(&loop_page), async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                // Trois appelants partis (client déconnecté, délai côté HTTP) : leur rendu est en file.
                for _ in 0..3 {
                    let abandoned = tokio::time::timeout(Duration::from_millis(5), ssr.render(&loop_page));
                    assert!(abandoned.await.is_err());
                }
                ssr.render(&page("ok")).await
            });
            assert!(busy.is_err(), "{busy:?}");
            assert!(served.is_ok(), "{served:?}");
            // Sans le saut : quatre délais (un par rendu en boucle) avant de servir.
            assert!(start.elapsed() < TIMEOUT * 3, "{:?}", start.elapsed());
        }

        #[tokio::test]
        async fn unlimited_timeout_does_not_overflow() {
            let ssr = EmbeddedSsr::builder(Bundle::from_static(SCENARIOS)).engine(ENGINE)
                .workers(1)
                .render_timeout(Duration::MAX)
                .build()
                .unwrap();
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn wrong_output_shape_is_rejected() {
            let err = ssr().render(&page("bad-output")).await.unwrap_err();
            assert!(matches!(err, SsrError::InvalidOutput(_)), "{err:?}");
        }

        #[tokio::test]
        async fn provides_minimal_environment() {
            let out = ssr().render(&page("env")).await.unwrap();
            // URL vient du bundle (shim @moltn/vite), pas du moteur.
            assert_eq!(out.body, "undefined,production,true,function");
        }

        #[tokio::test]
        async fn timers_are_accepted_and_never_fire() {
            let ssr = ssr();
            let out = ssr.render(&page("timers")).await.unwrap();
            assert_eq!(out.body, "number,true,false");
            // Attendre un minuteur ne peut pas aboutir : erreur claire, pas de blocage.
            let err = ssr.render(&page("await-timer")).await.unwrap_err();
            assert!(
                matches!(&err, SsrError::Js(m) if m.contains("never resolves")),
                "{err:?}"
            );
            still_serves(&ssr).await;
        }

        #[tokio::test]
        async fn text_encoder_encodes_utf8() {
            let out = ssr().render(&page("utf8")).await.unwrap();
            // Le surrogate isolé devient U+FFFD, comme le TextEncoder des navigateurs.
            let expected: Vec<String> = "aé€😀\u{FFFD}".bytes().map(|b| b.to_string()).collect();
            assert_eq!(out.body, expected.join(","));
        }

        #[test]
        fn bundle_without_render_function_fails_at_build() {
            let err = EmbeddedSsr::builder(Bundle::from_static("globalThis.nothing = 1")).engine(ENGINE)
                .workers(1)
                .build()
                .expect_err("doit échouer");
            assert!(matches!(err, SsrError::MissingRender), "{err:?}");
        }

        #[test]
        fn invalid_bundle_fails_at_build() {
            let err = EmbeddedSsr::builder(Bundle::from_static("globalThis.__moltnRender = (")).engine(ENGINE)
                .workers(1)
                .build()
                .expect_err("doit échouer");
            assert!(matches!(err, SsrError::Load(_)), "{err:?}");
        }

        #[test]
        fn missing_bundle_file_names_the_path() {
            let err = EmbeddedSsr::builder(Bundle::from_file("missing/ssr.js")).engine(ENGINE)
                .build()
                .expect_err("doit échouer");
            assert!(err.to_string().contains("missing/ssr.js"), "{err}");
            assert!(err.to_string().contains("moltn build"), "{err}");
        }

        #[tokio::test]
        async fn full_queue_rejects_immediately() {
            let ssr = EmbeddedSsr::builder(Bundle::from_static(SCENARIOS)).engine(ENGINE)
                .workers(1)
                .queue_capacity(1)
                .render_timeout(TIMEOUT)
                .build()
                .unwrap();
            // Un rendu bloque le worker, un autre remplit la file : le troisième est refusé.
            let loop_page = page("loop");
            // Envois décalés : sinon les deux premiers peuvent tomber dans la file avant que le
            // worker n'ait pris le premier, et c'est le deuxième qui est refusé.
            let after = |ms| tokio::time::sleep(Duration::from_millis(ms));
            let (busy, queued, rejected) = tokio::join!(
                ssr.render(&loop_page),
                async {
                    after(20).await;
                    ssr.render(&loop_page).await
                },
                async {
                    after(40).await;
                    let start = Instant::now();
                    (ssr.render(&page("ok")).await, start.elapsed())
                }
            );
            assert!(busy.is_err() && queued.is_err(), "{busy:?} {queued:?}");
            let (result, waited) = rejected;
            assert!(matches!(result, Err(SsrError::QueueFull)), "{result:?}");
            assert!(waited < Duration::from_millis(20), "attente : {waited:?}");
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn thousand_concurrent_renders() {
            let ssr = std::sync::Arc::new(
                EmbeddedSsr::builder(Bundle::from_static(SCENARIOS)).engine(ENGINE)
                    .workers(4)
                    .queue_capacity(1000)
                    .build()
                    .unwrap(),
            );
            let tasks: Vec<_> = (0..1000)
                .map(|n| {
                    let ssr = ssr.clone();
                    tokio::spawn(async move {
                        let page = json!({ "component": "ok", "props": { "n": n }, "url": "/" });
                        ssr.render(&page)
                            .await
                            .map(|out| out.body == format!(r#"<div id="app">{n}</div>"#))
                    })
                })
                .collect();
            for task in tasks {
                assert!(
                    task.await.unwrap().unwrap(),
                    "réponse mélangée entre requêtes"
                );
            }
        }
        }
    };
}

#[cfg(feature = "quickjs")]
engine_tests!(quickjs, Engine::QuickJs, intl = false);
#[cfg(feature = "v8")]
engine_tests!(v8, Engine::V8, intl = true);
