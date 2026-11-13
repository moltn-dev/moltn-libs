//! Conformité de l'exemple : HTML rendu côté serveur, visite Inertia, formulaire.
//! Prérequis : `pnpm -r build` (front et bundle SSR).

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    axum_react::app().expect("application: run `pnpm -r build`")
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app.clone().oneshot(request).await.expect("response");
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, usize::MAX).await.expect("body");
    (
        parts.status,
        parts.headers,
        String::from_utf8(body.to_vec()).expect("UTF-8"),
    )
}

async fn get_html(app: &Router, uri: &str) -> String {
    let (status, _, html) = send(app, Request::get(uri).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    html
}

/// Page object embarqué dans `<script data-page="app">`.
fn page_object(html: &str) -> Value {
    let start = html
        .find(r#"<script data-page="app" type="application/json">"#)
        .expect("script data-page");
    let json = &html[start..];
    let json = &json[json.find('>').unwrap() + 1..json.find("</script>").unwrap()];
    serde_json::from_str(json).expect("page object JSON")
}

#[tokio::test]
async fn first_visit_is_server_rendered() {
    let html = get_html(&app(), "/").await;
    assert!(
        html.contains(r#"data-server-rendered="true""#),
        "no server-side rendering:\n{html}"
    );
    assert!(
        html.contains("<li>Ada</li><li>Grace</li>"),
        "props not rendered:\n{html}"
    );
    assert!(
        html.contains(r#"<a href="/users/42">"#),
        "<Link> not rendered:\n{html}"
    );
    assert!(html.contains("<title"), "page title missing");
    let page = page_object(&html);
    assert_eq!(page["component"], "index");
    assert_eq!(page["props"]["names"], json!(["Ada", "Grace"]));
}

#[tokio::test]
async fn dynamic_page_is_server_rendered() {
    let html = get_html(&app(), "/users/42").await;
    // Neutre vis-à-vis des marqueurs que React insère entre les fragments de texte.
    assert!(html.contains("<h1>User 42"), "{html}");
    assert_eq!(page_object(&html)["props"]["user"]["id"], 42);
}

#[tokio::test]
async fn inertia_visit_returns_json() {
    let app = app();
    let version = page_object(&get_html(&app, "/").await)["version"].clone();
    let request = Request::get("/users/42")
        .header("X-Inertia", "true")
        .header("X-Inertia-Version", version.as_str().expect("version"))
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-inertia"], "true");
    let page: Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(page["component"], "users/[id]");
    assert_eq!(
        page["props"]["user"],
        json!({ "id": 42, "name": "User 42" })
    );
    assert_eq!(page["url"], "/users/42");
}

#[tokio::test]
async fn form_post_redirects_and_persists() {
    let app = app();
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
async fn assets_are_served() {
    let html = get_html(&app(), "/").await;
    let start = html.find("/build/assets/").expect("entry script");
    let path = &html[start..start + html[start..].find('"').unwrap()];
    let (status, headers, _) = send(&app(), Request::get(path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{path}");
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .contains("javascript")
    );
}
