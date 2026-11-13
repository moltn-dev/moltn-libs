//! `pnpm -r build` puis `cargo run -p loco-vue` (ou `moltn dev` depuis ce dossier).

use loco_rs::app::Hooks;
use loco_rs::boot::{ServeParams, StartMode};
use loco_rs::environment::{Environment, resolve_from_env};
use loco_vue::App;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    let environment = Environment::from(resolve_from_env());
    let config = App::load_config(&environment).await?;
    // `moltn dev` choisit le port par `PORT` ; sinon, celui de la configuration Loco.
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(config.server.port);
    let binding = config.server.binding.clone();
    let boot = App::boot(StartMode::ServerOnly, &environment, config).await?;
    loco_rs::boot::start::<App>(boot, ServeParams { port, binding }, true).await
}
