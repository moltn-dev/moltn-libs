//! Exemple moltn sur Loco : les pages de `web/src/pages/` et leurs loaders `#[page]` se
//! branchent dans le routeur de Loco (`before_routes`) ; les contrôleurs Loco restent
//! inchangés à côté ; l'état des loaders est l'`AppContext` de Loco.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use async_trait::async_trait;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use loco_rs::Result;
use loco_rs::app::{AppContext, Hooks};
use loco_rs::bgworker::Queue;
use loco_rs::boot::{BootResult, StartMode, create_app};
use loco_rs::config::Config;
use loco_rs::controller::AppRoutes;
use loco_rs::environment::Environment;
use loco_rs::task::Tasks;
use moltn_pages::{PageTable, page};
use moltn_ssr::EmbeddedSsr;
use serde::Serialize;
use ts_rs::TS;
use veer::ssr::http::HttpSsrClient;
use veer::{EmbeddedAssets, InertiaConfig, InertiaLayer, ViteManifest, ViteRootView};

pub mod controllers;

/// Noms saisis dans le formulaire (en mémoire), rangés dans le `shared_store` de Loco.
#[derive(Clone, Default)]
pub struct Names(pub Arc<Mutex<Vec<String>>>);

/// Table des pages, générée par `build.rs` depuis `web/src/pages/`.
pub fn pages() -> &'static PageTable {
    moltn_pages::pages!()
}

#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist/client/"]
#[allow_missing = true]
struct Assets;

pub struct App;

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self>(mode, environment, config).await
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        ctx.shared_store.insert(Names(Arc::new(Mutex::new(vec![
            "Ada".into(),
            "Grace".into(),
        ]))));
        Ok(ctx)
    }

    /// Les pages moltn forment le routeur de départ de Loco (état : `AppContext`).
    async fn before_routes(_ctx: &AppContext) -> Result<axum::Router<AppContext>> {
        moltn_pages::router::<AppContext>(pages())
            .map_err(|e| loco_rs::Error::string(&e.to_string()))
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes().add_route(controllers::names::routes())
    }

    /// Protocole Inertia (veer) et assets du front, autour de toutes les routes.
    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {
        let inertia = inertia_config().map_err(|e| loco_rs::Error::string(&format!("{e:#}")))?;
        Ok(router
            .nest_service(
                "/build",
                EmbeddedAssets::new(|path| Assets::get(path).map(|f| f.data)),
            )
            .layer(InertiaLayer::new(inertia)))
    }

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    fn register_tasks(_tasks: &mut Tasks) {}
}

/// Avec `MOLTN_VITE_URL` (posée par `moltn dev`) : assets et rendu serveur par Vite ;
/// sinon : bundle et assets construits, dans le binaire.
fn inertia_config() -> anyhow::Result<InertiaConfig> {
    Ok(match std::env::var("MOLTN_VITE_URL") {
        Ok(url) => InertiaConfig::new()
            .root_view(ViteRootView::dev().dev_server(&url).entry("src/app.ts"))
            .ssr(HttpSsrClient::new(format!("{url}/__inertia_ssr"))),
        Err(_) => {
            let manifest = Assets::get(".vite/manifest.json")
                .context("Vite manifest not found: run `moltn build` or `pnpm -r build`")?;
            let manifest: ViteManifest = std::str::from_utf8(&manifest.data)?.parse()?;
            let view = ViteRootView::production()
                .manifest(manifest)
                .entry("src/app.ts")
                .asset_base("/build");
            let ssr = EmbeddedSsr::builder(moltn_ssr::embedded_bundle!()).build()?;
            InertiaConfig::new().root_view(view).ssr(ssr)
        }
    })
}

#[derive(Serialize, TS)]
struct IndexProps {
    names: Vec<String>,
}

/// Loader moltn qui lit l'état Loco : `State<AppContext>`.
#[page("index")]
async fn index(State(ctx): State<AppContext>) -> std::result::Result<IndexProps, Infallible> {
    let names = ctx
        .shared_store
        .get::<Names>()
        .and_then(|n| n.0.lock().ok().map(|n| n.clone()))
        .unwrap_or_default();
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
async fn user_show(Path(id): Path<u32>) -> std::result::Result<UserProps, StatusCode> {
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
