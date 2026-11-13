# Getting started

## Requirements

- **Rust** 1.99 or newer ([rustup](https://rustup.rs)).
- **Node.js** 22 or newer, to develop and build the frontend.
- **pnpm** (recommended) or npm. If pnpm is installed, `moltn new` uses it.

To see what moltn finds on your machine, run `moltn doctor` inside a project.

## Install the `moltn` command

From the moltn repository:

```bash
cargo install --path crates/moltn-cli
```

The command goes to `~/.cargo/bin`, which is already on your `PATH` if you use rustup.

## Create a project

```bash
moltn new my-app             # Vue by default
moltn new my-app --front react
moltn new my-app --front svelte
```

`moltn new` writes the project, installs the frontend dependencies, then compiles the application once (one to a few minutes depending on the machine). When it is done, the project is ready:

```bash
cd my-app
moltn dev
```

Open the address moltn prints:

```
  ➜ moltn  http://127.0.0.1:3000   (Ctrl+C to stop)
```

## What a project contains

```
my-app/
  Cargo.toml
  build.rs                  # reads web/src/pages/ at compile time
  src/main.rs               # server: #[page] loaders, configuration
  web/
    package.json
    vite.config.ts          # plugins: vue() | react() | svelte(), inertia(), moltn()
    src/
      app.ts                # createInertiaApp()
      pages/
        index.vue           # → /
        about.vue           # → /about
      generated/
        moltn.ts            # generated: props types and route()
```

## The development loop

While `moltn dev` runs:

- edit a frontend file (`.vue`, `.tsx`, `.svelte`, CSS): the page updates **without reloading**;
- edit Rust code, `Cargo.toml`, or **add / remove a page**: moltn recompiles and restarts the application on its own; `web/src/generated/moltn.ts` is updated on restart;
- on a compile error, moltn waits for the next change;
- **Ctrl+C** stops everything and leaves no process behind.

## Next

- Add pages and their data: [Pages and loaders](pages.md).
- Type props and links: [Types and routes](types.md).
- Build the final binary: [Production](production.md).
