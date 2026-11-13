# With Loco

[Loco](https://loco.rs) is a Rust framework built on axum. moltn plugs into it without an adapter: moltn pages join Loco's router, Loco controllers stay as they are, and loaders read Loco's `AppContext`.

A complete example lives in the repository: `examples/loco-vue`.

## Dependencies

On top of a moltn project's (`moltn-pages`, `moltn-ssr`, `veer`, `ts-rs`, `rust-embed`):

```toml
[dependencies]
loco-rs = { version = "1.3", default-features = false }   # add the Loco features you need
async-trait = "0.1"
```

And the same `build.rs` as a moltn project (`moltn_pages::build::scan_project()`).

## Wiring moltn into `Hooks`

```rust
#[async_trait]
impl Hooks for App {
    // moltn pages become Loco's starting router.
    async fn before_routes(_ctx: &AppContext) -> Result<axum::Router<AppContext>> {
        moltn_pages::router::<AppContext>(moltn_pages::pages!())
            .map_err(|e| loco_rs::Error::string(&e.to_string()))
    }

    // Loco controllers, unchanged.
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes().add_route(controllers::posts::routes())
    }

    // Inertia protocol and frontend assets, around every route.
    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {
        let inertia = inertia_config()?;   // as in a moltn project's main.rs
        Ok(router
            .nest_service("/build", EmbeddedAssets::new(|p| Assets::get(p).map(|f| f.data)))
            .layer(InertiaLayer::new(inertia)))
    }

    // … app_name, boot, connect_workers, register_tasks as usual
}
```

## Loaders that read Loco's state

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

## With `moltn dev`

`moltn dev` starts the application with `cargo run` and passes its port in the `PORT` variable. If you use Loco's CLI (`cargo loco start`), write a `main` that starts the server directly instead, as in the example:

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
