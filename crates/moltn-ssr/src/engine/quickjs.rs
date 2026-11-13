//! Moteur QuickJS (feature `quickjs`, par défaut) : léger, compile partout, sans `Intl`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rquickjs::{CatchResultExt, Context, Ctx, Function, Module, Runtime, Value};

use super::{EngineLimits, JsEngine, LOAD_TIMEOUT, PRELUDE, log_from_js};
use crate::{SsrError, SsrOutput};

pub(crate) struct QuickJs {
    // Déclaré avant `_rt` : le contexte doit être détruit avant le runtime.
    ctx: Context,
    _rt: Runtime,
    epoch: Instant,
    deadline_ms: Arc<AtomicU64>,
}

impl JsEngine for QuickJs {
    fn load(bundle: &str, limits: &EngineLimits) -> Result<Self, SsrError> {
        let rt = Runtime::new().map_err(load_error)?;
        rt.set_memory_limit(limits.memory_limit);
        let epoch = Instant::now();
        let deadline_ms = Arc::new(AtomicU64::new(millis(LOAD_TIMEOUT)));
        let deadline = deadline_ms.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            millis(epoch.elapsed()) > deadline.load(Ordering::Relaxed)
        })));
        let ctx = Context::full(&rt).map_err(load_error)?;
        ctx.with(|ctx| -> Result<(), SsrError> {
            let log = Function::new(ctx.clone(), log_from_js).map_err(load_error)?;
            ctx.globals().set("__moltnLog", log).map_err(load_error)?;
            ctx.eval::<(), _>(PRELUDE)
                .catch(&ctx)
                .map_err(|e| SsrError::Load(format!("prelude: {e}")))?;
            Module::evaluate(ctx.clone(), "ssr.js", bundle)
                .catch(&ctx)
                .and_then(|p| p.finish::<()>().catch(&ctx))
                .map_err(|e| SsrError::Load(e.to_string()))?;
            let render: Value = ctx.globals().get("__moltnRender").map_err(load_error)?;
            if !render.is_function() {
                return Err(SsrError::MissingRender);
            }
            Ok(())
        })?;
        deadline_ms.store(u64::MAX, Ordering::Relaxed);
        Ok(Self {
            ctx,
            _rt: rt,
            epoch,
            deadline_ms,
        })
    }

    fn render(&mut self, page_json: &str, timeout: Duration) -> Result<SsrOutput, SsrError> {
        let start = self.epoch.elapsed();
        self.deadline_ms
            .store(millis(start.saturating_add(timeout)), Ordering::Relaxed);
        let epoch = self.epoch;
        let in_time = move || epoch.elapsed() - start < timeout;
        let result = self.ctx.with(|ctx| {
            let result = render_in(&ctx, page_json, &in_time);
            // Rendu en échec avant la purge : rien ne doit rester pour la requête suivante.
            while in_time() && ctx.execute_pending_job() {}
            result
        });
        self.deadline_ms.store(u64::MAX, Ordering::Relaxed);
        // Après interruption, rquickjs renvoie une erreur de promesse bloquée et non
        // « interrupted » : seul le temps écoulé dit qu'il s'agit d'un dépassement.
        if self.epoch.elapsed() - start >= timeout {
            return Err(SsrError::Timeout(timeout));
        }
        result
    }
}

fn render_in(
    ctx: &Ctx<'_>,
    page_json: &str,
    in_time: &dyn Fn() -> bool,
) -> Result<SsrOutput, SsrError> {
    let take_errors: Function = ctx.globals().get("__moltnTakeErrors").map_err(js_error)?;
    take_errors.call::<_, ()>(()).map_err(js_error)?;

    let page = ctx
        .json_parse(page_json)
        .catch(ctx)
        .map_err(|e| js_message(e.to_string()))?;
    let render: Function = ctx.globals().get("__moltnRender").map_err(js_error)?;
    let returned: Value = render
        .call((page,))
        .catch(ctx)
        .map_err(|e| js_message(e.to_string()))?;
    let output = match returned.as_promise() {
        // Boucle maison plutôt que `Promise::finish` : une chaîne sans fin de microtâches
        // (chacune en programme une autre) n'est pas arrêtée de façon fiable par le handler
        // d'interruption, qui n'est sondé que de loin en loin. Ici le délai est vérifié
        // entre deux microtâches.
        Some(promise) => loop {
            if let Some(settled) = promise.result::<Value>() {
                break settled.catch(ctx).map_err(|e| js_message(e.to_string()))?;
            }
            if !in_time() {
                // `render` en fait un `Timeout` (temps écoulé).
                return Err(SsrError::Js("render interrupted".into()));
            }
            if !ctx.execute_pending_job() {
                return Err(SsrError::Js(
                    "the render awaits a promise that never resolves".into(),
                ));
            }
        },
        None => returned,
    };

    // Microtâches encore en file une fois la promesse résolue (ex. un `nextTick`) : elles
    // font partie de ce rendu, et leurs `console.error` aussi.
    while in_time() && ctx.execute_pending_job() {}

    let errors: Vec<String> = take_errors.call(()).map_err(js_error)?;
    if !errors.is_empty() {
        return Err(js_message(errors.join("\n")));
    }

    let object = output
        .as_object()
        .ok_or_else(|| SsrError::InvalidOutput("the render did not return an object".into()))?;
    let head: Vec<String> = object
        .get("head")
        .map_err(|e| SsrError::InvalidOutput(format!("`head` must be an array of strings: {e}")))?;
    let body: String = object
        .get("body")
        .map_err(|e| SsrError::InvalidOutput(format!("`body` must be a string: {e}")))?;
    Ok(SsrOutput { head, body })
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

fn load_error(e: rquickjs::Error) -> SsrError {
    SsrError::Load(e.to_string())
}

fn js_error(e: rquickjs::Error) -> SsrError {
    js_message(e.to_string())
}

fn js_message(message: String) -> SsrError {
    if message.contains("out of memory") {
        SsrError::OutOfMemory
    } else {
        SsrError::Js(message)
    }
}
