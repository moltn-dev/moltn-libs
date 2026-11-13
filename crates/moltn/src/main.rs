//! `moltn` : lancer, construire et diagnostiquer une application moltn.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::{Context as _, bail};
use clap::{Parser, Subcommand};
use moltn_pages::config::Config;
use moltn_toolchain::{
    Description, DevProcess, InstallOpts, JsProject, OutputFn, ReadyProbe, Toolchain,
};
use tracing_subscriber::EnvFilter;

mod templates;

#[derive(Parser)]
#[command(
    name = "moltn",
    version,
    about = "Rust + Inertia applications: create, develop, build, check"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a moltn application: Rust (axum) + Inertia + Vue, React or Svelte.
    New {
        /// Project name (and folder name).
        name: String,
        /// Frontend framework.
        #[arg(long, value_enum, default_value = "vue")]
        front: templates::Front,
        /// JavaScript engine of server-side rendering.
        #[arg(long, value_enum, default_value = "quickjs")]
        engine: templates::Engine,
    },
    /// Run Vite (hot reload, dev server-side rendering) and the Rust app; Ctrl+C stops everything.
    Dev {
        /// Frontend folder [default: `front.dir` of moltn.toml, or web].
        #[arg(long)]
        web: Option<PathBuf>,
        /// Application port (passed through the `PORT` variable): the address to open
        /// [default: `dev.port` of moltn.toml, or 3000].
        #[arg(long)]
        port: Option<u16>,
        /// Vite port (assets and dev server-side rendering, never opened directly)
        /// [default: `dev.vite_port` of moltn.toml, or 5173].
        #[arg(long)]
        vite_port: Option<u16>,
    },
    /// Build the frontend (client + SSR), then the release binary.
    Build {
        /// Frontend folder [default: `front.dir` of moltn.toml, or web].
        #[arg(long)]
        web: Option<PathBuf>,
    },
    /// Regenerate `moltn.ts` (props types and routes) from the Rust code.
    Gen {
        /// Only check that the file is up to date (CI); fail otherwise.
        #[arg(long)]
        check: bool,
    },
    /// Show the detected JavaScript toolchain and what is missing.
    Doctor {
        /// Frontend folder [default: `front.dir` of moltn.toml, or web].
        #[arg(long)]
        web: Option<PathBuf>,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .without_time()
        .with_target(false)
        .init();
    let result = run(Cli::parse().command).await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> anyhow::Result<()> {
    if let Command::New {
        name,
        front,
        engine,
    } = command
    {
        return new_project(&name, front, engine).await;
    }
    let cwd = std::env::current_dir().context("current folder")?;
    let mut config = Config::load(&cwd)?;
    // Tous les chemins de moltn.toml sont relatifs à son dossier : on s'y place, quel que
    // soit le sous-dossier d'où la commande est lancée.
    if config.file.is_some() {
        std::env::set_current_dir(&config.root)
            .with_context(|| format!("entering {}", config.root.display()))?;
    }
    // Désormais dans le dossier racine : chemins relatifs, plus lisibles dans les messages.
    config.root = PathBuf::new();
    let mut front = |web: Option<PathBuf>| {
        if let Some(web) = web {
            config.front.dir = cwd.join(web);
        }
    };
    match command {
        Command::Dev {
            web,
            port,
            vite_port,
        } => {
            front(web);
            config.dev.port = port.unwrap_or(config.dev.port);
            config.dev.vite_port = vite_port.unwrap_or(config.dev.vite_port);
            dev(&config).await
        }
        Command::Build { web } => {
            front(web);
            build(&config.front_dir()).await
        }
        Command::Gen { check } => generate(&config, check).await,
        Command::Doctor { web, json } => {
            front(web);
            doctor(&config, json).await
        }
        Command::New { .. } => unreachable!("handled above"),
    }
}

/// Détection puis résolution (bloquante : lance `--version`), avertissements affichés.
async fn toolchain(web: &Path) -> anyhow::Result<Toolchain> {
    let project = JsProject::detect(web)?;
    for warning in &project.spec().warnings {
        tracing::warn!("{warning}");
    }
    let tc = tokio::task::spawn_blocking(move || project.resolve()).await??;
    Ok(tc.with_output(print_line))
}

/// Sortie des processus lancés, telle quelle (couleurs comprises), préfixée par leur nom.
/// Le script `dev` est Vite : sa bannière de démarrage est masquée, car l'adresse qu'elle
/// affiche n'est pas celle à ouvrir (c'est celle de l'application).
fn print_line(label: &str, line: &str) {
    if label == "dev" {
        if !is_vite_banner(line) {
            println!("[vite] {line}");
        }
    } else {
        println!("[{label}] {line}");
    }
}

fn is_vite_banner(line: &str) -> bool {
    let plain = strip_ansi(line);
    let text = plain.trim();
    text.is_empty()
        || text.starts_with("$ vite")
        || text.starts_with("> ")
        || [
            "VITE v",
            "Local:",
            "Network:",
            "press h",
            "Inertia SSR dev endpoint",
            "Warming up Inertia SSR",
            "SSR module graph warmed up",
        ]
        .iter()
        .any(|noise| text.contains(noise))
}

/// Retire les séquences de couleur (`ESC [ … lettre`).
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Un port déjà pris ferait annoncer une application qui n'est pas la nôtre.
fn ensure_port_free(port: u16, flag: &str) -> anyhow::Result<()> {
    std::net::TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .with_context(|| {
            format!("port {port} is already in use: stop the other process or pass `{flag} <port>`")
        })
}

async fn dev(config: &Config) -> anyhow::Result<()> {
    let (web, port, vite_port) = (&config.front_dir(), config.dev.port, config.dev.vite_port);
    ensure_port_free(port, "--port")?;
    ensure_port_free(vite_port, "--vite-port")?;
    let tc = toolchain(web).await?;
    if !web.join("node_modules").is_dir() {
        tracing::info!("installing frontend dependencies…");
        tc.install(InstallOpts::default()).await?;
    }

    tracing::info!("starting Vite and the application…");
    let vite_url = format!("http://localhost:{vite_port}");
    let vite_port_arg = vite_port.to_string();
    let mut vite = tc.spawn_script("dev", &["--port", &vite_port_arg, "--strictPort"])?;
    vite.wait_ready(
        ReadyProbe::Tcp(format!("localhost:{vite_port}")),
        Duration::from_secs(60),
    )
    .await
    .context("starting Vite (`dev` script of package.json)")?;

    // Une seule annonce en attente : sans cela, chaque compilation ratée en laisserait une
    // de plus, et toutes s'afficheraient au premier démarrage réussi.
    let mut announcing: Option<tokio::task::JoinHandle<()>> = None;
    let mut start_app = |first: bool| -> anyhow::Result<DevProcess> {
        let mut command = tokio::process::Command::new("cargo");
        // L'application sert assets et rendu serveur depuis Vite tant que cette variable existe.
        command
            .args(&config.dev.cargo)
            .env("MOLTN_VITE_URL", &vite_url)
            .env("PORT", port.to_string());
        let print: OutputFn = Arc::new(print_line);
        let app = DevProcess::spawn_with_output(command, "app", Some(&print))?;
        if let Some(previous) = announcing.replace(tokio::spawn(announce(port, first))) {
            previous.abort();
        }
        Ok(app)
    };

    let pages = config.pages_dir();
    let typescript = config.typescript();
    let watch = &config.dev.watch;
    let mut watched = watched_files(watch);
    let mut page_list = page_files(&pages);
    let mut app = Some(start_app(true)?);
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("stopping");
                break;
            }
            status = vite.wait() => {
                tracing::warn!("Vite stopped ({})", status.map(|s| s.to_string()).unwrap_or_default());
                break;
            }
            status = wait_app(&mut app) => {
                app = None;
                let status = status.map(|s| s.to_string()).unwrap_or_default();
                tracing::warn!("the application stopped ({status}); waiting for a Rust code change…");
            }
            _ = tick.tick() => {
                if watched_files(watch) != watched || page_files(&pages) != page_list {
                    // Laisse l'éditeur finir d'écrire (sauvegardes en plusieurs fichiers).
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    watched = watched_files(watch);
                    let now = page_files(&pages);
                    if now != page_list {
                        page_list = now;
                        // Routes et liste des pages tout de suite, sans attendre la compilation ;
                        // l'application complète les types des props à son redémarrage.
                        match moltn_pages::build::refresh_typescript(&pages, &typescript) {
                            Ok(true) => tracing::info!("pages changed: routes updated in {}", typescript.display()),
                            Ok(false) => {}
                            Err(e) => tracing::warn!("{e}"),
                        }
                    }
                    tracing::info!("recompiling…");
                    if let Some(running) = app.take() {
                        running.stop().await;
                    }
                    app = Some(start_app(false)?);
                }
            }
        }
    }
    // Détruire les DevProcess tue Vite, cargo et l'application, enfants compris.
    drop(app);
    drop(vite);
    Ok(())
}

