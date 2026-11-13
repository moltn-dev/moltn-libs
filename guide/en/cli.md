# Commands

Run the commands from the project root (the folder that contains `Cargo.toml`), or from any subfolder of a project that has a `moltn.toml`. The defaults below can be changed in that file: see [Configuration](configuration.md).

## `moltn new <name>`

Creates a project: Rust application, frontend, home page and "About" page.

| Option | Default | Purpose |
|---|---|---|
| `--front vue\|react\|svelte` | `vue` | frontend framework |
| `--engine quickjs\|v8` | `quickjs` | server-side rendering engine: V8 for `Intl` (locale-aware dates and numbers), at the cost of about 55 MB of binary. See [Server-side rendering](server-rendering.md) |

Installs the frontend dependencies (pnpm if present, otherwise npm), then compiles the application once.

## `moltn dev`

Starts Vite and the application; prints the address to open.

| Option | Default | Purpose |
|---|---|---|
| `--port` | `3000` | application port, the address to open (passed through the `PORT` variable) |
| `--vite-port` | `5173` | Vite port, used internally |
| `--web` | `web` | frontend folder |

- Frontend: hot reload by Vite.
- Rust code, `Cargo.toml`, `build.rs` (the list is configurable: `dev.watch`), adding or removing a page: automatic recompile and restart.
- Ctrl+C: stops everything, no orphan process.

## `moltn build`

Installs the frontend dependencies, builds the frontend (browser and server-side rendering), then `cargo build --release`. The binary is in `target/release/`. When the `CI` variable is set, installation refuses an out-of-date lockfile.

| Option | Default | Purpose |
|---|---|---|
| `--web` | `web` | frontend folder |

## `moltn gen`

Regenerates `web/src/generated/moltn.ts` (props types and routes) by compiling and briefly running the application.

| Option | Purpose |
|---|---|
| `--check` | changes nothing; fails if the file is out of date (continuous integration) |

## `moltn doctor`

Shows the effective configuration (`moltn.toml` or the defaults) and the detected JavaScript toolchain: runtime (Node, Bun, Deno), package manager, versions, and which file drove each choice (`package.json`, lockfile, `.nvmrc`…).

| Option | Default | Purpose |
|---|---|---|
| `--web` | `web` | frontend folder |
| `--json` | | JSON output |

moltn respects version managers: with a `.prototools` it goes through [proto](https://moonrepo.dev/proto), with a `mise.toml` or `.tool-versions` through [mise](https://mise.jdx.dev), and it uses [Volta](https://volta.sh) shims when they are on the `PATH`. If a version does not fit, the message gives the command to install the right one.
