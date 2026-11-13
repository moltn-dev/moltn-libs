# Configuration

moltn works without configuration. An **optional** `moltn.toml` file declares another folder layout or tunes `moltn dev`. A project created by `moltn new` contains one, fully commented out.

## Keys

All of them are optional; these are the defaults:

```toml
[front]
dir = "web"                             # frontend folder, relative to moltn.toml

[pages]
dir = "src/pages"                       # relative to front.dir
typescript = "src/generated/moltn.ts"   # relative to front.dir

[dev]
port = 3000
vite_port = 5173
cargo = ["run"]                         # cargo arguments that run the application
watch = ["src", "Cargo.toml", "build.rs"]   # changes that restart it
```

- **`pages.typescript`**: without this key, the file follows the pages folder (`generated/moltn.ts` next to it).
- **`dev.cargo`**: in a workspace with several binaries, `["run", "-p", "my-app"]`. `moltn gen` uses the same command.
- **`dev.watch`**: files and folders, relative to `moltn.toml`. A folder stands for all its files: add your migrations, your templates, a neighbouring crate.
- A command-line option (`--port`, `--vite-port`, `--web`) wins over the file.
- An unknown key is an error that names it: a typo is never silently ignored.

`moltn doctor` prints the effective configuration.

## Where to put the file

Next to the application's `Cargo.toml`, or in a parent folder. The `moltn` commands look for it from the current folder upwards and move into its folder, so they can be run from any subfolder.

On the Rust side, `build.rs` reads it with `scan_project()`:

```rust
fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
```

## Example: server and frontend side by side

```
my-app/
├── Cargo.toml        # [workspace] members = ["server"]
├── moltn.toml
├── server/           # Rust crate: Cargo.toml, build.rs, src/, migrations/
└── web/              # frontend
```

```toml
# moltn.toml
[dev]
watch = ["server/src", "server/migrations", "server/Cargo.toml", "server/build.rs"]
```

The frontend is already in `web/` (the default). In `server/src/main.rs`, paths read at compile time are relative to the crate:

```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "../web/dist/client/"]
struct Assets;

let ssr = EmbeddedSsr::builder(moltn_ssr::embedded_bundle!("../web/dist/ssr/ssr.js")).build()?;
```

## What is not configured here

- Page naming conventions (`[id]`, `[[tab]]`, `[...slug]`, `(group)`, `_ignored`).
- Server-side rendering settings (number of engines, timeout, memory): they stay in the code, see [Server-side rendering](server-rendering.md).
- The frontend entry and the asset prefix: still written in `vite.config.ts` (options of `moltn()`) and in `main.rs`.
