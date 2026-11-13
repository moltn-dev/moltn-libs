//! Resolution: find the binaries and check their versions, then run the commands.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use semver::Version;
use serde::Serialize;

use crate::process::{self, DevProcess, OutputFn};
use crate::{JsProject, PackageManager, Requirement, Runtime, ToolchainError, ToolchainSpec};

/// Version manager that execution is delegated to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionManager {
    /// `proto exec <tools> -- <command>` (`.prototools` present).
    Proto,
    /// `mise exec -- <command>` (`mise.toml` or `.tool-versions` present).
    Mise,
    /// Volta shims on the `PATH`: direct execution, Volta picks the version.
    Volta,
    /// Binaries on the `PATH`.
    None,
}

/// A tool that was found.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedTool {
    /// Name (`node`, `pnpm`…).
    pub name: String,
    /// Path of the binary (or of the version manager that runs it).
    pub path: PathBuf,
    /// Installed version.
    pub version: Version,
}

/// Full toolchain state, for `moltn doctor` or any display tool.
#[derive(Debug, Clone, Serialize)]
pub struct Description {
    /// Frontend project folder.
    pub dir: PathBuf,
    /// What was detected.
    pub spec: ToolchainSpec,
    /// Version manager used.
    pub version_manager: VersionManager,
    /// Runtime found.
    pub runtime: ResolvedTool,
    /// Package manager found.
    pub package_manager: ResolvedTool,
}

/// Options of [`Toolchain::install`].
#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOpts {
    /// Fail if the lockfile is out of date (CI): `npm ci`, `--frozen-lockfile`, `--immutable`.
    pub frozen_lockfile: bool,
}

/// A resolved toolchain: knows how to run the project's package manager.
#[derive(Clone)]
pub struct Toolchain {
    dir: PathBuf,
    spec: ToolchainSpec,
    manager: VersionManager,
    /// `proto exec node pnpm --` or `mise exec --`, program path first.
    prefix: Vec<OsString>,
    path_env: Option<OsString>,
    runtime: ResolvedTool,
    package_manager: ResolvedTool,
    output: Option<OutputFn>,
}

impl std::fmt::Debug for Toolchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolchain")
            .field("dir", &self.dir)
            .field("manager", &self.manager)
            .finish_non_exhaustive()
    }
}

pub(crate) fn resolve(
    project: &JsProject,
    path_env: Option<OsString>,
) -> Result<Toolchain, ToolchainError> {
    let spec = project.spec.clone();
    let dir = project.dir.clone();
    let find = |name: &str| {
        which::which_in(
            name,
            path_env.clone().or_else(|| std::env::var_os("PATH")),
            &dir,
        )
        .ok()
    };
    let available = Available {
        proto: find("proto").is_some(),
        mise: find("mise").is_some(),
        volta: find("volta").is_some(),
    };

    let rt = spec.runtime.name();
    let pm = spec.package_manager.name();
    let (manager, prefix) = if let (Some(proto), Some(_)) = (find("proto"), &spec.prototools) {
        let mut prefix = vec![proto.into_os_string(), "exec".into(), rt.into()];
        if pm != rt {
            prefix.push(pm.into());
        }
        prefix.push("--".into());
        (VersionManager::Proto, prefix)
    } else if let (Some(mise), Some(_)) = (find("mise"), &spec.mise_config) {
        (
            VersionManager::Mise,
            vec![mise.into_os_string(), "exec".into(), "--".into()],
        )
    } else if available.volta {
        (VersionManager::Volta, Vec::new())
    } else {
        (VersionManager::None, Vec::new())
    };

    let probe = |tool: &str, req: &Option<Requirement>| -> Result<ResolvedTool, ToolchainError> {
        let hint = || install_hint(tool, req.as_ref(), manager, available);
        let (program, args): (PathBuf, Vec<OsString>) = match prefix.split_first() {
            Some((program, rest)) => {
                let mut args = rest.to_vec();
                args.push(tool.into());
                (program.into(), args)
            }
            None => {
                let path = find(tool).ok_or_else(|| ToolchainError::ToolNotFound {
                    tool: tool.into(),
                    required: req.as_ref().map(ToString::to_string),
                    hint: hint(),
                })?;
                (path, Vec::new())
            }
        };
        let mut command = std::process::Command::new(&program);
        command.args(&args).arg("--version").current_dir(&dir);
        if let Some(path) = &path_env {
            command.env("PATH", path);
        }
        let output = command.output().map_err(|_| ToolchainError::ToolNotFound {
            tool: tool.into(),
            required: req.as_ref().map(ToString::to_string),
            hint: hint(),
        })?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            return Err(ToolchainError::ToolNotFound {
                tool: tool.into(),
                required: req.as_ref().map(ToString::to_string),
                hint: hint(),
            });
        }
        let version = parse_version(&text).ok_or_else(|| ToolchainError::UnreadableVersion {
            tool: tool.into(),
            output: text.trim().to_string(),
        })?;
        if let Some(req) = req
            && req.matches(&version) == Some(false)
        {
            return Err(ToolchainError::VersionMismatch {
                tool: tool.into(),
                required: req.to_string(),
                found: version,
                hint: hint(),
            });
        }
        Ok(ResolvedTool {
            name: tool.into(),
            path: program,
            version,
        })
    };

    let runtime = probe(rt, &spec.runtime_req)?;
    let package_manager = if pm == rt {
        runtime.clone()
    } else {
        probe(pm, &spec.pm_req)?
    };
    Ok(Toolchain {
        dir,
        spec,
        manager,
        prefix,
        path_env,
        runtime,
        package_manager,
        output: None,
    })
}

