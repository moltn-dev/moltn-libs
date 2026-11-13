//! Server-side rendering of Inertia v3 pages (Vue, React, Svelte) **inside the Rust process**,
//! with an embedded JavaScript engine (QuickJS or V8, see [`Engine`]) and no Node server on
//! the side.
//!
//! The SSR bundle is an ES module without imports that defines, when evaluated:
//!
//! ```ts
//! globalThis.__moltnRender = (page) => Promise<{ head: string[]; body: string }>
//! ```
//!
//! [`EmbeddedSsr`] loads this bundle into a pool of workers (one OS thread and one engine
//! each), enforces a timeout and a memory limit per render, and recreates an engine
//! after a fatal error. Every error is returned to the caller, which falls back to
//! client-side rendering: with the `veer` feature (on by default), `EmbeddedSsr` implements
//! `veer::SsrClient` and `veer` performs that fallback itself.
//!
//! ```
//! use moltn_ssr::{Bundle, EmbeddedSsr};
//!
//! # #[tokio::main] async fn main() -> Result<(), moltn_ssr::SsrError> {
//! let bundle = Bundle::from_static(r#"
//!     globalThis.__moltnRender = async (page) => ({ head: [], body: `<p>${page.component}</p>` })
//! "#);
//! let ssr = EmbeddedSsr::builder(bundle).workers(2).build()?;
//!
//! let out = ssr.render(&serde_json::json!({ "component": "index", "props": {} })).await?;
//! assert_eq!(out.body, "<p>index</p>");
//! # Ok(()) }
//! ```
//!
//! # Metrics
//!
//! Emitted through the [`metrics`] facade (no exporter imposed):
//! - `moltn_ssr_render_seconds` (histogram): duration of successful renders;
//! - `moltn_ssr_fallback_total{reason}`: failed renders, hence client-side fallbacks;
//! - `moltn_ssr_rejected_total`: renders refused because the queue was full.

mod engine;
mod pool;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::EngineLimits;
use pool::{Pool, PoolConfig};

#[cfg(not(any(feature = "quickjs", feature = "v8")))]
compile_error!(
    "moltn-ssr needs a JavaScript engine: enable the `quickjs` (default) or `v8` feature"
);

/// JavaScript engine that runs the bundle. Each variant exists only with its Cargo feature.
///
/// | | QuickJS | V8 |
/// |---|---|---|
/// | Feature | `quickjs` (default) | `v8` |
/// | Binary size | about 1 MB | tens of MB (ICU data included) |
/// | `Intl` (dates, numbers, plurals by locale) | **absent** | complete |
/// | Targets | everywhere | V8's prebuilt targets |
///
/// Pick V8 as soon as pages format dates or numbers with `Intl` or `toLocaleString`:
/// under QuickJS those calls fail or ignore the locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Engine {
    /// QuickJS: small and portable, without `Intl`.
    #[cfg(feature = "quickjs")]
    QuickJs,
    /// V8: full `Intl`, higher throughput, larger binary.
    #[cfg(feature = "v8")]
    V8,
}

impl Default for Engine {
    /// V8 when the `v8` feature is enabled, QuickJS otherwise.
    fn default() -> Self {
        #[cfg(feature = "v8")]
        return Self::V8;
        #[cfg(not(feature = "v8"))]
        Self::QuickJs
    }
}

/// SSR engine errors. All of them mean "fall back to client-side rendering".
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SsrError {
    /// The bundle file could not be read.
    #[error("cannot read the SSR bundle ({}): run `moltn build` or the Vite SSR build ({source})", path.display())]
    BundleRead {
        /// Expected bundle path.
        path: PathBuf,
        /// I/O error.
        source: std::io::Error,
    },
    /// The bundle could not be evaluated (syntax, exception while loading…).
    #[error("loading the SSR bundle: {0}")]
    Load(String),
    /// The bundle evaluates but does not define `globalThis.__moltnRender`.
    #[error(
        "the SSR bundle does not define `globalThis.__moltnRender`: was it built with @moltn/vite?"
    )]
    MissingRender,
    /// JavaScript exception (or `console.error`) during the render.
    #[error("JavaScript error during render: {0}")]
    Js(String),
    /// The render exceeded the timeout; the engine is recreated.
    #[error("render interrupted after {0:?}")]
    Timeout(Duration),
    /// The engine reached its memory limit; it is recreated.
    #[error("JS engine memory limit reached")]
    OutOfMemory,
    /// The engine panicked; it is recreated.
    #[error("panic in the JS engine")]
    Panic,
    /// `__moltnRender` did not return `{ head: string[], body: string }`.
    #[error("invalid render result: {0}")]
    InvalidOutput(String),
    /// Every worker is busy and the queue is full.
    #[error("SSR render queue full")]
    QueueFull,
    /// No worker responds anymore.
    #[error("no SSR worker available")]
    WorkerUnavailable,
}

