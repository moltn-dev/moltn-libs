//! Conformité de l'exemple Loco : pages moltn rendues côté serveur, visite Inertia,
//! contrôleur Loco (formulaire), routes Loco par défaut. Prérequis : `pnpm -r build`.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use loco_rs::app::Hooks;
use loco_rs::boot::StartMode;
use loco_rs::environment::Environment;
use loco_vue::App;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app() -> Router {
    let config = App::load_config(&Environment::Test)
        .await
        .expect("config/test.yaml");
    let boot = App::boot(StartMode::ServerOnly, &Environment::Test, config)
        .await
        .expect("Loco startup: run `pnpm -r build`");
    boot.router.expect("web router")
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app.clone().oneshot(request).await.expect("response");
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, usize::MAX).await.expect("body");
    (
        parts.status,
        parts.headers,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

async fn get_html(app: &Router, uri: &str) -> String {
    let (status, _, html) = send(app, Request::get(uri).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{uri}");
    html
}

fn page_object(html: &str) -> Value {
    let start = html
        .find(r#"<script data-page="app" type="application/json">"#)
        .expect("script data-page");
    let json = &html[start..];
    let json = &json[json.find('>').unwrap() + 1..json.find("</script>").unwrap()];
    serde_json::from_str(json).expect("page object JSON")
}

#[test]
fn pages_are_valid() {
    moltn_pages::assert_valid::<loco_rs::app::AppContext>(loco_vue::pages());
}

#[tokio::test]
async fn page_rendered_with_loco_state() {
    let html = get_html(&app().await, "/").await;
    assert!(html.contains(r#"data-server-rendered="true""#), "{html}");
    assert!(html.contains("<li>Ada</li><li>Grace</li>"), "{html}");
    assert_eq!(
        page_object(&html)["props"]["names"],
        json!(["Ada", "Grace"])
    );
}

#[tokio::test]
async fn inertia_visit_returns_json() {
    let app = app().await;
    let version = page_object(&get_html(&app, "/").await)["version"].clone();
    let request = Request::get("/users/42")
        .header("X-Inertia", "true")
        .header("X-Inertia-Version", version.as_str().expect("version"))
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    let page: Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(page["component"], "users/[id]");
    assert_eq!(page["props"]["user"]["id"], 42);
}

#[tokio::test]
async fn loco_controller_form_redirects() {
    let app = app().await;
    let request = Request::post("/names")
        .header("X-Inertia", "true")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"Linus"}"#))
        .unwrap();
    let (status, headers, _) = send(&app, request).await;
    assert!(status.is_redirection(), "status {status}");
    assert_eq!(headers[header::LOCATION], "/");
    assert!(get_html(&app, "/").await.contains("<li>Linus</li>"));
}

#[tokio::test]
async fn loco_default_routes_still_work() {
    let (status, _, body) = send(
        &app().await,
        Request::get("/_ping").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("ok"), "{body}");
}

#[tokio::test]
async fn links_and_assets_are_served() {
    let app = app().await;
    let html = get_html(&app, "/").await;
    for href in ["/users/42", "/about"] {
        assert!(html.contains(&format!(r#"href="{href}""#)), "{href}");
        get_html(&app, href).await;
    }
    let start = html.find("/build/assets/").expect("entry script");
    let path = &html[start..start + html[start..].find('"').unwrap()];
    let (status, _, _) = send(&app, Request::get(path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{path}");
}
