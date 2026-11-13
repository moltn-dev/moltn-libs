//! Detection: which runtime and which package manager does the project expect?
//! Reads files only: no process is started.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::{Requirement, ToolchainError};

/// JavaScript runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    /// Node.js.
    Node,
    /// Bun.
    Bun,
    /// Deno.
    Deno,
}

impl Runtime {
    /// Binary and tool name (proto, mise, Volta).
    pub fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Bun => "bun",
            Self::Deno => "deno",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "node" | "nodejs" => Some(Self::Node),
            "bun" => Some(Self::Bun),
            "deno" => Some(Self::Deno),
            _ => None,
        }
    }
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Package manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    /// npm.
    Npm,
    /// pnpm.
    Pnpm,
    /// Yarn (classic or modern, see [`ToolchainSpec::yarn_berry`]).
    Yarn,
    /// bun.
    Bun,
    /// deno.
    Deno,
}

impl PackageManager {
    /// Binary and tool name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
            Self::Deno => "deno",
        }
    }

    /// bun and deno are also their own runtime.
    pub fn runtime(self) -> Option<Runtime> {
        match self {
            Self::Bun => Some(Runtime::Bun),
            Self::Deno => Some(Runtime::Deno),
            _ => None,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            "bun" => Some(Self::Bun),
            "deno" => Some(Self::Deno),
            _ => None,
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Where a decision comes from: shown by `moltn doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectedFrom {
    /// The decision, e.g. "package manager: pnpm".
    pub decision: String,
    /// The source, e.g. "package.json → packageManager".
    pub source: String,
    /// File read, if any.
    pub path: Option<PathBuf>,
}

/// Detection result.
#[derive(Debug, Clone, Serialize)]
pub struct ToolchainSpec {
    /// Expected runtime.
    pub runtime: Runtime,
    /// Required runtime version, if the project asks for one.
    pub runtime_req: Option<Requirement>,
    /// Expected package manager.
    pub package_manager: PackageManager,
    /// Required package manager version.
    pub pm_req: Option<Requirement>,
    /// Modern Yarn (2+, `.yarnrc.yml`): different install options.
    pub yarn_berry: bool,
    /// Origin of each decision.
    pub sources: Vec<DetectedFrom>,
    /// Ambiguities found (several lockfiles…), never blocking.
    pub warnings: Vec<String>,
    /// `.prototools` found (enables delegation to proto).
    pub prototools: Option<PathBuf>,
    /// `mise.toml` / `.tool-versions` found (enables delegation to mise).
    pub mise_config: Option<PathBuf>,
}

const LOCKFILES: [(&str, PackageManager); 7] = [
    ("pnpm-lock.yaml", PackageManager::Pnpm),
    ("yarn.lock", PackageManager::Yarn),
    ("bun.lock", PackageManager::Bun),
    ("bun.lockb", PackageManager::Bun),
    ("package-lock.json", PackageManager::Npm),
    ("deno.lock", PackageManager::Deno),
    ("deno.json", PackageManager::Deno),
];

/// Detect the toolchain of the frontend project `dir` (which must contain a `package.json`).
/// Walks up the parent folders to the repository root (folder containing `.git`),
/// for monorepos whose lockfile sits at the root.
pub(crate) fn detect(dir: &Path) -> Result<ToolchainSpec, ToolchainError> {
    if !dir.join("package.json").is_file() {
        return Err(ToolchainError::NoPackageJson(dir.to_path_buf()));
    }
    // Chemin absolu : un chemin relatif (`web`) n'a pas de parents à remonter.
    let dir = std::path::absolute(dir).map_err(|e| ToolchainError::io(dir, e))?;
    let dirs = search_dirs(&dir);
    let mut spec = ToolchainSpec {
        runtime: Runtime::Node,
        runtime_req: None,
        package_manager: PackageManager::Npm,
        pm_req: None,
        yarn_berry: false,
        sources: Vec::new(),
        warnings: Vec::new(),
        prototools: None,
        mise_config: None,
    };

    detect_package_manager(&dirs, &mut spec)?;
    detect_runtime(&dirs, &mut spec)?;
    Ok(spec)
}

fn search_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for d in dir.ancestors() {
        dirs.push(d.to_path_buf());
        if d.join(".git").exists() {
            break;
        }
    }
    dirs
}

fn read_package_json(dir: &Path) -> Result<Option<Value>, ToolchainError> {
    let path = dir.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| ToolchainError::io(&path, e))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| ToolchainError::InvalidFile {
            path,
            message: e.to_string(),
        })
}

/// `devEngines.x`: an object `{ name, version }` or an array of objects (the first one is used).
fn dev_engine<'a>(pkg: &'a Value, key: &str) -> Option<(&'a str, Option<&'a str>)> {
    let entry = &pkg["devEngines"][key];
    let entry = entry.as_array().and_then(|a| a.first()).unwrap_or(entry);
    Some((entry["name"].as_str()?, entry["version"].as_str()))
}

fn detect_package_manager(
    dirs: &[PathBuf],
    spec: &mut ToolchainSpec,
) -> Result<(), ToolchainError> {
    for dir in dirs {
        let pkg = read_package_json(dir)?;
        let pkg_path = dir.join("package.json");
        let lockfiles: Vec<(&str, PackageManager)> = LOCKFILES
            .iter()
            .copied()
            .filter(|(file, _)| dir.join(file).is_file())
            .collect();

        let declared = pkg.as_ref().and_then(|pkg| {
            if let Some(field) = pkg["packageManager"].as_str() {
                let (name, version) = field.split_once('@').unwrap_or((field, ""));
                let version = version.split('+').next().unwrap_or_default();
                return Some((
                    PackageManager::from_name(name)?,
                    version.to_string(),
                    "package.json → packageManager",
                ));
            }
            let (name, version) = dev_engine(pkg, "packageManager")?;
            Some((
                PackageManager::from_name(name)?,
                version.unwrap_or_default().to_string(),
                "package.json → devEngines.packageManager",
            ))
        });

        let mut distinct: Vec<PackageManager> = lockfiles.iter().map(|(_, pm)| *pm).collect();
        distinct.dedup();
        if let Some((pm, version, source)) = declared {
            spec.package_manager = pm;
            spec.pm_req = (!version.is_empty()).then(|| Requirement::parse(&version));
            spec.sources.push(found(
                format!("package manager: {pm}"),
                source,
                Some(pkg_path),
            ));
            if let Some((file, other)) = lockfiles.iter().find(|(_, other)| *other != pm) {
                spec.warnings.push(format!(
                    "{} declares {pm}, but {file} ({other}) is present in {}",
                    "package.json",
                    dir.display()
                ));
            }
        } else if let Some((file, pm)) = lockfiles.first() {
            spec.package_manager = *pm;
            spec.sources.push(found(
                format!("package manager: {pm}"),
                format!("lockfile {file}"),
                Some(dir.join(file)),
            ));
            if distinct.len() > 1 {
                let names: Vec<&str> = lockfiles.iter().map(|(f, _)| *f).collect();
                spec.warnings.push(format!(
                    "several lockfiles in {} ({}): {pm} chosen; remove the others",
                    dir.display(),
                    names.join(", ")
                ));
            }
        } else {
            continue;
        }
        spec.yarn_berry = spec.package_manager == PackageManager::Yarn
            && (dir.join(".yarnrc.yml").is_file()
                || spec
                    .pm_req
                    .as_ref()
                    .is_some_and(|r| r.hint().parse::<u32>().is_ok_and(|major| major >= 2)));
        return Ok(());
    }
    spec.sources.push(found(
        "package manager: npm".into(),
        "default (no lockfile nor packageManager)",
        None,
    ));
    Ok(())
}

fn detect_runtime(dirs: &[PathBuf], spec: &mut ToolchainSpec) -> Result<(), ToolchainError> {
    // Runtime : devEngines.runtime, sinon celui du gestionnaire (bun, deno), sinon Node.
    let mut engines: Option<(Requirement, PathBuf)> = None;
    let mut runtime_decided = false;
    for dir in dirs {
        let Some(pkg) = read_package_json(dir)? else {
            continue;
        };
        let path = dir.join("package.json");
        if !runtime_decided
            && let Some((name, version)) = dev_engine(&pkg, "runtime")
            && let Some(runtime) = Runtime::from_name(name)
        {
            {
                spec.runtime = runtime;
                spec.sources.push(found(
                    format!("runtime: {runtime}"),
                    "package.json → devEngines.runtime",
                    Some(path.clone()),
                ));
                runtime_decided = true;
                if let Some(v) = version {
                    engines.get_or_insert((Requirement::parse(v), path.clone()));
                }
            }
        }
        if engines.is_none()
            && let Some(v) = pkg["engines"]["node"].as_str()
        {
            engines = Some((Requirement::parse(v), path));
        }
    }
    if !runtime_decided {
        if let Some(runtime) = spec.package_manager.runtime() {
            spec.runtime = runtime;
            spec.sources.push(found(
                format!("runtime: {runtime}"),
                format!("package manager {}", spec.package_manager),
                None,
            ));
        } else {
            spec.sources
                .push(found("runtime: node".into(), "default", None));
        }
    }

    // Version : fichiers de version d'abord (le plus proche gagne), puis package.json.
    let runtime = spec.runtime;
    for dir in dirs {
        let prototools = dir.join(".prototools");
        if prototools.is_file() {
            spec.prototools.get_or_insert(prototools.clone());
            let table = read_toml(&prototools)?;
            take_version(spec, &table, runtime, &prototools, ".prototools");
        }
        for name in ["mise.toml", ".mise.toml"] {
            let path = dir.join(name);
            if path.is_file() {
                spec.mise_config.get_or_insert(path.clone());
                let table = read_toml(&path)?;
                if let Some(tools) = table.get("tools").and_then(toml::Value::as_table) {
                    take_version(spec, tools, runtime, &path, name);
                }
            }
        }
        let tool_versions = dir.join(".tool-versions");
        if tool_versions.is_file() {
            spec.mise_config.get_or_insert(tool_versions.clone());
            let text = std::fs::read_to_string(&tool_versions)
                .map_err(|e| ToolchainError::io(&tool_versions, e))?;
            for line in text.lines() {
                let mut words = line.split_whitespace();
                if let (Some(tool), Some(version)) = (words.next(), words.next()) {
                    set_version(spec, tool, version, &tool_versions, ".tool-versions");
                }
            }
        }
        if runtime == Runtime::Node {
            for name in [".nvmrc", ".node-version"] {
                let path = dir.join(name);
                if spec.runtime_req.is_none() && path.is_file() {
                    let text =
                        std::fs::read_to_string(&path).map_err(|e| ToolchainError::io(&path, e))?;
                    let version = text.lines().next().unwrap_or_default().trim();
                    if !version.is_empty() {
                        spec.runtime_req = Some(Requirement::parse(version));
                        spec.sources.push(found(
                            format!("node version: {version}"),
                            name,
                            Some(path),
                        ));
                    }
                }
            }
        }
    }
    if spec.runtime_req.is_none()
        && let Some((req, path)) = engines
    {
        spec.sources.push(found(
            format!("{runtime} version: {req}"),
            "package.json → engines / devEngines",
            Some(path),
        ));
        spec.runtime_req = Some(req);
    }
    if spec.runtime_req.is_none() && runtime == Runtime::Node {
        spec.warnings.push(
            "no Node version requested (.nvmrc, engines.node…): any installed version will be used; an active LTS is recommended".into(),
        );
    }
    Ok(())
}

fn read_toml(path: &Path) -> Result<toml::Table, ToolchainError> {
    let text = std::fs::read_to_string(path).map_err(|e| ToolchainError::io(path, e))?;
    text.parse()
        .map_err(|e: toml::de::Error| ToolchainError::InvalidFile {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
}

/// `node = "24"` (or `node = ["24"]`) keys of a proto / mise table.
fn take_version(
    spec: &mut ToolchainSpec,
    table: &toml::Table,
    runtime: Runtime,
    path: &Path,
    source: &str,
) {
    let keys = [runtime.name(), spec.package_manager.name()];
    for key in keys {
        let version = table
            .get(key)
            .and_then(|v| v.as_str().or_else(|| v.as_array()?.first()?.as_str()));
        if let Some(version) = version {
            set_version(spec, key, version, path, source);
        }
    }
}

/// Keep the first version found for the runtime and for the package manager.
fn set_version(spec: &mut ToolchainSpec, tool: &str, version: &str, path: &Path, source: &str) {
    if Runtime::from_name(tool) == Some(spec.runtime) && spec.runtime_req.is_none() {
        spec.runtime_req = Some(Requirement::parse(version));
        spec.sources.push(found(
            format!("{} version: {version}", spec.runtime),
            source,
            Some(path.to_path_buf()),
        ));
    } else if PackageManager::from_name(tool) == Some(spec.package_manager)
        && spec.package_manager.runtime().is_none()
        && spec.pm_req.is_none()
    {
        spec.pm_req = Some(Requirement::parse(version));
        spec.sources.push(found(
            format!("{} version: {version}", spec.package_manager),
            source,
            Some(path.to_path_buf()),
        ));
    }
}

fn found(decision: String, source: impl Into<String>, path: Option<PathBuf>) -> DetectedFrom {
    DetectedFrom {
        decision,
        source: source.into(),
        path,
    }
}
