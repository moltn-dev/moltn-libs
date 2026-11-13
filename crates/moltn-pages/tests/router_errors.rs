//! Routeur : incohérences entre pages et loaders, toutes signalées d'un coup.
//! Binaire de test séparé : les loaders `#[page]` sont globaux au binaire.

use axum::extract::{Path, State};
use moltn_pages::{PageFile, PageTable, page};
use serde_json::Value;

type Infallible = std::convert::Infallible;

#[derive(Clone)]
struct AppState;

#[derive(Clone)]
struct OtherState;

/// Vise une page qui n'existe pas.
#[page("profil")]
async fn orphan() -> Result<Value, Infallible> {
    Ok(Value::Null)
}

/// Extrait `slug` alors que la page définit `[id]`.
#[page("users/[id]")]
async fn wrong_param(Path(slug): Path<String>) -> Result<Value, Infallible> {
    Ok(Value::String(slug))
}

/// Lit un autre état que celui de l'application.
#[page("settings")]
async fn wrong_state(State(_): State<OtherState>) -> Result<Value, Infallible> {
    Ok(Value::Null)
}

static PAGES: &[PageFile] = &[
    PageFile {
        file: "posts/[slug].vue",
        component: "posts/[slug]",
        routes: &["/posts/{slug}"],
        params: &["slug"],
    },
    PageFile {
        file: "settings.vue",
        component: "settings",
        routes: &["/settings"],
        params: &[],
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

#[test]
fn all_problems_reported_together() {
    let err = moltn_pages::router::<AppState>(&TABLE).expect_err("doit échouer");
    let message = err.to_string();
    assert_eq!(err.problems.len(), 4, "{message}");

    // Critère M4 : page dynamique sans loader → message clair, avec la correction.
    assert!(
        message.contains(r#"page posts/[slug].vue has parameters ([slug]) but no loader: add #[moltn_pages::page("posts/[slug]")]"#),
        "{message}"
    );
    assert!(
        message.contains("orphan") && message.contains(r#"page "profil", which does not exist"#),
        "{message}"
    );
    assert!(
        message.contains("wrong_param")
            && message.contains("extracts Path(slug) but users/[id].vue defines [id]"),
        "{message}"
    );
    assert!(
        message.contains("reads State<")
            && message.contains("OtherState")
            && message.contains("state = "),
        "{message}"
    );
    // Chaque loader fautif est localisé.
    assert!(message.contains("router_errors.rs:"), "{message}");
}

#[test]
#[should_panic(expected = "no loader")]
fn assert_valid_panics_with_the_list() {
    moltn_pages::assert_valid::<AppState>(&TABLE);
}
