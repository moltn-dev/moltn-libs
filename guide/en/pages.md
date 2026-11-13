# Pages and loaders

## One page, one file

Every file in `web/src/pages/` is a page. Its path gives the URL:

| File | URL | Note |
|---|---|---|
| `index.vue` | `/` | |
| `about.vue` | `/about` | |
| `users/index.vue` | `/users` | |
| `users/[id].vue` | `/users/42` | parameter `id` |
| `users/[[tab]].vue` | `/users` and `/users/profile` | optional parameter (last segment only) |
| `docs/[...slug].vue` | `/docs/guide/install` | one or more segments (last segment only) |
| `(admin)/stats.vue` | `/stats` | `(group)`: organizes files without changing the URL |
| `_components/Card.vue` | — | `_`: ignored, for internal components |

Recognized extensions: `.vue`, `.tsx`, `.jsx`, `.svelte`.

If two files produce the same URL (for example `about.vue` and `about/index.vue`), **compilation fails** and the message names both files.

## A page without data

A page without parameters needs no Rust code: the file is enough. It receives empty props.

## A loader: a page's data

The loader is the Rust function that prepares a page's data. It is an **ordinary axum handler** (same extractors: `Path`, `State`, `Query`, headers, authentication…) annotated with `#[page]`:

```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use moltn_pages::page;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
struct UserProps {
    user: User,
    can_edit: bool,
}

#[page("users/[id]")]
async fn user_show(
    Path(id): Path<u32>,
    State(app): State<AppState>,
) -> Result<UserProps, StatusCode> {
    let user = app.users.find(id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(UserProps { can_edit: false, user })
}
```

Rules:

- the string in `#[page("…")]` is the page name: its path in `pages/`, without extension;
- the function is `async` and returns `Result<Props, E>`:
  - `Props` derives `Serialize` and `TS` (see [Types and routes](types.md));
  - `E` is any axum response (`StatusCode`, a redirect, your error type…): it is what gets sent on error;
- a loader can live in any module of the crate.

On the frontend, the page receives the props:

```vue
<script setup lang="ts">
import type { Pages } from '../../generated/moltn'
defineProps<Pages['users/[id]']>()
</script>

<template>
  <h1>{{ user.name }}</h1>
</template>
```

## Application state

The state (database pool, configuration…) is axum's. moltn infers it from the loader's `State<T>` extractor. If a loader reads **part** of the state (`State<Db>` with `Db: FromRef<AppState>`), name the full state:

```rust
#[page("users/[id]", state = AppState)]
async fn user_show(State(db): State<Db>, Path(id): Path<u32>) -> Result<UserProps, AppError> { … }
```

## Wiring the router

In `main.rs`:

```rust
let app = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route("/users", post(create_user))     // regular axum routes alongside
    .layer(veer::InertiaLayer::new(inertia))
    .with_state(state);
```

And in `build.rs` (already present in a project created by `moltn new`):

```rust
fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
```

## Pages without server-side rendering

A page that only makes sense in the browser (video player, heavy editor) can skip server-side rendering; the browser renders it, as it does after an Inertia navigation:

```rust
#[page("watch/[id]", ssr = false)]
async fn watch(Path(id): Path<u32>) -> Result<WatchProps, AppError> { … }
```

## A "not found" page

moltn reserves no name for the 404 page. Create `pages/_404.vue` (the `_` keeps it out of the routes, but the frontend can still load it) and render it from a catch-all route, which only answers when no other route matches:

```rust
async fn not_found(inertia: veer::Inertia) -> impl IntoResponse {
    inertia.render("_404", serde_json::json!({})).status(StatusCode::NOT_FOUND)
}

let pages = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route("/{*path}", axum::routing::any(not_found));
```

## Protecting pages

A loader is an axum handler: an authentication extractor is enough, and its rejection (redirect to sign-in, 403) becomes the response. Mind the pages **without a loader**: they are served to everyone. To protect a whole set of pages, with or without loaders, put a middleware on the pages router:

```rust
let pages = moltn_pages::router::<AppState>(moltn_pages::pages!())?
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_session));
```

## Startup checks

`router()` checks that pages and loaders match, and lists **every** problem at once:

```
invalid moltn pages:
  - page posts/[slug].vue has parameters ([slug]) but no loader: add #[moltn_pages::page("posts/[slug]")] to a handler
  - loader orphan (src/main.rs:17) targets page "profil", which does not exist (expected file: pages/profil.vue, .tsx, .jsx or .svelte)
  - loader wrong_param (src/main.rs:23) extracts Path(slug) but users/[id].vue defines [id]
```


To catch these errors in continuous integration, add this test:

```rust
#[test]
fn pages_are_valid() {
    moltn_pages::assert_valid::<AppState>(moltn_pages::pages!());
}
```

## Forms and actions

Form submissions (POST, PUT, DELETE…) are **regular axum routes**, added next to the page router. With `veer`, the `InertiaForm` extractor reads the form and `redirect` goes back to a page:

```rust
async fn create_user(
    inertia: veer::Inertia,
    State(app): State<AppState>,
    veer::InertiaForm(form): veer::InertiaForm<NewUser>,
) -> veer::InertiaResponse {
    app.users.create(form).await;
    inertia.redirect("/users")
}
```

On the frontend, Inertia's `useForm` works as usual.
