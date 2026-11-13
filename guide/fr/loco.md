# Avec Loco

[Loco](https://loco.rs) est un framework Rust construit sur axum. moltn s'y branche sans adaptateur : les pages moltn s'ajoutent au routeur de Loco, les contrôleurs Loco restent tels quels, et les loaders lisent l'`AppContext` de Loco.

Un exemple complet est dans le dépôt : `examples/loco-vue`.

## Dépendances

En plus de celles d'un projet moltn (`moltn-pages`, `moltn-ssr`, `veer`, `ts-rs`, `rust-embed`) :

```toml
[dependencies]
loco-rs = { version = "1.3", default-features = false }   # ajoutez les features Loco dont vous avez besoin
async-trait = "0.1"
```

Et le même `build.rs` qu'un projet moltn (`moltn_pages::build::scan_project()`).

## Brancher moltn dans `Hooks`

```rust
#[async_trait]
impl Hooks for App {
    // Les pages moltn forment le routeur de départ de Loco.
    async fn before_routes(_ctx: &AppContext) -> Result<axum::Router<AppContext>> {
        moltn_pages::router::<AppContext>(moltn_pages::pages!())
            .map_err(|e| loco_rs::Error::string(&e.to_string()))
    }

    // Les contrôleurs Loco, inchangés.
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes().add_route(controllers::posts::routes())
    }

    // Protocole Inertia et assets du front, autour de toutes les routes.
    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {
        let inertia = inertia_config()?;   // comme dans le main.rs d'un projet moltn
        Ok(router
            .nest_service("/build", EmbeddedAssets::new(|p| Assets::get(p).map(|f| f.data)))
            .layer(InertiaLayer::new(inertia)))
    }

    // … app_name, boot, connect_workers, register_tasks comme d'habitude
}
```

## Des loaders qui lisent l'état Loco

```rust
#[page("posts/[id]")]
async fn post_show(
    Path(id): Path<i32>,
    State(ctx): State<AppContext>,
) -> Result<PostProps, loco_rs::Error> {
    let post = posts::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(loco_rs::Error::NotFound)?;
    Ok(PostProps { post: post.into() })
}
```

## Avec `moltn dev`

`moltn dev` lance l'application par `cargo run` et lui passe son port dans la variable `PORT`. Si vous utilisez la CLI de Loco (`cargo loco start`), écrivez plutôt un `main` qui démarre directement le serveur, comme dans l'exemple :

```rust
#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    let environment = Environment::from(resolve_from_env());
    let config = App::load_config(&environment).await?;
    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(config.server.port);
    let binding = config.server.binding.clone();
    let boot = App::boot(StartMode::ServerOnly, &environment, config).await?;
    loco_rs::boot::start::<App>(boot, ServeParams { port, binding }, true).await
}
```
