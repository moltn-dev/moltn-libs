//! `build.rs` side: scanning the `pages/` folder and the route table.
//!
//! ```no_run
//! // in the `fn main` of the application's build.rs
//! moltn_pages::build::scan("web/src/pages").expect("pages/");
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Recognized page extensions.
pub const EXTENSIONS: [&str; 4] = ["vue", "tsx", "jsx", "svelte"];

/// A page's dynamic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Param {
    /// `[id]`: required.
    Required(String),
    /// `[[tab]]`: optional (last segment only).
    Optional(String),
    /// `[...slug]`: one or more segments (last segment only).
    CatchAll(String),
}

impl Param {
    /// Parameter name.
    pub fn name(&self) -> &str {
        match self {
            Self::Required(n) | Self::Optional(n) | Self::CatchAll(n) => n,
        }
    }
}

/// A page found in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedPage {
    /// Path relative to the pages folder, with `/` separators (`users/[id].vue`).
    pub file: String,
    /// Inertia component name: the path without extension (`users/[id]`).
    pub component: String,
    /// axum 0.8 routes (`/users/{id}`; two for an optional parameter).
    pub routes: Vec<String>,
    /// Dynamic parameters, in path order.
    pub params: Vec<Param>,
}

/// Problems found in the pages folder, all listed at once.
#[derive(Debug, thiserror::Error)]
#[error("invalid pages folder ({dir}):\n{}", problems.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n"))]
pub struct ScanError {
    /// Scanned folder.
    pub dir: String,
    /// One message per problem.
    pub problems: Vec<String>,
}

/// Scan `dir`, write the page table into `OUT_DIR` (for [`crate::pages!`]) and
/// ask Cargo to rerun `build.rs` when the folder changes. Call it from `build.rs`.
///
/// The TypeScript module (`moltn.ts`: props types and routes) is kept up to date by
/// the application in `generated/`, next to the pages folder (`web/src/generated/moltn.ts`
/// for `web/src/pages`).
pub fn scan(dir: impl AsRef<Path>) -> Result<Vec<ScannedPage>, ScanError> {
    let dir = dir.as_ref();
    let typescript = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(|root| crate::config::default_typescript(&Path::new(&root).join(dir)));
    scan_to(dir, typescript.as_deref())
}