/// First word of the output that looks like a version (`v24.1.0`, `deno 2.1.4 (…)`).
fn parse_version(text: &str) -> Option<Version> {
    text.split_whitespace()
        .find_map(|word| Version::parse(word.trim_start_matches('v')).ok())
}

#[derive(Clone, Copy)]
struct Available {
    proto: bool,
    mise: bool,
    volta: bool,
}

/// The exact command to run to get `tool` at version `req`.
fn install_hint(
    tool: &str,
    req: Option<&Requirement>,
    manager: VersionManager,
    available: Available,
) -> String {
    let version = req.map(Requirement::hint);
    let at = version
        .as_deref()
        .map(|v| format!("@{v}"))
        .unwrap_or_default();
    let space = version
        .as_deref()
        .map(|v| format!(" {v}"))
        .unwrap_or_default();
    let preferred = match manager {
        VersionManager::None if available.proto => VersionManager::Proto,
        VersionManager::None if available.mise => VersionManager::Mise,
        VersionManager::None if available.volta => VersionManager::Volta,
        other => other,
    };
    match preferred {
        VersionManager::Proto => format!("proto install {tool}{space}"),
        VersionManager::Mise => format!("mise use {tool}{at}"),
        VersionManager::Volta => format!("volta install {tool}{at}"),
        VersionManager::None => match tool {
            "node" => format!(
                "install Node{space}: https://nodejs.org/en/download (or with a version manager: proto, mise, Volta)"
            ),
            "bun" => "install Bun: https://bun.sh/docs/installation".into(),
            "deno" => {
                "install Deno: https://docs.deno.com/runtime/getting_started/installation/".into()
            }
            // Corepack n'est plus livré avec Node depuis la 25 : installation directe.
            other => format!("npm install -g {other}{at}"),
        },
    }
}

impl Toolchain {
    /// The package manager with `args`, in the project folder.
    ///
    /// ```no_run
    /// # fn run(tc: &moltn_toolchain::Toolchain) {
    /// let mut command = tc.command(&["outdated"]);
    /// # let _ = &mut command; }
    /// ```
    pub fn command<S: AsRef<OsStr>>(&self, args: &[S]) -> tokio::process::Command {
        let mut command = match self.prefix.split_first() {
            Some((program, rest)) => {
                let mut command = tokio::process::Command::new(program);
                command.args(rest).arg(self.spec.package_manager.name());
                command
            }
            None => tokio::process::Command::new(&self.package_manager.path),
        };
        command.args(args).current_dir(&self.dir);
        if let Some(path) = &self.path_env {
            command.env("PATH", path);
        }
        command
    }

    /// Install the dependencies.
    pub async fn install(&self, opts: InstallOpts) -> Result<(), ToolchainError> {
        let frozen = opts.frozen_lockfile;
        let args: &[&str] = match (self.spec.package_manager, frozen) {
            (PackageManager::Npm, true) => &["ci"],
            (PackageManager::Npm, false) => &["install"],
            (PackageManager::Pnpm | PackageManager::Bun, true) => &["install", "--frozen-lockfile"],
            (PackageManager::Yarn, true) if self.spec.yarn_berry => &["install", "--immutable"],
            (PackageManager::Yarn, true) => &["install", "--frozen-lockfile"],
            (PackageManager::Deno, true) => &["install", "--frozen"],
            (_, false) => &["install"],
        };
        process::run(self.command(args), "install", self.output.as_ref()).await
    }

    /// Run a `package.json` script to completion.
    pub async fn run_script(&self, script: &str, args: &[&str]) -> Result<(), ToolchainError> {
        process::run(
            self.command(&self.script_args(script, args)),
            script,
            self.output.as_ref(),
        )
        .await
    }

    /// Start a long-running script (dev server), supervised.
    pub fn spawn_script(&self, script: &str, args: &[&str]) -> Result<DevProcess, ToolchainError> {
        DevProcess::spawn_with_output(
            self.command(&self.script_args(script, args)),
            script,
            self.output.as_ref(),
        )
    }

    fn script_args<'a>(&self, script: &'a str, args: &[&'a str]) -> Vec<&'a str> {
        let mut all = match self.spec.package_manager {
            PackageManager::Deno => vec!["task", script],
            _ => vec!["run", script],
        };
        // npm transmet les arguments au script seulement après `--`.
        if self.spec.package_manager == PackageManager::Npm && !args.is_empty() {
            all.push("--");
        }
        all.extend_from_slice(args);
        all
    }

    /// Send the output of started commands to `output` instead of `tracing`.
    ///
    /// ```no_run
    /// # fn run(tc: moltn_toolchain::Toolchain) {
    /// let tc = tc.with_output(|label, line| println!("[{label}] {line}"));
    /// # let _ = tc; }
    /// ```
    #[must_use]
    pub fn with_output(mut self, output: impl Fn(&str, &str) + Send + Sync + 'static) -> Self {
        self.output = Some(std::sync::Arc::new(output));
        self
    }

    /// Frontend project folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Toolchain state: spec, version manager, binaries and versions.
    pub fn describe(&self) -> Description {
        Description {
            dir: self.dir.clone(),
            spec: self.spec.clone(),
            version_manager: self.manager,
            runtime: self.runtime.clone(),
            package_manager: self.package_manager.clone(),
        }
    }

    /// Resolved runtime.
    pub fn runtime(&self) -> Runtime {
        self.spec.runtime
    }
}
