# moltn guide

moltn lets you build a web application whose **server is written in Rust** (axum) and whose **interface uses Vue, React or Svelte**, connected by [Inertia](https://inertiajs.com): no JSON API to design, the server picks a page and hands it its data.

What moltn adds:

- **Server-side rendering inside the binary.** The first visit gets fully rendered HTML (SEO, instant display) without a Node server on the side: a JavaScript engine is embedded in the application.
- **File-based routing.** `web/src/pages/users/[id].vue` answers `/users/42`. A page's server logic is a Rust function annotated with `#[page]`.
- **End-to-end types.** Rust structs become TypeScript types: renaming a field in Rust makes the frontend type check fail.
- **One command for everything.** `moltn new`, `moltn dev`, `moltn build`.

In production, everything fits in **one executable**: server, pages, assets, server-side rendering. Node is only needed to develop and to build the frontend.

## Contents

1. [Getting started](getting-started.md): install, create a project, run it.
2. [Pages and loaders](pages.md): file-based routing, server data, forms.
3. [Types and routes](types.md): typed props, `route()`, shared props, `moltn gen`.
4. [Server-side rendering](server-rendering.md): how it works, rules for page code, settings.
5. [Commands](cli.md): CLI reference.
6. [Configuration](configuration.md): the optional `moltn.toml` file.
7. [Production](production.md): build and deploy the binary.
8. [With Loco](loco.md): plug moltn into a Loco application.
9. [Troubleshooting](troubleshooting.md): common error messages and fixes.

> **Status**: moltn is in development (0.1) and not yet published on crates.io or npm. Projects created by `moltn new` depend on the moltn repository present on the machine.
