//! Résolution : `PATH` simulé avec de faux binaires qui affichent une version.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use moltn_toolchain::{JsProject, ToolchainError, VersionManager};

struct Fixture {
    _root: tempfile::TempDir,
    web: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        let web = root.path().join("web");
        let bin = root.path().join("bin");
        std::fs::create_dir_all(&web).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(web.join("package.json"), "{}").unwrap();
        for (path, content) in files {
            std::fs::write(web.join(path), content).unwrap();
        }
        Self {
            _root: root,
            web,
            bin,
        }
    }

    /// Faux binaire qui affiche `output` quels que soient ses arguments.
    fn tool(&self, name: &str, output: &str) -> &Self {
        fake_binary(&self.bin, name, output);
        self
    }

    fn path(&self) -> OsString {
        self.bin.clone().into_os_string()
    }

    fn resolve(&self) -> Result<moltn_toolchain::Toolchain, ToolchainError> {
        JsProject::detect(&self.web)
            .unwrap()
            .resolve_with_path(self.path())
    }
}

#[cfg(windows)]
fn fake_binary(dir: &Path, name: &str, output: &str) {
    std::fs::write(
        dir.join(format!("{name}.cmd")),
        format!("@echo {output}\r\n"),
    )
    .unwrap();
}

#[cfg(unix)]
fn fake_binary(dir: &Path, name: &str, output: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\necho '{output}'\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn args(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn finds_node_and_pnpm_in_path() {
    let f = Fixture::new(&[("pnpm-lock.yaml", ""), (".nvmrc", "24")]);
    f.tool("node", "v24.1.0").tool("pnpm", "10.4.1");
    let description = f.resolve().unwrap().describe();
    assert_eq!(description.version_manager, VersionManager::None);
    assert_eq!(description.runtime.version.to_string(), "24.1.0");
    assert_eq!(description.package_manager.version.to_string(), "10.4.1");
}

#[test]
fn wrong_node_version_says_what_to_install() {
    let f = Fixture::new(&[(".nvmrc", "24")]);
    f.tool("node", "v20.11.0").tool("npm", "10.2.0");
    let err = f.resolve().unwrap_err();
    assert!(
        matches!(&err, ToolchainError::VersionMismatch { tool, .. } if tool == "node"),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(
        message.contains("node 24 required, 20.11.0 found"),
        "{message}"
    );
    assert!(message.contains("https://nodejs.org"), "{message}");
}

#[test]
fn hint_uses_an_installed_version_manager() {
    let f = Fixture::new(&[(".nvmrc", "24")]);
    f.tool("node", "v20.11.0")
        .tool("npm", "10.2.0")
        .tool("mise", "2026.1.0");
    let message = f.resolve().unwrap_err().to_string();
    assert!(message.contains("mise use node@24"), "{message}");
}

#[test]
fn missing_package_manager_says_how_to_get_it() {
    let f = Fixture::new(&[("package.json", r#"{ "packageManager": "pnpm@10.4.1" }"#)]);
    f.tool("node", "v24.1.0");
    let err = f.resolve().unwrap_err();
    assert!(
        matches!(&err, ToolchainError::ToolNotFound { tool, .. } if tool == "pnpm"),
        "{err:?}"
    );
    assert!(err.to_string().contains("npm install -g pnpm@10"), "{err}");
}

#[test]
fn missing_runtime() {
    let f = Fixture::new(&[]);
    let err = f.resolve().unwrap_err();
    assert!(
        matches!(&err, ToolchainError::ToolNotFound { tool, .. } if tool == "node"),
        "{err:?}"
    );
}

#[test]
fn proto_with_prototools_wraps_commands() {
    let f = Fixture::new(&[(".prototools", "node = \"24\"\n"), ("pnpm-lock.yaml", "")]);
    // Le faux proto répond « 24.2.0 » à `proto exec node pnpm -- <outil> --version`, pour
    // node comme pour pnpm : pas de version de pnpm imposée ici.
    f.tool("proto", "24.2.0");
    let tc = f.resolve().unwrap();
    assert_eq!(tc.describe().version_manager, VersionManager::Proto);
    let command = tc.command(&["install"]);
    assert!(
        command
            .as_std()
            .get_program()
            .to_string_lossy()
            .contains("proto")
    );
    assert_eq!(
        args(&command),
        ["exec", "node", "pnpm", "--", "pnpm", "install"]
    );
}

#[test]
fn mise_with_config_wraps_commands() {
    let f = Fixture::new(&[("mise.toml", "[tools]\nnode = \"24\"\n")]);
    f.tool("mise", "24.0.0");
    let tc = f.resolve().unwrap();
    assert_eq!(tc.describe().version_manager, VersionManager::Mise);
    assert_eq!(
        args(&tc.command(&["install"])),
        ["exec", "--", "npm", "install"]
    );
}

#[test]
fn proto_without_prototools_is_not_used() {
    let f = Fixture::new(&[]);
    f.tool("proto", "0.53.0")
        .tool("node", "v24.1.0")
        .tool("npm", "10.2.0");
    assert_eq!(
        f.resolve().unwrap().describe().version_manager,
        VersionManager::None
    );
}

#[test]
fn volta_shims_are_used_directly() {
    let f = Fixture::new(&[]);
    f.tool("volta", "2.0.2")
        .tool("node", "v24.1.0")
        .tool("npm", "10.2.0");
    let tc = f.resolve().unwrap();
    assert_eq!(tc.describe().version_manager, VersionManager::Volta);
    assert_eq!(args(&tc.command(&["install"])), ["install"]);
}

#[test]
fn deno_version_output() {
    let f = Fixture::new(&[("deno.json", "{}")]);
    f.tool(
        "deno",
        "deno 2.1.4 (stable, release, x86_64-pc-windows-msvc)",
    );
    let description = f.resolve().unwrap().describe();
    assert_eq!(description.runtime.version.to_string(), "2.1.4");
    assert_eq!(description.package_manager.name, "deno");
}
