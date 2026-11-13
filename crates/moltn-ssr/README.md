# moltn-ssr

Server-side rendering of Inertia v3 pages (Vue, React, Svelte) **inside the Rust process**, with an embedded JavaScript engine. No Node server in production.

Two engines, chosen with a Cargo feature: **QuickJS** (default: about 1 MB, builds everywhere, no `Intl`) and **V8** (`features = ["v8"]`: full `Intl`, several times faster, tens of MB).

- Worker pool on dedicated threads; never blocks the async runtime.
- Render timeout, memory limit, engine recreated after a fatal error.
- Any error falls back to client-side rendering.
- Implements `veer::SsrClient` (feature `veer`, on by default).

```rust
let ssr = moltn_ssr::EmbeddedSsr::builder(moltn_ssr::embedded_bundle!()).build()?;
let config = veer::InertiaConfig::new().ssr(ssr);
```

The bundle is produced by the [`@moltn/vite`](https://www.npmjs.com/package/@moltn/vite) plugin.

## Part of moltn

Rust + Inertia v3 (Vue, React, Svelte) with server-side rendering inside the binary.
Guide (English and French): <https://docs.moltn.eu>. Source: <https://github.com/moltn-dev/moltn-libs>.

## License

MIT OR Apache-2.0.
