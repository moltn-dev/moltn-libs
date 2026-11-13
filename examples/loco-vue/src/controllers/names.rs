//! Contrôleur Loco classique : les mutations restent des routes Loco, à côté des pages moltn.

use axum::extract::State;
use axum::routing::post;
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use serde::Deserialize;
use veer::{Inertia, InertiaForm, InertiaResponse};

use crate::Names;

#[derive(Deserialize)]
pub struct NewName {
    name: String,
}

async fn add(
    inertia: Inertia,
    State(ctx): State<AppContext>,
    InertiaForm(form): InertiaForm<NewName>,
) -> InertiaResponse {
    if let Some(names) = ctx.shared_store.get::<Names>()
        && let Ok(mut names) = names.0.lock()
    {
        names.push(form.name);
    }
    inertia.redirect("/")
}

pub fn routes() -> Routes {
    Routes::new().prefix("names").add("/", post(add))
}
