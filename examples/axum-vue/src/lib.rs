//! Exemple moltn : axum + veer + Vue, rendu serveur dans le binaire, routage par fichiers.
//!
//! Chaque fichier de `web/src/pages/` est une route (`users/[id].vue` → `/users/{id}`) ;
//! les pages qui ont besoin de données déclarent un loader `#[page]`. Le front se construit
//! avec `pnpm -r build` ; assets et bundle SSR sont lus sur le disque en debug et inclus
//! dans le binaire en release.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use moltn_pages::{PageTable, page};
use moltn_ssr::EmbeddedSsr;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use veer::ssr::http::HttpSsrClient;
use veer::{
    EmbeddedAssets, Inertia, InertiaConfig, InertiaForm, InertiaLayer, ViteManifest, ViteRootView,
};

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist/client/"]
#[allow_missing = true]
struct Assets;

/// État de l'application : noms saisis dans le formulaire de l'accueil (en mémoire).
pub type Names = Arc<Mutex<Vec<String>>>;

#[derive(Deserialize)]
struct NewName {
    name: String,
}

/// Table des pages, générée par `build.rs` depuis `web/src/pages/`.
pub fn pages() -> &'static PageTable {
    moltn_pages::pages!()
}

/// Application complète : pages Inertia, formulaire, assets.
///
/// Avec `MOLTN_VITE_URL` (posée par `moltn dev`), les assets et le rendu serveur viennent du
/// serveur de dev Vite (HMR compris) ; sinon, du bundle et des assets construits.
pub fn app() -> anyhow::Result<Router> {
    // D'abord : vérifie pages et loaders, tient `web/src/generated/moltn.ts` à jour (debug)
    // et répond à `moltn gen` avant d'avoir besoin du front construit.
    let pages = moltn_pages::router::<Names>(pages())?;

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

    // Les pages viennent des fichiers ; les mutations restent des routes axum classiques.
    Ok(pages
        .route("/names", post(add_name))
        .nest_service(
            "/build",
            EmbeddedAssets::new(|path| Assets::get(path).map(|f| f.data)),
        )
        .layer(InertiaLayer::new(inertia))
        .with_state(names))
}

#[derive(Serialize, TS)]
struct IndexProps {
    names: Vec<String>,
}

#[page("index")]
async fn index(State(names): State<Names>) -> Result<IndexProps, Infallible> {
    let names = names.lock().map(|n| n.clone()).unwrap_or_default();
    Ok(IndexProps { names })
}

#[derive(Serialize, TS)]
struct User {
    id: u32,
    name: String,
}

#[derive(Serialize, TS)]
struct UserProps {
    user: User,
}

#[page("users/[id]")]
async fn user_show(Path(id): Path<u32>) -> Result<UserProps, StatusCode> {
    if id == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(UserProps {
        user: User {
            id,
            name: format!("User {id}"),
        },
    })
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
