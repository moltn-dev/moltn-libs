//! `pnpm -r build` puis `cargo run -p axum-vue` → http://127.0.0.1:3000

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app = axum_vue::app()?;
    // `moltn dev` transmet le port choisi par `PORT`.
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
