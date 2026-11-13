# Troubleshooting

### `moltn`: command not found

The CLI is not installed, or `~/.cargo/bin` is not on the `PATH`:

```bash
cargo install --path crates/moltn   # from the moltn repository
```

After updating the moltn repository, run the same command again.

### `node 24 required, 20.11.0 found`

The installed Node version does not match what the project asks for (`.nvmrc`, `engines`…). The message gives the command to run (`proto install node 24`, `mise use node@24`, `volta install node@24`, or the download link). `moltn doctor` shows which file sets the version.

### `pnpm blocked the install: unapproved install scripts`

pnpm 11 requires install scripts to be explicitly allowed or denied. As the message says, add to `pnpm-workspace.yaml`:

```yaml
allowBuilds:
  package-name: false   # true if its script is needed
```

### `Node module "fs" is imported by …`

A page, or a library it uses, imports a Node module: it does not exist in the server-side rendering engine. Replace the library, or import it only in the browser (`onMounted`, `useEffect`, `onMount`). See [Server-side rendering](server-rendering.md).

### `invalid moltn pages: …`

Pages and loaders do not match; each line says what to fix. See [Pages and loaders](pages.md#startup-checks).

### `… produce the same route …` at compile time

Two files in `pages/` produce the same URL (for example `about.vue` and `about/index.vue`). Remove or rename one of them.

### `moltn.ts is out of date`

`moltn gen --check` found a file lagging behind the Rust code. Run `moltn gen` (or restart `moltn dev`) and commit the file.

### `Invalid module name in augmentation, module '@inertiajs/core' cannot be found`

Shared props require `@inertiajs/core` as a direct frontend dependency:

```bash
pnpm add -D @inertiajs/core
```

### `vue-tsc` or `svelte-check` crashes with TypeScript 7

These tools do not support TypeScript 7 yet: keep `"typescript": "~6.0.3"` in Vue and Svelte projects.

### The page shows, but without server-side rendering

Server-side rendering failed and the browser took over. The cause is in the application logs (`SSR failed, falling back to client-side rendering`, with the page name and the error). Most often: accessing `window` or `document` during rendering.

### "Frontend not built" although the frontend is built

In debug, assets are read from disk from a folder located **at compile time**. If the application was compiled before the first frontend build (seen when the folder is given with `..`, for instance `#[folder = "../web/dist/client/"]`), it does not find it afterwards. Recompile the crate that declares the assets (edit or touch its source file), then start again. `moltn dev` and `moltn build` are not affected.

### Port 3000 (or 5173) is already in use

`moltn dev` stops with `port 3000 is already in use`. Stop the other process, or change the ports:

```bash
moltn dev --port 3001 --vite-port 5174
```