impl SsrError {
    /// Short, stable label used in metrics and logs.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::BundleRead { .. } => "bundle_read",
            Self::Load(_) => "load",
            Self::MissingRender => "missing_render",
            Self::Js(_) => "js",
            Self::Timeout(_) => "timeout",
            Self::OutOfMemory => "out_of_memory",
            Self::Panic => "panic",
            Self::InvalidOutput(_) => "invalid_output",
            Self::QueueFull => "queue_full",
            Self::WorkerUnavailable => "worker_unavailable",
        }
    }

    /// The JS context is in an unknown state: it must not be reused.
    fn requires_restart(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::OutOfMemory | Self::Panic)
    }
}

/// HTML produced by a render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SsrOutput {
    /// Elements to insert into `<head>`.
    pub head: Vec<String>,
    /// `<body>` content: `<script data-page>` and the rendered root element.
    pub body: String,
}

/// Source of the SSR bundle. Usually obtained with [`embedded_bundle!`].
#[derive(Debug, Clone)]
pub struct Bundle(Source);

#[derive(Debug, Clone)]
enum Source {
    Static(&'static str),
    Owned(Arc<str>),
    File(PathBuf),
}

impl Bundle {
    /// Bundle included in the binary (`include_str!`).
    ///
    /// ```
    /// let bundle = moltn_ssr::Bundle::from_static("globalThis.__moltnRender = async () => ({ head: [], body: '' })");
    /// # let _ = bundle;
    /// ```
    pub const fn from_static(source: &'static str) -> Self {
        Self(Source::Static(source))
    }

    /// Bundle already in memory.
    ///
    /// ```
    /// let bundle = moltn_ssr::Bundle::from_string(String::from("globalThis.__moltnRender = async () => ({ head: [], body: '' })"));
    /// # let _ = bundle;
    /// ```
    pub fn from_string(source: impl Into<String>) -> Self {
        Self(Source::Owned(Arc::from(source.into())))
    }

    /// Bundle read from disk at startup (`EmbeddedSsrBuilder::build`).
    ///
    /// ```
    /// let bundle = moltn_ssr::Bundle::from_file("web/dist/ssr/ssr.js");
    /// # let _ = bundle;
    /// ```
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self(Source::File(path.into()))
    }

    fn read(&self) -> Result<Arc<str>, SsrError> {
        match &self.0 {
            Source::Static(s) => Ok(Arc::from(*s)),
            Source::Owned(s) => Ok(s.clone()),
            Source::File(path) => std::fs::read_to_string(path)
                .map(Arc::from)
                .map_err(|source| SsrError::BundleRead {
                    path: path.clone(),
                    source,
                }),
        }
    }
}

/// The application's SSR bundle, relative to the calling crate's `Cargo.toml`
/// (default: `web/dist/ssr/ssr.js`).
///
/// - In **debug**: read from disk at startup, to iterate without recompiling Rust.
/// - In **release**: included in the binary. If the file does not exist, compilation
///   fails naming the expected path: run `moltn build` (or the Vite SSR build).
///
/// ```no_run
/// let default = moltn_ssr::embedded_bundle!();
/// let custom = moltn_ssr::embedded_bundle!("front/dist/ssr/ssr.js");
/// # let _ = (default, custom);
/// ```
#[macro_export]
macro_rules! embedded_bundle {
    () => {
        $crate::embedded_bundle!("web/dist/ssr/ssr.js")
    };
    ($path:literal) => {{
        #[cfg(debug_assertions)]
        let bundle = $crate::Bundle::from_file(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path));
        #[cfg(not(debug_assertions))]
        let bundle = $crate::Bundle::from_static(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/",
            $path
        )));
        bundle
    }};
}

/// Embedded SSR engine: a pool of workers, each with its own JavaScript engine running the bundle.
#[derive(Debug)]
pub struct EmbeddedSsr {
    pool: Pool,
}

