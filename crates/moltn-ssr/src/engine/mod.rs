//! Moteur JS derrière un trait : QuickJS (feature `quickjs`) et V8 (feature `v8`).

use std::time::Duration;

use crate::{SsrError, SsrOutput};

#[cfg(feature = "quickjs")]
pub(crate) mod quickjs;
#[cfg(feature = "v8")]
pub(crate) mod v8;

/// Limites appliquées à chaque moteur.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EngineLimits {
    pub memory_limit: usize,
}

/// Un moteur qui exécute un bundle définissant `globalThis.__moltnRender`.
/// Une instance vit et meurt sur un seul thread : pas besoin de `Send`.
///
/// Un rendu est fini quand la file de microtâches est vide : ce qu'elle exécute encore après
/// la résolution de la promesse (un `console.error` tardif, par exemple) compte pour ce rendu
/// et ne déborde jamais sur le suivant.
pub(crate) trait JsEngine: Sized + 'static {
    /// Évalue le bundle une fois ; échoue si `__moltnRender` n'est pas défini.
    fn load(bundle: &str, limits: &EngineLimits) -> Result<Self, SsrError>;
    /// Appelle `__moltnRender(page)`, attend la promesse, respecte le délai.
    fn render(&mut self, page_json: &str, timeout: Duration) -> Result<SsrOutput, SsrError>;
}

const PRELUDE: &str = include_str!("../prelude.js");
/// Un bundle qui boucle à son évaluation ne doit pas bloquer le démarrage.
const LOAD_TIMEOUT: Duration = Duration::from_secs(10);

fn log_from_js(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "moltn_ssr::js", "{message}"),
        "warn" => tracing::warn!(target: "moltn_ssr::js", "{message}"),
        "debug" => tracing::debug!(target: "moltn_ssr::js", "{message}"),
        _ => tracing::info!(target: "moltn_ssr::js", "{message}"),
    }
}
