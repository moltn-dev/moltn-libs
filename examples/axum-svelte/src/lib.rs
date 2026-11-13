//! Exemple moltn : axum + veer + Svelte, rendu serveur dans le binaire.
//!
//! Le front (`web/`) se construit avec `pnpm -r build` ; ses assets et son bundle SSR
//! sont lus sur le disque en debug et inclus dans le binaire en release.

use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use moltn_ssr::EmbeddedSsr;
use serde::Deserialize;
use serde_json::json;
use veer::ssr::http::HttpSsrClient;
use veer::{
    EmbeddedAssets, Inertia, InertiaConfig, InertiaForm, InertiaLayer, ViteManifest, ViteRootView,
};

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist/client/"]
#[allow_missing = true]
struct Assets;

/// Noms saisis dans le formulaire de la page d'accueil (en mémoire).
type Names = Arc<Mutex<Vec<String>>>;

#[derive(Deserialize)]
struct NewName {
    name: String,
}

/// Application complète : pages Inertia, formulaire, assets.
///
/// Avec `MOLTN_VITE_URL` (posée par `moltn dev`), les assets et le rendu serveur viennent du
/// serveur de dev Vite (HMR compris) ; sinon, du bundle et des assets construits.
pub fn app() -> anyhow::Result<Router> {
    let inertia = match std::env::var("MOLTN_VITE_URL") {
        Ok(url) => InertiaConfig::new()
            .root_view(ViteRootView::dev().dev_server(&url).entry("src/app.ts"))
            .ssr(HttpSsrClient::new(format!("{url}/__inertia_ssr"))),
        Err(_) => {
            let manifest = Assets::get(".vite/manifest.json")
                .context("Vite manifest not found: run `pnpm -r build`")?;
            let manifest: ViteManifest = std::str::from_utf8(&manifest.data)?.parse()?;
            let view = ViteRootView::production()
                .manifest(manifest)
                .entry("src/app.ts")
                .asset_base("/build");
            let ssr = EmbeddedSsr::builder(moltn_ssr::embedded_bundle!()).build()?;
            InertiaConfig::new().root_view(view).ssr(ssr)
        }
    };
    let names: Names = Arc::new(Mutex::new(vec!["Ada".into(), "Grace".into()]));

    Ok(Router::new()
        .route("/", get(index))
        .route("/names", post(add_name))
        .route("/users/{id}", get(user_show))
        .nest_service(
            "/build",
            EmbeddedAssets::new(|path| Assets::get(path).map(|f| f.data)),
        )
        .layer(InertiaLayer::new(inertia))
        .with_state(names))
}

async fn index(inertia: Inertia, State(names): State<Names>) -> veer::InertiaResponse {
    let names = names.lock().map(|n| n.clone()).unwrap_or_default();
    inertia.render("index", json!({ "names": names }))
}

async fn add_name(
    inertia: Inertia,
    State(names): State<Names>,
    InertiaForm(form): InertiaForm<NewName>,
) -> veer::InertiaResponse {
    if let Ok(mut names) = names.lock() {
        names.push(form.name);
    }
    inertia.redirect("/")
}

async fn user_show(inertia: Inertia, Path(id): Path<u32>) -> veer::InertiaResponse {
    inertia.render(
        "users/[id]",
        json!({ "user": { "id": id, "name": format!("User {id}") } }),
    )
}
