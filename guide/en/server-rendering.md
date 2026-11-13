# Server-side rendering

## How it works

On the first visit to a page, the Rust application:

1. calls the page's loader;
2. has the page rendered to HTML by an **embedded JavaScript engine** (QuickJS or V8, see below) that runs your page code;
3. sends back a complete HTML page, which Vue, React or Svelte then take over in the browser (hydration).

Later navigations go through Inertia: only the data (JSON) travels.

No Node server is needed in production. Rendering a page takes around a millisecond.

In development, `moltn dev` has Vite render the pages, so hot reload keeps working.

## When rendering fails

An exception in a component, a render that takes too long, an out-of-memory: the page is sent **without server-side rendering**, and the browser displays it normally. Nothing breaks for the visitor; the error is logged on the server with the page name.

A `console.error` during rendering also counts as a failure: that is how Vue reports an error in a component.

## Rules for page code

Page code also runs on the server, in a JavaScript engine with no browser and no Node:

- **no `window`, `document`, `localStorage`** during rendering: use them in `onMounted` (Vue), `useEffect` (React) or `onMount` (Svelte);
- **no network request** during rendering: data arrives through the props, from the Rust loader;
- **no Node module** (`fs`, `path`, `stream`, `crypto`…), nor any library that depends on one. The build fails with a message naming the offending file or package.
- **no per-visitor state in a module-level variable** (a `ref` declared outside a component, a global i18n instance): the server keeps the same engine from one request to the next, so one visitor's value would be served to the next. Reset that state at the start of every render from the props, for instance in `withApp` of `createInertiaApp` (language, signed-in user).
- **with QuickJS, no `Intl` and no locale-aware formatting**: `Intl.DateTimeFormat` does not exist (the page is sent without server-side rendering) and `toLocaleDateString(locale, options)` ignores the locale (the server writes `10/31/2010`, the browser corrects it to `31 Oct 2010`). The V8 engine has no such limit: see "Choosing the engine".

`URL`, `URLSearchParams` and `TextEncoder` are available.

`setTimeout` and `setInterval` exist but **never fire** on the server: rendering stops as soon as the HTML is produced. A library that sets a timer therefore works; a render that *waits* for one (`await` on a `setTimeout`) fails and the page is sent without server-side rendering.

## Choosing the engine

Two JavaScript engines are available, selected in `Cargo.toml`:

| | QuickJS (default) | V8 |
|---|---|---|
| Size added to the binary | about 1 MB | about 55 MB |
| `Intl`, locale-aware dates and numbers | **no** | yes, like the browser |
| Rendering a page | around 1 ms | around 0.2 ms |
| Platforms | all | those V8 is prebuilt for |

Pick **V8** as soon as your pages show locale-aware dates or numbers (`Intl.DateTimeFormat`, `toLocaleDateString`, `toLocaleString`): with QuickJS those calls fail or ignore the locale.

```toml
moltn-ssr = { version = "0.1", default-features = false, features = ["veer", "v8"] }
```

A new project gets it from the start with `moltn new my-app --engine v8`. Nothing else to change: with the `v8` feature, `EmbeddedSsr::builder(…)` uses V8. To choose explicitly: `.engine(moltn_ssr::Engine::V8)`.

## Settings

The engine is configured when it is created, in `main.rs`:

```rust
use std::time::Duration;

let ssr = moltn_ssr::EmbeddedSsr::builder(moltn_ssr::embedded_bundle!())
    .workers(4)                                   // engines in parallel (default: half the cores)
    .render_timeout(Duration::from_millis(500))   // maximum render time (default: 500 ms)
    .memory_limit_mb(64)                          // maximum memory per engine (default: 64 MB)
    .build()?;
```

When every engine is busy and the queue is full, the page is sent without server-side rendering instead of waiting.

## Metrics

moltn publishes three metrics through the [`metrics`](https://docs.rs/metrics) crate, to plug into the exporter of your choice (Prometheus…):

- `moltn_ssr_render_seconds`: duration of successful renders;
- `moltn_ssr_fallback_total{reason}`: failed renders, by cause;
- `moltn_ssr_rejected_total`: renders refused for lack of capacity.
