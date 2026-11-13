# moltn

**Rust + Inertia, with server-side rendering, shipped as a single binary.**

moltn is the tooling that makes a Rust backend (axum, Loco) and a Vue, React or Svelte frontend feel like one project, connected by [Inertia v3](https://inertiajs.com). Think Laravel + Inertia, in Rust, with one thing Laravel does not have: **no Node server in production**. Pages are rendered on the server by a JavaScript engine embedded in your binary.

📖 **Documentation: <https://docs.moltn.eu>** ([Français](https://docs.moltn.eu/fr/))

```bash
moltn new my-app          # Rust app + Vue (or --front react | svelte)
cd my-app
moltn dev                 # Vite with hot reload + the Rust app, restarted on change
moltn build               # one executable: server, pages, assets, server-side rendering
```

## What you get

- **Server-side rendering inside the binary.** The first visit gets fully rendered HTML. Two engines: QuickJS (default, about 1 MB) or V8 (`--engine v8`: full `Intl`, several times faster, about 55 MB).
- **File-based routing.** `web/src/pages/users/[id].vue` answers `/users/42`. Its data comes from an ordinary axum handler annotated with `#[page]`.
- **Types from Rust to TypeScript.** Props structs become TypeScript types, routes become a typed `route()` helper. Rename a field in Rust and the frontend type check fails.
- **One command for the JavaScript side.** moltn detects Node, Bun or Deno and npm, pnpm, Yarn or bun, and drives them; it works with proto, mise and Volta.

```rust
#[derive(Serialize, TS)]
struct UserProps { user: User }

#[moltn_pages::page("users/[id]")]
async fn user_show(Path(id): Path<u32>, State(app): State<AppState>) -> Result<UserProps, AppError> {
    Ok(UserProps { user: app.users.find(id).await? })
}
```

```vue
<script setup lang="ts">
import type { Pages } from '../../generated/moltn'
defineProps<Pages['users/[id]']>()
</script>
```

## Status

Version 0.1, **not yet published** on crates.io or npm: projects created by `moltn new` depend on a local checkout of this repository. Developed and tested on Windows; Linux and macOS are not yet covered by CI. The Inertia protocol itself is provided by [`veer`](https://crates.io/crates/veer).

## What is in this repository

| Path | What |
|---|---|
| `crates/moltn-ssr` | Embedded server-side rendering (QuickJS or V8), implements `veer::SsrClient` |
| `crates/moltn-pages`, `crates/moltn-pages-macros` | File-based routing, `#[page]` loaders, TypeScript generation, `moltn.toml` |
| `crates/moltn-toolchain` | Detects and drives the JavaScript toolchain |
| `crates/moltn-cli` | The `moltn` command |
| `packages/vite` | `@moltn/vite`, the Vite plugin that builds the server-side rendering bundle |
| `examples/` | axum with Vue, React and Svelte; Loco with Vue |
| `guide/` | User guide, English and French (the documentation site) |
| `docs/` | Design notes and roadmap (in French) |

## Building from source

Requires Rust 1.99 and Node 22 or later with pnpm.

```bash
pnpm install
pnpm -r build                              # npm package, example frontends, guide
cargo test --workspace --all-features      # after `pnpm -r build`
cargo install --path crates/moltn-cli          # installs the `moltn` command
```

## License

MIT OR Apache-2.0.
