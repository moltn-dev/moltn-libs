//! `#[page("users/[id]")]`: declares the loader of a `moltn-pages` page.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{FnArg, Ident, ItemFn, LitStr, Pat, Token, Type, parse_macro_input};

struct Args {
    page: LitStr,
    state: Option<Type>,
    ssr: bool,
}

impl Parse for Args {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let page: LitStr = input.parse()?;
        let mut state = None;
        let mut ssr = true;
        while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if key == "state" {
                state = Some(input.parse()?);
            } else if key == "ssr" {
                ssr = input.parse::<syn::LitBool>()?.value;
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "unknown option; expected: state = Type, ssr = false",
                ));
            }
        }
        Ok(Self { page, state, ssr })
    }
}

/// Declare a page's loader: an ordinary axum handler (usual extractors)
/// returning `Result<Props, E>`, with `Props: Serialize + ts_rs::TS` and `E: IntoResponse`.
///
/// ```ignore
/// #[moltn_pages::page("users/[id]")]
/// async fn user_show(Path(id): Path<u32>, State(app): State<AppState>) -> Result<UserProps, AppError> {
///     Ok(UserProps { user: app.users.find(id).await? })
/// }
/// ```
///
/// The state is inferred from the `State<T>` extractor. For a sub-state (`FromRef`), name
/// the application state: `#[page("users/[id]", state = AppState)]`.
///
/// A page that only makes sense in the browser (video player, heavy editor) skips
/// server-side rendering with `#[page("watch/[id]", ssr = false)]`.
#[proc_macro_attribute]
pub fn page(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as Args);
    let func = parse_macro_input!(item as ItemFn);
    expand(args, func)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(args: Args, func: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "a #[page] loader must be an `async fn`",
        ));
    }
    let name = &func.sig.ident;
    let page = &args.page;
    let ssr = args.ssr;
    let mut types = Vec::new();
    let mut state = args.state.clone();
    let mut params: Option<Vec<String>> = Some(Vec::new());

    for input in &func.sig.inputs {
        let FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "a #[page] loader does not take `self`",
            ));
        };
        let ty = &*arg.ty;
        match last_segment(ty).as_deref() {
            Some("State") if args.state.is_none() => state = generic_arg(ty),
            Some("Path") => match path_names(&arg.pat) {
                Some(names) => {
                    if let Some(p) = &mut params {
                        p.extend(names);
                    }
                }
                None => params = None,
            },
            // `Option<Path<…>>` (page à paramètre facultatif) : noms non vérifiables.
            _ if quote!(#ty).to_string().contains("Path") => params = None,
            _ => {}
        }
        types.push(ty.clone());
    }

    // `Result<Props, E>` : Props est exporté en TypeScript (ts-rs).
    let props = match &func.sig.output {
        syn::ReturnType::Type(_, ty) if last_segment(ty).as_deref() == Some("Result") => generic_arg(ty),
        _ => None,
    }
    .ok_or_else(|| {
        syn::Error::new_spanned(
            &func.sig.output,
            "a #[page] loader returns `Result<Props, E>` (Props: Serialize + ts_rs::TS, E: IntoResponse)",
        )
    })?;

    let wrapper = format_ident!("__moltn_page_{}", name);
    let route_fn = format_ident!("__moltn_route_{}", name);
    let args_idents: Vec<Ident> = (0..types.len())
        .map(|i| Ident::new(&format!("__arg{i}"), Span::call_site()))
        .collect();
    let params_tokens = match params {
        Some(names) => quote! { ::core::option::Option::Some(&[#(#names),*]) },
        None => quote! { ::core::option::Option::None },
    };
    let route = match &state {
        Some(state) => quote! {
            ::moltn_pages::LoaderRoute::stateful::<#state>(::moltn_pages::__private::get(#wrapper))
        },
        None => quote! {
            ::moltn_pages::LoaderRoute::stateless(::moltn_pages::__private::get(#wrapper))
        },
    };

    Ok(quote! {
        #func

        #[doc(hidden)]
        #[allow(non_snake_case)]
        async fn #wrapper(
            __inertia: ::moltn_pages::__private::Inertia,
            #(#args_idents: #types),*
        ) -> ::moltn_pages::__private::Response {
            ::moltn_pages::__private::render(__inertia, #page, #name(#(#args_idents),*).await, #ssr)
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #route_fn() -> ::moltn_pages::LoaderRoute {
            #route
        }

        ::moltn_pages::__private::inventory::submit! {
            ::moltn_pages::Loader {
                page: #page,
                name: ::core::stringify!(#name),
                params: #params_tokens,
                file: ::core::file!(),
                line: ::core::line!(),
                route: #route_fn,
                props: ::moltn_pages::__private::ts_type::<#props>,
            }
        }
    })
}

fn last_segment(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// `State<AppState>` → `AppState`.
fn generic_arg(ty: &Type) -> Option<Type> {
    let Type::Path(p) = ty else { return None };
    let syn::PathArguments::AngleBracketed(args) = &p.path.segments.last()?.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    })
}

/// `Path(id)` → `["id"]`, `Path((a, b))` → `["a", "b"]`; any other shape → `None` (not checkable).
fn path_names(pat: &Pat) -> Option<Vec<String>> {
    let Pat::TupleStruct(ts) = pat else {
        return None;
    };
    let inner = ts.elems.first()?;
    let ident = |p: &Pat| match p {
        Pat::Ident(i) => Some(i.ident.to_string().trim_start_matches('_').to_string()),
        _ => None,
    };
    match inner {
        Pat::Tuple(t) => t.elems.iter().map(ident).collect(),
        other => ident(other).map(|n| vec![n]),
    }
}