async fn wait_app(
    app: &mut Option<DevProcess>,
) -> Result<std::process::ExitStatus, moltn_toolchain::ToolchainError> {
    match app {
        Some(app) => app.wait().await,
        None => std::future::pending().await,
    }
}

/// Annonce l'adresse de l'application dès qu'elle répond.
async fn announce(port: u16, first: bool) {
    let addr = format!("127.0.0.1:{port}");
    while tokio::net::TcpStream::connect(&addr).await.is_err() {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if first {
        println!("\n  \u{1b}[1;32m➜ moltn\u{1b}[0m  http://{addr}   (Ctrl+C to stop)");
        println!("    frontend: hot reload; Rust code: recompiled on every change\n");
    } else {
        println!("\n  \u{1b}[1;32m➜ moltn\u{1b}[0m  application restarted: http://{addr}\n");
    }
}

/// Fichiers surveillés (`dev.watch` : par défaut `src`, `Cargo.toml`, `build.rs`) et leur date
/// de modification : un changement demande de recompiler. Un dossier compte pour tous ses
/// fichiers (sources, migrations, gabarits…).
fn watched_files(watch: &[PathBuf]) -> BTreeMap<PathBuf, Option<SystemTime>> {
    let mut files = BTreeMap::new();
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    for path in watch {
        if path.is_dir() {
            walk(path, &mut |p| {
                files.insert(p.to_path_buf(), modified(p));
            });
        } else {
            files.insert(path.clone(), modified(path));
        }
    }
    files
}

/// **Liste** des fichiers de `pages/` (ajout, suppression, renommage) : leur contenu, Vite
/// s'en charge ; la liste change les routes, donc `moltn.ts` et le routeur.
fn page_files(pages: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    walk(pages, &mut |p| {
        files.insert(p.to_path_buf());
    });
    files
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}

async fn build(web: &Path) -> anyhow::Result<()> {
    let tc = toolchain(web).await?;
    tracing::info!("installing frontend dependencies…");
    tc.install(InstallOpts {
        frozen_lockfile: std::env::var_os("CI").is_some(),
    })
    .await?;
    tracing::info!("building the frontend (client + SSR)…");
    tc.run_script("build", &[]).await?;
    tracing::info!("building the release binary…");
    let status = tokio::process::Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .await
        .context("running cargo")?;
    if !status.success() {
        bail!("`cargo build --release` failed ({status})");
    }
    tracing::info!("done: the binary is in target/release");
    Ok(())
}

/// Dépôt moltn dont la CLI est issue : tant que les crates ne sont pas publiés, les projets
/// créés en dépendent par chemin.
fn moltn_repo() -> Option<PathBuf> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2)?;
    repo.join("crates/moltn-ssr")
        .is_dir()
        .then(|| repo.to_path_buf())
}

