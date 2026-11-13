# moltn-pages

File-based routing for Inertia on axum, with TypeScript types generated from Rust.

- `web/src/pages/users/[id].vue` answers `/users/{id}`.
- A page's server logic is an ordinary axum handler annotated with `#[page("users/[id]")]`, returning `Result<Props, E>`.
- Pages and loaders are checked at startup; every problem is reported at once.
- `moltn.ts` (props types via ts-rs, `Pages`, typed `route()`) is kept up to date by the application.

```rust
#[moltn_pages::page("users/[id]")]
async fn user_show(Path(id): Path<u32>) -> Result<UserProps, StatusCode> { /* … */ }

let app = moltn_pages::router::<AppState>(moltn_pages::pages!())?;
```

## Part of moltn

Rust + Inertia v3 (Vue, React, Svelte) with server-side rendering inside the binary.
Guide (English and French): the `guide/` folder of the moltn repository.

## License

MIT OR Apache-2.0.
