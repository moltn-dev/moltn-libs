//! Routeur : pages et loaders cohérents, requêtes Inertia de bout en bout.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use moltn_pages::{PageFile, PageTable, page};
use serde::Serialize;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Clone, Default)]
struct AppState {
    greeting: Arc<str>,
}

/// Props typées : `UserProps` apparaît dans le module TypeScript.
#[derive(Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct UserProps {
    id: u32,
    greeting: String,
    is_admin: bool,
}

#[page("users/[id]")]
async fn user_show(
    Path(id): Path<u32>,
    State(app): State<AppState>,
) -> Result<UserProps, StatusCode> {
    if id == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(UserProps {
        id,
        greeting: app.greeting.to_string(),
        is_admin: false,
    })
}

/// Fichier `[teamId]`, variable Rust `team_id` : le même paramètre, pas une incohérence.
#[page("teams/[teamId]")]
async fn team(Path(team_id): Path<String>) -> Result<Value, std::convert::Infallible> {
    Ok(json!({ "team": team_id }))
}

/// Sans `State` : branché dans le `Router<AppState>` quand même.
#[page("docs/[...slug]")]
async fn docs(Path(slug): Path<String>) -> Result<Value, std::convert::Infallible> {
    Ok(json!({ "slug": slug }))
}

#[page("tabs/[[tab]]")]
async fn tabs(tab: Option<Path<String>>) -> Result<Value, std::convert::Infallible> {
    Ok(json!({ "tab": tab.map(|Path(t)| t) }))
}

static PAGES: &[PageFile] = &[
    PageFile {
        file: "about.vue",
        component: "about",
        routes: &["/about"],
        params: &[],
    },
    PageFile {
        file: "docs/[...slug].vue",
        component: "docs/[...slug]",
        routes: &["/docs/{*slug}"],
        params: &["slug"],
    },
    PageFile {
        file: "player.vue",
        component: "player",
        routes: &["/player"],
        params: &[],
    },
    PageFile {
        file: "tabs/[[tab]].vue",
        component: "tabs/[[tab]]",
        routes: &["/tabs", "/tabs/{tab}"],
        params: &["tab"],
    },
    PageFile {
        file: "teams/[teamId].vue",
        component: "teams/[teamId]",
        routes: &["/teams/{teamId}"],
        params: &["teamId"],
    },
    PageFile {
        file: "users/[id].vue",
        component: "users/[id]",
        routes: &["/users/{id}"],
        params: &["id"],
    },
];

static TABLE: PageTable = PageTable {
    pages: PAGES,
    typescript: None,
};

fn app() -> axum::Router {
    moltn_pages::router::<AppState>(&TABLE)
        .expect("pages valides")
        .layer(veer::InertiaLayer::new(
            veer::InertiaConfig::new().version_str("test"),
        ))
        .with_state(AppState {
            greeting: "bonjour".into(),
        })
}

async fn visit(uri: &str) -> (StatusCode, Value) {
    let request = Request::get(uri)
        .header("X-Inertia", "true")
        .header("X-Inertia-Version", "test")
        .body(Body::empty())
        .unwrap();
    let response = app().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn loader_with_state() {
    let (status, page) = visit("/users/42").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["component"], "users/[id]");
    assert_eq!(page["props"]["id"], 42);
    assert_eq!(page["props"]["greeting"], "bonjour");
}

#[tokio::test]
async fn camel_case_file_param_matches_snake_case_binding() {
    assert_eq!(visit("/teams/red").await.1["props"]["team"], "red");
}