/// Like [`scan`], with the folders taken from `moltn.toml` (see [`crate::config`]): the one
/// next to the application's `Cargo.toml` or in a parent folder. Without the file, same as
/// `scan("web/src/pages")`.
///
/// ```no_run
/// // in the `fn main` of the application's build.rs
/// moltn_pages::build::scan_project().expect("pages/");
/// ```
pub fn scan_project() -> Result<Vec<ScannedPage>, ScanError> {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default();
    let config = crate::config::Config::load(Path::new(&manifest)).map_err(|e| ScanError {
        dir: Path::new(&manifest).display().to_string(),
        problems: vec![e.to_string()],
    })?;
    // Seulement s'il existe : un chemin absent ferait relancer build.rs à chaque compilation.
    if let Some(file) = &config.file {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    scan_to(&config.pages_dir(), Some(&config.typescript()))
}

fn scan_to(dir: &Path, typescript: Option<&Path>) -> Result<Vec<ScannedPage>, ScanError> {
    println!("cargo:rerun-if-changed={}", dir.display());
    let pages = scan_dir(dir)?;
    let error = |message: String| ScanError {
        dir: dir.display().to_string(),
        problems: vec![message],
    };
    let out_dir = std::env::var_os("OUT_DIR")
        .ok_or_else(|| error("OUT_DIR is not set: call `scan` from build.rs".into()))?;
    let out = Path::new(&out_dir).join("moltn_pages.rs");
    std::fs::write(&out, table_rs(&pages, typescript))
        .map_err(|e| error(format!("writing {}: {e}", out.display())))?;
    Ok(pages)
}

/// Update the pages and routes of an existing `moltn.ts` right away, from a scan of `dir`,
/// without compiling the application: props types already in the file are kept, and a new
/// page gets `{}` until the application rewrites the full file at its next start.
///
/// Returns `true` if the file changed. Used by `moltn dev` when a page is created, moved
/// or removed.
///
/// ```no_run
/// let changed = moltn_pages::build::refresh_typescript("web/src/pages", "web/src/generated/moltn.ts")?;
/// # let _ = changed;
/// # Ok::<(), moltn_pages::build::ScanError>(())
/// ```
pub fn refresh_typescript(
    dir: impl AsRef<Path>,
    typescript: impl AsRef<Path>,
) -> Result<bool, ScanError> {
    let (dir, typescript) = (dir.as_ref(), typescript.as_ref());
    let pages = scan_dir(dir)?;
    // Fins de ligne ignorées : git sous Windows (`autocrlf`) restitue le fichier en CRLF.
    let existing = std::fs::read_to_string(typescript)
        .ok()
        .map(|text| text.replace("\r\n", "\n"));
    let typed = existing
        .as_deref()
        .and_then(crate::ts_render::Typed::parse)
        .unwrap_or_else(crate::ts_render::Typed::empty);
    let mut routes: Vec<crate::ts_render::RoutePage<'_>> = pages
        .iter()
        .map(|p| crate::ts_render::RoutePage {
            component: &p.component,
            routes: p.routes.iter().map(String::as_str).collect(),
            params: p.params.iter().map(Param::name).collect(),
        })
        .collect();
    let content = crate::ts_render::render(&typed, &mut routes);
    if existing.as_deref() == Some(content.as_str()) {
        return Ok(false);
    }
    let write = || -> std::io::Result<()> {
        if let Some(parent) = typescript.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(typescript, &content)
    };
    write().map_err(|e| ScanError {
        dir: dir.display().to_string(),
        problems: vec![format!("writing {}: {e}", typescript.display())],
    })?;
    Ok(true)
}

/// Scan `dir` without writing anything: sorted pages, or every problem found.
pub fn scan_dir(dir: impl AsRef<Path>) -> Result<Vec<ScannedPage>, ScanError> {
    let dir = dir.as_ref();
    let mut problems = Vec::new();
    let mut files = Vec::new();
    collect(dir, "", &mut files, &mut problems);
    files.sort();

    let mut pages = Vec::new();
    for file in files {
        match page(&file) {
            Ok(page) => pages.push(page),
            Err(problem) => problems.push(format!("{file}: {problem}")),
        }
    }

    // Deux fichiers pour la même route (aux noms de paramètres près) : axum refuserait.
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for page in &pages {
        for route in &page.routes {
            if let Some(other) = seen.insert(shape(route), &page.file) {
                problems.push(format!(
                    "{other} and {} produce the same route {route}",
                    page.file
                ));
            }
        }
    }

    if problems.is_empty() {
        Ok(pages)
    } else {
        Err(ScanError {
            dir: dir.display().to_string(),
            problems,
        })
    }
}

fn collect(dir: &Path, prefix: &str, files: &mut Vec<String>, problems: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => return problems.push(format!("reading {}: {e}", dir.display())),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // `_composants/`, `_Card.vue` : internes ; fichiers cachés ignorés aussi.
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let path = entry.path();
        if path.is_dir() {
            collect(&path, &rel, files, problems);
        } else if path
            .extension()
            .is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x))
        {
            files.push(rel);
        }
    }
}

