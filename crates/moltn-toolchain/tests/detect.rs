//! Détection : un projet jetable par cas (`.git` à la racine borne la remontée).

use std::path::{Path, PathBuf};

use moltn_toolchain::{JsProject, PackageManager, Runtime, ToolchainError};

/// Crée les fichiers donnés dans un dossier temporaire marqué comme racine de dépôt.
fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    for (path, content) in files {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let web = root.path().join("web");
    (root, web)
}

fn detect(dir: &Path) -> moltn_toolchain::ToolchainSpec {
    JsProject::detect(dir).unwrap().spec().clone()
}

const PKG: &str = "{}";

#[test]
fn pnpm_lockfile() {
    let (_root, web) = project(&[("web/package.json", PKG), ("web/pnpm-lock.yaml", "")]);
    let spec = detect(&web);
    assert_eq!(spec.package_manager, PackageManager::Pnpm);
    assert_eq!(spec.runtime, Runtime::Node);
    assert!(
        spec.warnings
            .iter()
            .any(|w| w.contains("no Node version requested")),
        "{:?}",
        spec.warnings
    );
}

#[test]
fn package_manager_field_wins_over_lockfile() {
    let (_root, web) = project(&[
        (
            "web/package.json",
            r#"{ "packageManager": "pnpm@10.4.1+sha512.abc" }"#,
        ),
        ("web/yarn.lock", ""),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.package_manager, PackageManager::Pnpm);
    assert_eq!(spec.pm_req.unwrap().as_str(), "10.4.1");
    assert!(
        spec.warnings.iter().any(|w| w.contains("yarn.lock")),
        "{:?}",
        spec.warnings
    );
}

#[test]
fn dev_engines_package_manager_and_modern_yarn() {
    let (_root, web) = project(&[
        (
            "web/package.json",
            r#"{ "devEngines": { "packageManager": { "name": "yarn", "version": "^4" } } }"#,
        ),
        ("web/.yarnrc.yml", ""),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.package_manager, PackageManager::Yarn);
    assert!(spec.yarn_berry);
}

#[test]
fn bun_lockfile_means_bun_runtime() {
    let (_root, web) = project(&[("web/package.json", PKG), ("web/bun.lock", "")]);
    let spec = detect(&web);
    assert_eq!(
        (spec.package_manager, spec.runtime),
        (PackageManager::Bun, Runtime::Bun)
    );
}

#[test]
fn deno_json_means_deno() {
    let (_root, web) = project(&[("web/package.json", PKG), ("web/deno.json", "{}")]);
    let spec = detect(&web);
    assert_eq!(
        (spec.package_manager, spec.runtime),
        (PackageManager::Deno, Runtime::Deno)
    );
}

#[test]
fn several_lockfiles_warn_and_keep_priority() {
    let (_root, web) = project(&[
        ("web/package.json", PKG),
        ("web/package-lock.json", "{}"),
        ("web/pnpm-lock.yaml", ""),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.package_manager, PackageManager::Pnpm);
    assert!(
        spec.warnings
            .iter()
            .any(|w| w.contains("several lockfiles")),
        "{:?}",
        spec.warnings
    );
}

#[test]
fn nothing_means_npm_and_node() {
    let (_root, web) = project(&[("web/package.json", PKG)]);
    let spec = detect(&web);
    assert_eq!(
        (spec.package_manager, spec.runtime),
        (PackageManager::Npm, Runtime::Node)
    );
    assert!(spec.sources.iter().any(|s| s.source.starts_with("default")));
}

#[test]
fn nvmrc_version() {
    let (_root, web) = project(&[("web/package.json", PKG), ("web/.nvmrc", "v24.1.0\n")]);
    assert_eq!(detect(&web).runtime_req.unwrap().as_str(), "v24.1.0");
}

#[test]
fn engines_node_when_no_version_file() {
    let (_root, web) = project(&[("web/package.json", r#"{ "engines": { "node": ">=22" } }"#)]);
    assert_eq!(detect(&web).runtime_req.unwrap().as_str(), ">=22");
}

#[test]
fn version_file_beats_engines() {
    let (_root, web) = project(&[
        ("web/package.json", r#"{ "engines": { "node": ">=18" } }"#),
        ("web/.node-version", "22.4.0"),
    ]);
    assert_eq!(detect(&web).runtime_req.unwrap().as_str(), "22.4.0");
}

#[test]
fn prototools_gives_both_versions() {
    let (_root, web) = project(&[
        ("web/package.json", PKG),
        ("web/pnpm-lock.yaml", ""),
        ("web/.prototools", "node = \"24\"\npnpm = \"10\"\n"),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.runtime_req.unwrap().as_str(), "24");
    assert_eq!(spec.pm_req.unwrap().as_str(), "10");
    assert!(spec.prototools.is_some());
}

#[test]
fn mise_toml_tools() {
    let (_root, web) = project(&[
        ("web/package.json", PKG),
        ("web/mise.toml", "[tools]\nnode = \"22\"\n"),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.runtime_req.unwrap().as_str(), "22");
    assert!(spec.mise_config.is_some());
}

#[test]
fn tool_versions_uses_asdf_name() {
    let (_root, web) = project(&[
        ("web/package.json", PKG),
        ("web/.tool-versions", "nodejs 20.1.0\n"),
    ]);
    assert_eq!(detect(&web).runtime_req.unwrap().as_str(), "20.1.0");
}

#[test]
fn monorepo_root_decides() {
    let (_root, web) = project(&[
        ("package.json", r#"{ "packageManager": "pnpm@11.8.0" }"#),
        ("pnpm-lock.yaml", ""),
        (".nvmrc", "24"),
        ("web/package.json", PKG),
    ]);
    let spec = detect(&web);
    assert_eq!(spec.package_manager, PackageManager::Pnpm);
    assert_eq!(spec.runtime_req.unwrap().as_str(), "24");
}

/// Bug : `moltn doctor` lancé avec `--web web` (chemin relatif) ne remontait pas aux parents.
#[test]
fn relative_path_still_searches_parents() {
    let (root, _web) = project(&[("pnpm-lock.yaml", ""), ("web/package.json", PKG)]);
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(root.path()).unwrap();
    let spec = JsProject::detect("web").map(|p| p.spec().clone());
    std::env::set_current_dir(previous).unwrap();
    assert_eq!(spec.unwrap().package_manager, PackageManager::Pnpm);
}

#[test]
fn search_stops_at_repository_root() {
    // Un lockfile au-dessus de la racine du dépôt (ex. dossier personnel) est ignoré.
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("yarn.lock"), "").unwrap();
    let repo = outer.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("web")).unwrap();
    std::fs::write(repo.join("web/package.json"), PKG).unwrap();
    assert_eq!(
        detect(&repo.join("web")).package_manager,
        PackageManager::Npm
    );
}

#[test]
fn explicit_configuration_overrides() {
    let (_root, web) = project(&[("web/package.json", PKG), ("web/yarn.lock", "")]);
    let project = JsProject::detect(&web)
        .unwrap()
        .with_package_manager(PackageManager::Pnpm, None);
    assert_eq!(project.spec().package_manager, PackageManager::Pnpm);
    assert_eq!(
        project.spec().sources.last().unwrap().source,
        "explicit configuration"
    );
}

#[test]
fn missing_package_json() {
    let (_root, web) = project(&[]);
    assert!(matches!(
        JsProject::detect(&web),
        Err(ToolchainError::NoPackageJson(_))
    ));
}

#[test]
fn invalid_package_json_names_file() {
    let (_root, web) = project(&[("web/package.json", "{ pas du json")]);
    let err = JsProject::detect(&web).unwrap_err();
    assert!(err.to_string().contains("package.json"), "{err}");
}