impl EmbeddedSsr {
    /// Start configuring an engine for this bundle.
    ///
    /// ```
    /// # fn main() -> Result<(), moltn_ssr::SsrError> {
    /// use std::time::Duration;
    /// # let bundle = moltn_ssr::Bundle::from_static("globalThis.__moltnRender = async () => ({ head: [], body: '' })");
    /// let ssr = moltn_ssr::EmbeddedSsr::builder(bundle)
    ///     .workers(4)
    ///     .render_timeout(Duration::from_millis(500))
    ///     .memory_limit_mb(64)
    ///     .build()?;
    /// # let _ = ssr; Ok(()) }
    /// ```
    pub fn builder(bundle: Bundle) -> EmbeddedSsrBuilder {
        let workers = std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1));
        EmbeddedSsrBuilder {
            bundle,
            engine: Engine::default(),
            workers,
            queue_capacity: None,
            timeout: Duration::from_millis(500),
            memory_limit: 64 * 1024 * 1024,
        }
    }

    /// Render an Inertia page object. Never blocks the async runtime: rendering runs
    /// on a dedicated worker, and a full queue returns [`SsrError::QueueFull`] immediately.
    /// Dropping the returned future cancels a render that is still waiting in the queue.
    ///
    /// ```
    /// # #[tokio::main] async fn main() -> Result<(), moltn_ssr::SsrError> {
    /// # let bundle = moltn_ssr::Bundle::from_static("globalThis.__moltnRender = async (p) => ({ head: [], body: p.url })");
    /// # let ssr = moltn_ssr::EmbeddedSsr::builder(bundle).workers(1).build()?;
    /// let page = serde_json::json!({ "component": "users/[id]", "props": {}, "url": "/users/42" });
    /// let out = ssr.render(&page).await?;
    /// assert_eq!(out.body, "/users/42");
    /// # Ok(()) }
    /// ```
    pub async fn render(&self, page: &serde_json::Value) -> Result<SsrOutput, SsrError> {
        let start = Instant::now();
        let result = self.pool.render(page.to_string()).await;
        match &result {
            Ok(_) => metrics::histogram!("moltn_ssr_render_seconds")
                .record(start.elapsed().as_secs_f64()),
            Err(e) => {
                let component = page
                    .get("component")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                tracing::warn!(component, reason = e.reason(), error = %e, "SSR failed, falling back to client-side rendering");
                metrics::counter!("moltn_ssr_fallback_total", "reason" => e.reason()).increment(1);
                if matches!(e, SsrError::QueueFull) {
                    metrics::counter!("moltn_ssr_rejected_total").increment(1);
                }
            }
        }
        result
    }
}

/// Configuration of an [`EmbeddedSsr`].
#[derive(Debug, Clone)]
#[must_use]
pub struct EmbeddedSsrBuilder {
    bundle: Bundle,
    engine: Engine,
    workers: usize,
    queue_capacity: Option<usize>,
    timeout: Duration,
    memory_limit: usize,
}

impl EmbeddedSsrBuilder {
    /// JavaScript engine. Default: V8 when the `v8` feature is enabled, QuickJS otherwise.
    ///
    /// ```
    /// # fn main() -> Result<(), moltn_ssr::SsrError> {
    /// # let bundle = moltn_ssr::Bundle::from_static("globalThis.__moltnRender = async () => ({ head: [], body: '' })");
    /// let ssr = moltn_ssr::EmbeddedSsr::builder(bundle)
    ///     .engine(moltn_ssr::Engine::default())
    ///     .workers(1)
    ///     .build()?;
    /// # let _ = ssr; Ok(()) }
    /// ```
    pub fn engine(mut self, engine: Engine) -> Self {
        self.engine = engine;
        self
    }

    /// Number of workers (threads and engines). Default: `max(1, cores / 2)`.
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers.max(1);
        self
    }

    /// Pending renders beyond which new ones are refused (client fallback). Default: `8 × workers`.
    pub fn queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = Some(capacity.max(1));
        self
    }

    /// Maximum duration of a render. Default: 500 ms.
    pub fn render_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Maximum memory of each engine, in MB. Default: 64.
    pub fn memory_limit_mb(mut self, megabytes: usize) -> Self {
        self.memory_limit = megabytes.saturating_mul(1024 * 1024);
        self
    }

    /// Read the bundle and load it into every worker. **Blocking** (a few tens of
    /// ms): call it at startup. Fails early if the bundle is missing or invalid.
    pub fn build(self) -> Result<EmbeddedSsr, SsrError> {
        let bundle = self.bundle.read()?;
        let config = PoolConfig {
            workers: self.workers,
            queue_capacity: self.queue_capacity.unwrap_or(self.workers * 8),
            timeout: self.timeout,
            limits: EngineLimits {
                memory_limit: self.memory_limit,
            },
        };
        let pool = match self.engine {
            #[cfg(feature = "quickjs")]
            Engine::QuickJs => Pool::start::<engine::quickjs::QuickJs>(bundle, config)?,
            #[cfg(feature = "v8")]
            Engine::V8 => Pool::start::<engine::v8::V8>(bundle, config)?,
        };
        Ok(EmbeddedSsr { pool })
    }
}

#[cfg(feature = "veer")]
#[async_trait::async_trait]
impl veer::SsrClient for EmbeddedSsr {
    async fn render(
        &self,
        page: &serde_json::Value,
    ) -> Result<veer::SsrPayload, veer::ssr::SsrError> {
        let out = EmbeddedSsr::render(self, page)
            .await
            .map_err(|e| veer::ssr::SsrError::Transport(e.to_string()))?;
        Ok(veer::SsrPayload {
            head: out.head,
            body: out.body,
        })
    }
}