/// `users/[[tab]].vue` → component, routes, parameters.
fn page(file: &str) -> Result<ScannedPage, String> {
    let component = file
        .rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_string();
    let segments: Vec<&str> = component.split('/').collect();
    let last = segments.len() - 1;
    let mut url: Vec<String> = Vec::new();
    let mut params: Vec<Param> = Vec::new();
    let mut optional = false;

    for (i, segment) in segments.iter().enumerate() {
        if segment.starts_with('(') && segment.ends_with(')') {
            continue; // groupe : n'apparaît pas dans l'URL
        }
        if i == last && *segment == "index" {
            continue;
        }
        let param = if let Some(name) = segment
            .strip_prefix("[[")
            .and_then(|s| s.strip_suffix("]]"))
        {
            Param::Optional(ident(name)?)
        } else if let Some(name) = segment
            .strip_prefix("[...")
            .and_then(|s| s.strip_suffix(']'))
        {
            Param::CatchAll(ident(name)?)
        } else if let Some(name) = segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Param::Required(ident(name)?)
        } else if segment.contains(['[', ']', '{', '}']) {
            return Err(format!(
                "invalid segment \"{segment}\" (expected: name, [param], [[param]] or [...param])"
            ));
        } else {
            url.push((*segment).to_string());
            continue;
        };
        if i != last && !matches!(param, Param::Required(_)) {
            return Err(format!(
                "\"{segment}\" is only allowed as the last segment (file name)"
            ));
        }
        if params.iter().any(|p| p.name() == param.name()) {
            return Err(format!("duplicate parameter \"{}\"", param.name()));
        }
        url.push(match &param {
            Param::Required(n) => format!("{{{n}}}"),
            Param::Optional(n) => {
                optional = true;
                format!("{{{n}}}")
            }
            Param::CatchAll(n) => format!("{{*{n}}}"),
        });
        params.push(param);
    }

    let full = format!("/{}", url.join("/"));
    let routes = if optional {
        let without = format!("/{}", url[..url.len() - 1].join("/"));
        vec![without, full]
    } else {
        vec![full]
    };
    Ok(ScannedPage {
        file: file.to_string(),
        component,
        routes,
        params,
    })
}

fn ident(name: &str) -> Result<String, String> {
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(name.to_string())
    } else {
        Err(format!(
            "invalid parameter name \"{name}\" (letters, digits, _)"
        ))
    }
}

/// Shape of a route without parameter names: `/users/{id}` ≡ `/users/{slug}`.
fn shape(route: &str) -> String {
    route
        .split('/')
        .map(|s| match s.strip_prefix('{') {
            Some(rest) if rest.starts_with('*') => "{*}",
            Some(_) => "{}",
            None => s,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Rust code of the table included by `pages!()`.
fn table_rs(pages: &[ScannedPage], typescript: Option<&Path>) -> String {
    let mut out = String::from(
        "// Generated by moltn-pages (build.rs). Do not edit.\n&::moltn_pages::PageTable {\n    pages: &[\n",
    );
    for page in pages {
        let list = |items: Vec<&str>| {
            items
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            out,
            "        ::moltn_pages::PageFile {{ file: {:?}, component: {:?}, routes: &[{}], params: &[{}] }},",
            page.file,
            page.component,
            list(page.routes.iter().map(String::as_str).collect()),
            list(page.params.iter().map(Param::name).collect()),
        );
    }
    let typescript = typescript.map_or("None".to_string(), |p| {
        format!("Some({:?})", p.to_string_lossy().replace('\\', "/"))
    });
    let _ = write!(out, "    ],\n    typescript: {typescript},\n}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routes(file: &str) -> Vec<String> {
        page(file).unwrap().routes
    }

    #[test]
    fn conventions() {
        assert_eq!(routes("index.vue"), ["/"]);
        assert_eq!(routes("about.vue"), ["/about"]);
        assert_eq!(routes("users/index.vue"), ["/users"]);
        assert_eq!(routes("users/[id].vue"), ["/users/{id}"]);
        assert_eq!(routes("users/[[tab]].vue"), ["/users", "/users/{tab}"]);
        assert_eq!(routes("docs/[...slug].vue"), ["/docs/{*slug}"]);
        assert_eq!(routes("(admin)/stats.vue"), ["/stats"]);
        assert_eq!(
            routes("teams/[team]/members/[id].svelte"),
            ["/teams/{team}/members/{id}"]
        );
        assert_eq!(page("users/[id].vue").unwrap().component, "users/[id]");
    }

    #[test]
    fn invalid_segments() {
        assert!(page("[[tab]]/x.vue").unwrap_err().contains("last segment"));
        assert!(page("[...all]/x.vue").unwrap_err().contains("last segment"));
        assert!(page("a/[id]/b/[id].vue").unwrap_err().contains("duplicate"));
        assert!(page("[a-b].vue").unwrap_err().contains("invalid"));
        assert!(page("x{y}.vue").unwrap_err().contains("invalid"));
    }
}
