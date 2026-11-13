//! Detect, resolve and drive a project's JavaScript toolchain (Node, Bun, Deno;
//! npm, pnpm, Yarn, bun, deno) from Rust. A **development and build** tool:
//! an application in production never uses it.
//!
//! Three steps:
//! 1. [`JsProject::detect`] reads the project (`package.json`, lockfiles, `.nvmrc`,
//!    `.prototools`, `mise.toml`…) without starting any process;
//! 2. [`JsProject::resolve`] finds the binaries (through proto, mise, Volta or the `PATH`) and
//!    checks their versions, with an error that says what to install;
//! 3. [`Toolchain`] runs `install`, scripts, and supervised dev processes
//!    ([`DevProcess`]) that leave no child process behind.
//!
//! ```no_run
//! use moltn_toolchain::{InstallOpts, JsProject, ReadyProbe};
//! use std::time::Duration;
//!
//! # async fn run() -> Result<(), moltn_toolchain::ToolchainError> {
//! let project = JsProject::detect("web")?;
//! let tc = project.resolve()?;
//! tc.install(InstallOpts { frozen_lockfile: false }).await?;
//! tc.run_script("build", &[]).await?;
//!
//! let mut vite = tc.spawn_script("dev", &["--port", "5173"])?;
//! vite.wait_ready(ReadyProbe::Tcp("localhost:5173".into()), Duration::from_secs(30)).await?;
//! // `vite` and its children are killed when it is dropped.
//! # Ok(()) }
//! ```

mod detect;
mod process;
mod requirement;
mod resolve;

use std::path::{Path, PathBuf};
use std::time::Duration;

pub use detect::{DetectedFrom, PackageManager, Runtime, ToolchainSpec};
pub use process::{DevProcess, OutputFn, ReadyProbe};
pub use requirement::Requirement;
pub use resolve::{Description, InstallOpts, ResolvedTool, Toolchain, VersionManager};

/// Toolchain errors; each message says what to do.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolchainError {
    /// The folder is not a frontend project.
    #[error("no package.json in {}: is this the frontend folder?", .0.display())]
    NoPackageJson(PathBuf),
    /// A file could not be read.
    #[error("reading {}: {source}", path.display())]
    Io {
        /// File.
        path: PathBuf,
        /// I/O error.
        source: std::io::Error,
    },
    /// Unreadable configuration file (JSON, TOML).
    #[error("invalid {}: {message}", path.display())]
    InvalidFile {
        /// File.
        path: PathBuf,
        /// Parse error.
        message: String,
    },
    /// Tool not found.
    #[error("{tool}{} not found.\n  → {hint}", required.as_ref().map(|r| format!(" {r}")).unwrap_or_default())]
    ToolNotFound {
        /// Tool (`node`, `pnpm`…).
        tool: String,
        /// Required version.
        required: Option<String>,
        /// Command or link to install it.
        hint: String,
    },
    /// Installed version does not match.
    #[error("{tool} {required} required, {found} found.\n  → {hint}")]
    VersionMismatch {
        /// Tool.
        tool: String,
        /// Required version.
        required: String,
        /// Version found.
        found: semver::Version,
        /// Command to install the right version.
        hint: String,
    },
    /// `<tool> --version` did not return a readable version.
    #[error("unreadable {tool} version: {output:?}")]
    UnreadableVersion {
        /// Tool.
        tool: String,
        /// Output received.
        output: String,
    },
    /// The process could not be started.
    #[error("starting `{command}`: {source}")]
    Spawn {
        /// Command.
        command: String,
        /// System error.
        source: std::io::Error,
    },
    /// The command failed.
    #[error("`{command}` failed ({}):\n{tail}", code.map_or("killed".into(), |c| format!("code {c}")))]
    CommandFailed {
        /// Command.
        command: String,
        /// Exit code.
        code: Option<i32>,
        /// Last lines of output.
        tail: String,
    },
    /// pnpm refuses to install while install scripts are neither allowed nor denied.
    #[error(
        "pnpm blocked the install: unapproved install scripts ({}).\n  → add to pnpm-workspace.yaml (at the pnpm project root):\n    allowBuilds:\n{}",
        packages.join(", "),
        packages.iter().map(|p| format!("      {p}: false   # true if its script is needed")).collect::<Vec<_>>().join("\n")
    )]
    IgnoredBuilds {
        /// Packages involved.
        packages: Vec<String>,
    },
    /// The dev process does not respond.
    #[error("{label} not responding after {waited:?}")]
    NotReady {
        /// Process.
        label: String,
        /// Time waited.
        waited: Duration,
    },
    /// The dev process stopped before being ready.
    #[error("{label} stopped ({status}):\n{tail}")]
    ExitedEarly {
        /// Process.
        label: String,
        /// Exit status.
        status: std::process::ExitStatus,
        /// Last lines of output.
        tail: String,
    },
}