#[tokio::test]
async fn loader_error_is_its_response() {
    assert_eq!(visit("/users/0").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn page_without_loader_has_empty_props() {
    let (_, page) = visit("/about").await;
    assert_eq!(page["component"], "about");
    assert_eq!(page["props"]["errors"], json!({}));
}

#[tokio::test]
async fn catch_all_and_optional() {
    assert_eq!(
        visit("/docs/guide/install").await.1["props"]["slug"],
        "guide/install"
    );
    assert_eq!(visit("/tabs").await.1["props"]["tab"], Value::Null);
    assert_eq!(visit("/tabs/profil").await.1["props"]["tab"], "profil");
}

#[test]
fn assert_valid_passes() {
    moltn_pages::assert_valid::<AppState>(&TABLE);
}

#[derive(Serialize, ts_rs::TS)]
struct Shared {
    app_name: String,
}
moltn_pages::shared_props!(Shared);

/// Module TypeScript complet : types des props (avec `rename_all` de serde), `Pages`,
/// props partagées et routes.
#[test]
fn typescript_module_snapshot() {
    let ts = moltn_pages::typescript(PAGES);
    assert!(
        ts.contains("isAdmin: boolean"),
        "serde rename_all respecté :
{ts}"
    );
    insta::assert_snapshot!(ts);
}

/// Dossier `pages/` identique à `PAGES`, et `moltn.ts` tel que l'application l'écrit.
fn pages_dir_with_typescript() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let pages = root.path().join("pages");
    for page in PAGES {
        let path = pages.join(page.file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    let ts = root.path().join("generated/moltn.ts");
    std::fs::create_dir_all(ts.parent().unwrap()).unwrap();
    std::fs::write(&ts, moltn_pages::typescript(PAGES)).unwrap();
    (root, pages, ts)
}

/// Un fichier restitué en CRLF par git (Windows, `autocrlf`) est à jour, pas périmé.
#[test]
fn refresh_ignores_crlf_line_endings() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    let crlf = std::fs::read_to_string(&ts).unwrap().replace('\n', "\r\n");
    std::fs::write(&ts, &crlf).unwrap();
    assert!(!moltn_pages::build::refresh_typescript(&pages, &ts).unwrap());
    assert_eq!(
        std::fs::read_to_string(&ts).unwrap(),
        crlf,
        "fichier intact"
    );
}

/// Sans changement de pages, la mise à jour rapide ne touche pas au fichier de l'application
/// (sinon Vite rechargerait pour rien).
#[test]
fn refresh_is_a_no_op_on_an_up_to_date_file() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    let before = std::fs::read_to_string(&ts).unwrap();
    assert!(!moltn_pages::build::refresh_typescript(&pages, &ts).unwrap());
    assert_eq!(std::fs::read_to_string(&ts).unwrap(), before);
}

#[test]
fn refresh_adds_a_new_page_and_keeps_known_types() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    std::fs::write(pages.join("blog.vue"), "").unwrap();
    assert!(moltn_pages::build::refresh_typescript(&pages, &ts).unwrap());
    let content = std::fs::read_to_string(&ts).unwrap();
    // Nouvelle page : dans Pages (props encore inconnues) et dans les routes.
    assert!(content.contains("  'blog': {}\n"), "{content}");
    assert!(content.contains("  'blog': '/blog',"), "{content}");
    // Types déjà connus : conservés.
    assert!(content.contains("export type UserProps = "), "{content}");
    assert!(content.contains("  'users/[id]': UserProps\n"), "{content}");
    assert!(
        content.contains("sharedPageProps: SharedProps"),
        "{content}"
    );
}

#[test]
fn refresh_removes_a_deleted_page() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    std::fs::remove_file(pages.join("users/[id].vue")).unwrap();
    assert!(moltn_pages::build::refresh_typescript(&pages, &ts).unwrap());
    let content = std::fs::read_to_string(&ts).unwrap();
    // Les commentaires du fichier citent `users/[id]` en exemple : on vérifie les entrées.
    assert!(!content.contains("  'users/[id]':"), "{content}");
}

#[test]
fn refresh_without_a_file_writes_routes_with_empty_props() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    std::fs::remove_file(&ts).unwrap();
    assert!(moltn_pages::build::refresh_typescript(&pages, &ts).unwrap());
    let content = std::fs::read_to_string(&ts).unwrap();
    assert!(content.contains("  'users/[id]': {}\n"), "{content}");
    assert!(content.contains("export function route"), "{content}");
}

#[test]
fn refresh_reports_page_conflicts() {
    let (_root, pages, ts) = pages_dir_with_typescript();
    std::fs::write(pages.join("about/index.vue"), "").ok();
    std::fs::create_dir_all(pages.join("about")).unwrap();
    std::fs::write(pages.join("about/index.vue"), "").unwrap();
    let err = moltn_pages::build::refresh_typescript(&pages, &ts).unwrap_err();
    assert!(
        err.to_string().contains("produce the same route /about"),
        "{err}"
    );
}

/// Faux moteur SSR : marque le HTML pour savoir s'il a été appelé.
struct Marker;

#[async_trait::async_trait]
impl veer::SsrClient for Marker {
    async fn render(&self, _page: &Value) -> Result<veer::SsrPayload, veer::ssr::SsrError> {
        Ok(veer::SsrPayload {
            head: vec![],
            body: "<p>SERVER-RENDERED</p>".into(),
        })
    }
}

#[page("player", ssr = false)]
async fn player() -> Result<Value, std::convert::Infallible> {
    Ok(json!({}))
}

#[tokio::test]
async fn ssr_false_leaves_the_page_to_the_browser() {
    let app = moltn_pages::router::<AppState>(&TABLE)
        .expect("pages valides")
        .layer(veer::InertiaLayer::new(
            veer::InertiaConfig::new().ssr(Marker),
        ))
        .with_state(AppState::default());
    let html = |uri: &'static str| {
        let app = app.clone();
        async move {
            let response = app
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(body.to_vec()).unwrap()
        }
    };
    assert!(html("/about").await.contains("SERVER-RENDERED"));
    let player = html("/player").await;
    assert!(!player.contains("SERVER-RENDERED"), "{player}");
    assert!(player.contains("data-page"), "{player}");
}
