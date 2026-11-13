//! The `moltn.ts` TypeScript module: props types (Rust structs through ts-rs), `Pages`,
//! shared props and `route()`. Written by the application itself, the only one that knows
//! its loaders' types: at startup in debug, or when `moltn gen` asks.

use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::ts_render::{RoutePage, Typed, render, shared_section};
use crate::{Loader, PageFile, PageTable, SharedProps};

/// Collected TypeScript declarations: Rust type → (name, declaration).
pub type Decls = HashMap<TypeId, (String, String)>;

/// TypeScript name of `T`; its declarations and its dependencies' go into `out`.
pub fn ts_type<T: ts_rs::TS + 'static + ?Sized>(out: &mut Decls) -> String {
    veer::bindings::collect_decls::<T>(out);
    T::ident(&ts_rs::Config::from_env())
}

/// Full content of `moltn.ts` for these pages and the binary's `#[page]` loaders.
///
/// ```
/// let pages = [moltn_pages::PageFile { file: "about.vue", component: "about", routes: &["/about"], params: &[] }];
/// let ts = moltn_pages::typescript(&pages);
/// assert!(ts.contains("'about': {}"));
/// assert!(ts.contains("export function route"));
/// ```
pub fn typescript(pages: &[PageFile]) -> String {
    let mut decls = Decls::new();
    let props: BTreeMap<String, String> = inventory::iter::<Loader>
        .into_iter()
        .map(|loader| (loader.page.to_string(), (loader.props)(&mut decls)))
        .collect();
    let shared: Vec<String> = inventory::iter::<SharedProps>
        .into_iter()
        .map(|s| (s.props)(&mut decls))
        .collect();

    // Ensemble trié : une même déclaration atteinte par deux types Rust (ex. `JsonValue`)
    // n'apparaît qu'une fois.
    let ordered: std::collections::BTreeSet<&(String, String)> = decls
        .values()
        .filter(|(name, _)| !name.is_empty())
        .collect();
    let mut declarations = String::new();
    for (_, decl) in ordered {
        let _ = writeln!(declarations, "export {decl}\n");
    }

    let typed = Typed {
        decls: declarations,
        props,
        shared: shared_section(&shared),
    };
    let mut routes: Vec<RoutePage<'_>> = pages
        .iter()
        .map(|p| RoutePage {
            component: p.component,
            routes: p.routes.to_vec(),
            params: p.params.to_vec(),
        })
        .collect();
    render(&typed, &mut routes)
}

/// Keep `moltn.ts` up to date, and answer `moltn gen`. [`crate::router`] calls it, so most
/// applications never do.
///
/// Call it yourself at the very top of `main` when the router is built late (after a
/// database connection, for instance): `moltn gen` then works without that setup.
///
/// With `MOLTN_GEN` set (by `moltn gen`): writes (`write`) or checks (`check`) the file,
/// then **exits the process**. Otherwise, in debug: rewrites the file if it changed.
/// In release: nothing.
///
/// ```
/// static TABLE: moltn_pages::PageTable = moltn_pages::PageTable { pages: &[], typescript: None };
/// moltn_pages::sync_typescript(&TABLE); // in an application: `moltn_pages::pages!()`
/// ```
pub fn sync_typescript(table: &PageTable) {
    let Some(path) = table.typescript else { return };
    let mode = std::env::var("MOLTN_GEN").ok();
    if mode.is_none() && !cfg!(debug_assertions) {
        return;
    }
    let content = typescript(table.pages);
    // Fins de ligne ignorées : git sous Windows (`autocrlf`) restitue le fichier en CRLF.
    let up_to_date =
        std::fs::read_to_string(path).is_ok_and(|current| current.replace("\r\n", "\n") == content);
    let write = || {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &content)
    };
    // Mode `moltn gen` : le processus ne sert qu'à produire le fichier. Codes de sortie lus
    // par la CLI : 0 à jour ou écrit, 1 périmé (--check), 2 écriture impossible.
    match mode.as_deref() {
        Some("check") => std::process::exit(if up_to_date { 0 } else { 1 }),
        Some(_) => std::process::exit(if up_to_date || write().is_ok() { 0 } else { 2 }),
        None if !up_to_date => match write() {
            Ok(()) => tracing::info!("TypeScript types updated: {path}"),
            Err(e) => tracing::warn!("could not write {path}: {e}"),
        },
        None => {}
    }
}