impl ToolchainError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// A detected frontend project, ready to be resolved.
#[derive(Debug, Clone)]
pub struct JsProject {
    dir: PathBuf,
    spec: ToolchainSpec,
}

impl JsProject {
    /// Detect the toolchain expected by the frontend project `dir` (reads files only).
    ///
    /// ```no_run
    /// let project = moltn_toolchain::JsProject::detect("web")?;
    /// println!("{} with {}", project.spec().runtime, project.spec().package_manager);
    /// # Ok::<(), moltn_toolchain::ToolchainError>(())
    /// ```
    pub fn detect(dir: impl AsRef<Path>) -> Result<Self, ToolchainError> {
        let dir = dir.as_ref().to_path_buf();
        let spec = detect::detect(&dir)?;
        Ok(Self { dir, spec })
    }

    /// What was detected.
    pub fn spec(&self) -> &ToolchainSpec {
        &self.spec
    }

    /// Frontend project folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Force the package manager (explicit configuration, takes precedence over detection).
    ///
    /// ```no_run
    /// use moltn_toolchain::{JsProject, PackageManager, Requirement};
    /// let project = JsProject::detect("web")?.with_package_manager(PackageManager::Pnpm, Some(Requirement::parse("10")));
    /// # Ok::<(), moltn_toolchain::ToolchainError>(())
    /// ```
    #[must_use]
    pub fn with_package_manager(mut self, pm: PackageManager, req: Option<Requirement>) -> Self {
        self.spec.package_manager = pm;
        self.spec.pm_req = req;
        self.spec.sources.push(DetectedFrom {
            decision: format!("package manager: {pm}"),
            source: "explicit configuration".into(),
            path: None,
        });
        self
    }

    /// Force the runtime and its version (explicit configuration).
    ///
    /// ```no_run
    /// use moltn_toolchain::{JsProject, Requirement, Runtime};
    /// let project = JsProject::detect("web")?.with_runtime(Runtime::Node, Some(Requirement::parse(">=22")));
    /// # Ok::<(), moltn_toolchain::ToolchainError>(())
    /// ```
    #[must_use]
    pub fn with_runtime(mut self, runtime: Runtime, req: Option<Requirement>) -> Self {
        self.spec.runtime = runtime;
        self.spec.runtime_req = req;
        self.spec.sources.push(DetectedFrom {
            decision: format!("runtime: {runtime}"),
            source: "explicit configuration".into(),
            path: None,
        });
        self
    }

    /// Find the binaries on the `PATH` (directly or through proto, mise, Volta) and check
    /// their versions. **Blocking**: runs `<tool> --version`. From async code,
    /// call it inside `tokio::task::spawn_blocking`.
    pub fn resolve(&self) -> Result<Toolchain, ToolchainError> {
        resolve::resolve(self, None)
    }

    /// Like [`Self::resolve`], with a given `PATH` (tests, isolated environments).
    pub fn resolve_with_path(
        &self,
        path: impl Into<std::ffi::OsString>,
    ) -> Result<Toolchain, ToolchainError> {
        resolve::resolve(self, Some(path.into()))
    }
}