async fn new_project(
    name: &str,
    front: templates::Front,
    engine: templates::Engine,
) -> anyhow::Result<()> {
    let valid = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        bail!(
            "invalid name \"{name}\": lowercase letters, digits, - and _, starting with a letter"
        );
    }
    let dir = Path::new(name);
    if dir.exists() {
        bail!("folder {name} already exists");
    }

    // pnpm s'il est installé (déclaré dans package.json, pour que la détection le retienne), sinon npm.
    let pnpm_version =
        tokio::process::Command::new(if cfg!(windows) { "pnpm.cmd" } else { "pnpm" })
            .arg("--version")
            .output()
            .await
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let pnpm = pnpm_version.is_some();
    let package_manager = pnpm_version
        .map(|v| format!("\n  \"packageManager\": \"pnpm@{v}\","))
        .unwrap_or_default();
    let (moltn_pages, moltn_ssr, moltn_vite) = match moltn_repo() {
        Some(repo) => {
            let repo = repo.to_string_lossy().replace('\\', "/");
            (
                format!(r#"{{ path = "{repo}/crates/moltn-pages" }}"#),
                format!(r#"{{ path = "{repo}/crates/moltn-ssr" }}"#),
                format!(
                    "{}:{repo}/packages/vite",
                    if pnpm { "link" } else { "file" }
                ),
            )
        }
        None => {
            tracing::warn!(
                "moltn crates not published yet: the project will only build once moltn is on crates.io and npm"
            );
            (r#""0.1""#.into(), r#""0.1""#.into(), "^0.1.0".into())
        }
    };
    // build.rs n'utilise que le scan : moltn-pages sans le runtime (axum, veer, ts-rs).
    let moltn_pages_build = match moltn_pages.strip_suffix(" }") {
        Some(spec) => format!("{spec}, default-features = false }}"),
        None => format!("{{ version = {moltn_pages}, default-features = false }}"),
    };
    // V8 se choisit à la compilation : une feature de moltn-ssr, à la place de QuickJS.
    let moltn_ssr = match (engine, moltn_ssr.strip_suffix(" }")) {
        (templates::Engine::Quickjs, _) => moltn_ssr,
        (templates::Engine::V8, Some(spec)) => {
            format!(r#"{spec}, default-features = false, features = ["veer", "v8"] }}"#)
        }
        (templates::Engine::V8, None) => format!(
            r#"{{ version = {moltn_ssr}, default-features = false, features = ["veer", "v8"] }}"#
        ),
    };
    let react_refresh = if front == templates::Front::React {
        ".react_refresh(true)"
    } else {
        ""
    };

    for (path, content) in templates::files(front) {
        let content = content
            .replace("{{name}}", name)
            .replace("{{moltn_pages_build}}", &moltn_pages_build)
            .replace("{{moltn_pages}}", &moltn_pages)
            .replace("{{moltn_ssr}}", &moltn_ssr)
            .replace("{{moltn_vite}}", &moltn_vite)
            .replace("{{react_refresh}}", react_refresh)
            .replace("{{package_manager}}", &package_manager);
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    }
    tracing::info!("project {name} created; installing frontend dependencies…");

    let web = dir.join("web");
    let tc = toolchain(&web).await?;
    tc.install(InstallOpts::default()).await?;

    tracing::info!("first build of the application (a few minutes, only once)…");
    let status = tokio::process::Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(dir)
        .env("MOLTN_GEN", "write")
        .status()
        .await
        .context("running cargo")?;
    if !status.success() {
        bail!("first build failed ({status}): see the errors above");
    }
    println!(
        "\n  \u{1b}[1;32m➜ moltn\u{1b}[0m  project ready:\n\n      cd {name}\n      moltn dev\n"
    );
    Ok(())
}

/// Lance l'application en mode génération : `moltn_pages::router()` écrit (ou vérifie)
/// `moltn.ts` puis quitte, avant de servir quoi que ce soit.
async fn generate(config: &Config, check: bool) -> anyhow::Result<()> {
    tracing::info!("compiling the application…");
    // `--quiet` juste après la sous-commande : avant un éventuel `--` suivi d'arguments.
    let mut args = config.dev.cargo.clone();
    args.insert(args.len().min(1), "--quiet".into());
    let status = tokio::process::Command::new("cargo")
        .args(&args)
        .env("MOLTN_GEN", if check { "check" } else { "write" })
        .status()
        .await
        .context("running cargo")?;
    match status.code() {
        Some(0) if check => tracing::info!("moltn.ts is up to date"),
        Some(0) => tracing::info!("moltn.ts regenerated"),
        Some(1) if check => {
            bail!("moltn.ts is out of date: run `moltn gen` and commit the file")
        }
        Some(2) => bail!("could not write moltn.ts (permissions, path?)"),
        _ => bail!(
            "the application did not produce moltn.ts ({status}): compile error above, \
             or `moltn_pages::router()` is not called at startup"
        ),
    }
    Ok(())
}

async fn doctor(config: &Config, json: bool) -> anyhow::Result<()> {
    if !json {
        print_config(config);
    }
    let project = JsProject::detect(config.front_dir())?;
    let spec = project.spec().clone();
    let resolved = tokio::task::spawn_blocking(move || project.resolve()).await?;
    if json {
        let value = match &resolved {
            Ok(tc) => serde_json::to_value(tc.describe())?,
            Err(e) => serde_json::json!({ "spec": spec, "error": e.to_string() }),
        };
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        match &resolved {
            Ok(tc) => print_description(&tc.describe()),
            Err(_) => print_spec(&spec),
        }
    }
    resolved.map(drop).map_err(Into::into)
}

fn print_config(config: &Config) {
    match &config.file {
        Some(file) => println!("configuration    : {}", file.display()),
        None => println!("configuration    : defaults (no moltn.toml)"),
    }
    println!("pages            : {}", config.pages_dir().display());
    println!("generated types  : {}", config.typescript().display());
    println!(
        "moltn dev        : cargo {}, ports {} (app) and {} (Vite)",
        config.dev.cargo.join(" "),
        config.dev.port,
        config.dev.vite_port
    );
    let watch: Vec<String> = config
        .dev
        .watch
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    println!("watched          : {}", watch.join(", "));
}

fn print_description(d: &Description) {
    println!("frontend         : {}", d.dir.display());
    for (label, tool, req) in [
        ("runtime         ", &d.runtime, &d.spec.runtime_req),
        ("package manager ", &d.package_manager, &d.spec.pm_req),
    ] {
        let req = req
            .as_ref()
            .map(|r| format!(", required: {r}"))
            .unwrap_or_default();
        println!(
            "{label} : {} {} ({}{req})",
            tool.name,
            tool.version,
            tool.path.display()
        );
    }
    println!("version manager  : {:?}", d.version_manager);
    print_spec_details(&d.spec);
}

fn print_spec(spec: &moltn_toolchain::ToolchainSpec) {
    let req = |r: &Option<moltn_toolchain::Requirement>| {
        r.as_ref().map(|r| format!(" {r}")).unwrap_or_default()
    };
    println!(
        "expected runtime         : {}{}",
        spec.runtime,
        req(&spec.runtime_req)
    );
    println!(
        "expected package manager : {}{}",
        spec.package_manager,
        req(&spec.pm_req)
    );
    print_spec_details(spec);
}

fn print_spec_details(spec: &moltn_toolchain::ToolchainSpec) {
    println!("decisions:");
    for source in &spec.sources {
        let path = source
            .path
            .as_ref()
            .map(|p| format!(" ({})", p.display()))
            .unwrap_or_default();
        println!("  - {} ← {}{path}", source.decision, source.source);
    }
    if !spec.warnings.is_empty() {
        println!("warnings:");
        for warning in &spec.warnings {
            println!("  - {warning}");
        }
    }
}
