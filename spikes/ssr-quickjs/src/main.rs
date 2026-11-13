//! Spike moltn : une app Inertia v3 + Vue rendue par QuickJS dans un binaire axum + veer.
//! `SPIKE_WEB=web-react cargo run --release` : serveur sur :3000. `cargo run --release -- bench [n]` : banc de rendu.

mod engine;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{Router, extract::{Path, State}, routing::{get, post}};
use serde::Deserialize;
use serde_json::json;
use tower_http::services::ServeDir;
use veer::{Inertia, InertiaConfig, InertiaForm, InertiaLayer, ViteManifest, ViteRootView};

type Names = Arc<Mutex<Vec<String>>>;

#[derive(Deserialize)]
struct NewName {
    name: String,
}

#[tokio::main]
async fn main() {
    // Front choisi par SPIKE_WEB : web-vue (défaut), web-react, web-svelte.
    let web = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), std::env::var("SPIKE_WEB").unwrap_or_else(|_| "web-vue".into()));
    let bundle = std::fs::read_to_string(format!("{web}/dist/ssr/ssr.js")).expect("lancer `pnpm build` dans le dossier front");
    let robust_mode = std::env::args().nth(1).as_deref() == Some("robust");
    // `robust` : un seul worker, pour prouver que c'est bien lui qui est recréé.
    let workers = if robust_mode { 1 } else { std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1)) };
    let t = Instant::now();
    let engine = engine::Engine::new(bundle, workers, Duration::from_millis(500), 64).expect("chargement SSR");
    eprintln!("[ssr] {workers} workers prêts en {:?}", t.elapsed());

    if std::env::args().nth(1).as_deref() == Some("bench") {
        let n: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(500);
        return bench(&engine, n).await;
    }
    if std::env::args().nth(1).as_deref() == Some("robust") {
        return robust(&engine).await;
    }

    let manifest = ViteManifest::load(format!("{web}/dist/client/.vite/manifest.json")).expect("manifeste Vite");
    let view = ViteRootView::production().manifest(manifest).entry("src/app.ts").asset_base("/build").title("moltn");
    let config = InertiaConfig::new().root_view(view).ssr(engine);

    let names: Names = Arc::new(Mutex::new(vec!["Ada".into(), "Grace".into()]));
    let app = Router::new()
        .route(
            "/",
            get(|i: Inertia, State(n): State<Names>| async move {
                let names = n.lock().map(|v| v.clone()).unwrap_or_default();
                i.render("index", json!({ "names": names }))
            }),
        )
        .route(
            "/names",
            post(|i: Inertia, State(n): State<Names>, InertiaForm(f): InertiaForm<NewName>| async move {
                if let Ok(mut v) = n.lock() {
                    v.push(f.name);
                }
                i.redirect("/")
            }),
        )
        .route(
            "/users/{id}",
            get(|i: Inertia, Path(id): Path<u32>| async move {
                i.render("users/[id]", json!({ "user": { "id": id, "name": format!("Utilisateur {id}") } }))
            }),
        )
        .nest_service("/build", ServeDir::new(format!("{web}/dist/client")))
        .layer(InertiaLayer::new(config))
        .with_state(names);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.expect("port 3000");
    eprintln!("http://127.0.0.1:3000");
    axum::serve(listener, app).await.expect("serveur");
}

async fn bench(engine: &engine::Engine, n: usize) {
    let page = |i: usize| {
        json!({
            "component": "index",
            "props": { "names": (0..100).map(|k| format!("nom {k}")).collect::<Vec<_>>(), "errors": {} },
            "url": format!("/?i={i}"), "version": null,
            "clearHistory": false, "encryptHistory": false,
        })
        .to_string()
    };
    // Premier rendu à part : compilation paresseuse des fonctions.
    let t = Instant::now();
    let first = engine.render_json(page(0)).await.expect("premier rendu");
    eprintln!("premier rendu : {:?}\n{}", t.elapsed(), &first.body[..first.body.len().min(400)]);
    let mut times = Vec::with_capacity(n);
    for i in 0..n {
        let t = Instant::now();
        engine.render_json(page(i)).await.expect("rendu");
        times.push(t.elapsed());
    }
    times.sort();
    let pct = |p: f64| times[((times.len() as f64 * p) as usize).min(times.len() - 1)];
    eprintln!("{n} rendus séquentiels (liste de 100) : médiane {:?}, p99 {:?}, max {:?}", pct(0.5), pct(0.99), times[n - 1]);
}

/// Boucle infinie et exception : délai respecté, worker recréé, service intact.
async fn robust(engine: &engine::Engine) {
    let page = |c: &str, mode: &str| json!({ "component": c, "props": { "mode": mode, "names": [], "errors": {} }, "url": "/", "version": null }).to_string();
    for (c, m) in [("boom", "loop"), ("boom", "throw"), ("index", "")] {
        let t = Instant::now();
        let r = engine.render_json(page(c, m)).await;
        eprintln!("{c}/{m} -> {} en {:?}", if r.is_ok() { "ok" } else { "erreur (repli client)" }, t.elapsed());
    }
}
