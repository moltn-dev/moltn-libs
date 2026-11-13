//! Routeur, loaders `#[page]`, props partagées : tout ce qui demande axum, veer et ts-rs
//! (feature `runtime`). Réexporté à la racine du crate.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use axum::Router;
use axum::routing::{MethodRouter, get};

pub use moltn_pages_macros::page;

use crate::PageTable;
use crate::typescript;

/// Loader registered by `#[page]` (used by the macro).
#[doc(hidden)]
pub struct Loader {
    pub page: &'static str,
    pub name: &'static str,
    /// Names extracted from `Path(id)` / `Path((a, b))`, `None` if they cannot be inferred.
    pub params: Option<&'static [&'static str]>,
    pub file: &'static str,
    pub line: u32,
    pub route: fn() -> LoaderRoute,
    /// TypeScript type of the props: declarations added to the table, name returned.
    pub props: fn(&mut typescript::Decls) -> String,
}

inventory::collect!(Loader);

/// Shared props (signed-in user, flash messages…), registered by [`shared_props!`].
#[doc(hidden)]
pub struct SharedProps {
    pub props: fn(&mut typescript::Decls) -> String,
}

inventory::collect!(SharedProps);

/// Declare the type of the props shared by every page (veer's `InertiaConfig::share`).
/// The generated TypeScript module exposes it as `SharedProps` and registers it in Inertia's
/// global typing (`usePage().props`).
///
/// ```ignore
/// #[derive(serde::Serialize, ts_rs::TS)]
/// struct Shared { auth: Option<User> }
/// moltn_pages::shared_props!(Shared);
/// ```
#[macro_export]
macro_rules! shared_props {
    ($ty:ty) => {
        $crate::__private::inventory::submit! {
            $crate::SharedProps { props: $crate::__private::ts_type::<$ty> }
        }
    };
}

/// A loader's handler, with or without state (used by the macro).
#[doc(hidden)]
pub enum LoaderRoute {
    /// No `State` extractor: fits any `Router<S>`.
    Stateless(Box<MethodRouter<()>>),
    /// `State<T>`: only in a `Router<T>`.
    Stateful {
        state: TypeId,
        state_name: &'static str,
        route: Box<dyn Any + Send>,
    },
}

impl LoaderRoute {
    #[doc(hidden)]
    pub fn stateless(route: MethodRouter<()>) -> Self {
        Self::Stateless(Box::new(route))
    }

    #[doc(hidden)]
    pub fn stateful<T: Clone + Send + Sync + 'static>(route: MethodRouter<T>) -> Self {
        Self::Stateful {
            state: TypeId::of::<T>(),
            state_name: std::any::type_name::<T>(),
            route: Box::new(route),
        }
    }
}

/// Mismatches between pages and loaders, all listed at once.
#[derive(Debug, thiserror::Error)]
#[error("invalid moltn pages:\n{}", problems.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n"))]
pub struct PagesError {
    /// One message per problem.
    pub problems: Vec<String>,
}

/// Build the pages router and check at startup that pages and loaders match:
/// dynamic page without a loader, loader without a page, mismatched parameters, incompatible state.
///
/// A page with no parameter and no loader is rendered with empty props.
pub fn router<S>(table: &'static PageTable) -> Result<Router<S>, PagesError>
where
    S: Clone + Send + Sync + 'static,
{
    let pages = table.pages;
    let mut problems = Vec::new();
    let mut loaders: BTreeMap<&str, &Loader> = BTreeMap::new();
    for loader in inventory::iter::<Loader> {
        let at = format!("{} ({}:{})", loader.name, loader.file, loader.line);
        match pages.iter().find(|p| p.component == loader.page) {
            None => problems.push(format!(
                "loader {at} targets page \"{}\", which does not exist (expected file: pages/{}.vue, .tsx, .jsx or .svelte)",
                loader.page, loader.page
            )),
            Some(page) => {
                if let Some(params) = loader.params {
                    // `[animeId]` côté fichier, `Path(anime_id)` côté Rust : même paramètre.
                    let normalized = |names: &[&str]| {
                        let mut names: Vec<String> = names
                            .iter()
                            .map(|n| n.replace('_', "").to_ascii_lowercase())
                            .collect();
                        names.sort_unstable();
                        names
                    };
                    let (expected, got) = (page.params.to_vec(), params.to_vec());
                    if normalized(&expected) != normalized(&got) {
                        problems.push(format!(
                            "loader {at} extracts Path({}) but {} defines [{}]",
                            got.join(", "),
                            page.file,
                            expected.join("], [")
                        ));
                    }
                }
                if let Some(other) = loaders.insert(loader.page, loader) {
                    problems.push(format!(
                        "two loaders for page \"{}\": {} ({}:{}) and {at}",
                        loader.page, other.name, other.file, other.line
                    ));
                }
            }
        }
    }

    let mut router = Router::new();
    for page in pages {
        let route: MethodRouter<S> = match loaders.get(page.component) {
            Some(loader) => match (loader.route)() {
                LoaderRoute::Stateless(route) => route.with_state(()),
                LoaderRoute::Stateful {
                    state,
                    state_name,
                    route,
                } => match route.downcast::<MethodRouter<S>>() {
                    Ok(route) if state == TypeId::of::<S>() => *route,
                    _ => {
                        problems.push(format!(
                                "loader {} ({}:{}) reads State<{state_name}> but the application uses {}; \
                                 for a sub-state (FromRef), write #[page(\"{}\", state = {})]",
                                loader.name,
                                loader.file,
                                loader.line,
                                std::any::type_name::<S>(),
                                page.component,
                                std::any::type_name::<S>()
                            ));
                        continue;
                    }
                },
            },
            None if !page.params.is_empty() => {
                problems.push(format!(
                    "page {} has parameters ([{}]) but no loader: add #[moltn_pages::page(\"{}\")] to a handler",
                    page.file,
                    page.params.join("], ["),
                    page.component
                ));
                continue;
            }
            None => {
                let component = page.component;
                get(move |inertia: veer::Inertia| async move {
                    inertia.render(component, serde_json::json!({}))
                })
            }
        };
        for path in page.routes {
            router = router.route(path, route.clone());
        }
    }

    if !problems.is_empty() {
        return Err(PagesError { problems });
    }
    typescript::sync_typescript(table);
    Ok(router)
}

/// For a `#[test]`: fails with the list of problems found by [`router`].
///
/// ```ignore
/// #[test]
/// fn pages_are_valid() {
///     moltn_pages::assert_valid::<AppState>(moltn_pages::pages!());
/// }
/// ```
#[track_caller]
pub fn assert_valid<S: Clone + Send + Sync + 'static>(table: &'static PageTable) {
    if let Err(e) = router::<S>(table) {
        panic!("{e}");
    }
}

#[doc(hidden)]
pub mod __private {
    pub use axum::response::{IntoResponse, Response};
    pub use axum::routing::get;
    pub use inventory;
    pub use veer::Inertia;

    pub use crate::typescript::ts_type;

    /// Inertia render of a loader's result; `ssr = false` leaves the page to the browser.
    pub fn render<P: serde::Serialize, E: IntoResponse>(
        inertia: Inertia,
        component: &str,
        result: Result<P, E>,
        ssr: bool,
    ) -> Response {
        match result {
            Ok(props) if ssr => inertia.render(component, props).into_response(),
            Ok(props) => inertia.render(component, props).no_ssr().into_response(),
            Err(error) => error.into_response(),
        }
    }
}
