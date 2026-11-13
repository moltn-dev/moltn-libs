//! Moteur V8 (feature `v8`) : liaisons `v8` de Deno, données ICU comprises (`Intl`).
//!
//! Deux différences de fond avec QuickJS, toutes deux traitées ici :
//! - V8 n'a pas de point d'interruption à sonder : le délai est tenu par un thread de garde
//!   qui appelle `terminate_execution` ;
//! - un tas plein **abat le processus** par défaut : le rappel « proche de la limite »
//!   interrompt le rendu et accorde la marge nécessaire pour dérouler la pile.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use super::{EngineLimits, JsEngine, LOAD_TIMEOUT, PRELUDE, log_from_js};
use crate::{SsrError, SsrOutput};

/// Plateforme V8 et données ICU : une fois par processus.
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Sans ces données, `Intl` ne connaît que la locale racine.
        if v8::icu::set_common_data_78(deno_core_icudata::ICU_DATA).is_err() {
            tracing::warn!("ICU data rejected by V8: Intl will only know the root locale");
        }
        v8::V8::initialize_platform(v8::new_default_platform(0, false).make_shared());
        v8::V8::initialize();
    });
}

/// Ce que lit le rappel « proche de la limite du tas ».
struct Heap {
    exhausted: AtomicBool,
    isolate: v8::IsolateHandle,
}

unsafe extern "C" fn near_heap_limit(data: *mut c_void, current: usize, _initial: usize) -> usize {
    // SAFETY: `data` pointe sur le `Heap` de l'instance `V8`, détruit après son isolat.
    let heap = unsafe { &*data.cast::<Heap>() };
    heap.exhausted.store(true, Ordering::Relaxed);
    heap.isolate.terminate_execution();
    // Marge pour que l'interruption se propage au lieu d'un arrêt brutal du processus.
    current.saturating_mul(2)
}

pub(crate) struct V8 {
    // Ordre de destruction : les poignées avant l'isolat, `heap` après (son rappel le lit).
    context: v8::Global<v8::Context>,
    render: v8::Global<v8::Function>,
    take_errors: v8::Global<v8::Function>,
    isolate: v8::OwnedIsolate,
    heap: Box<Heap>,
    /// `Some(échéance)` arme le thread de garde, `None` le désarme.
    watchdog: Sender<Option<Instant>>,
    timed_out: Arc<AtomicBool>,
}

/// Thread de garde : interrompt l'isolat si l'échéance passe avant le désarmement.
fn watchdog(
    isolate: v8::IsolateHandle,
    timed_out: Arc<AtomicBool>,
) -> Result<Sender<Option<Instant>>, SsrError> {
    let (tx, rx) = channel::<Option<Instant>>();
    std::thread::Builder::new()
        .name("moltn-ssr-watchdog".into())
        .spawn(move || {
            let mut deadline: Option<Instant> = None;
            loop {
                let message = match deadline {
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                };
                match message {
                    Ok(next) => deadline = next,
                    Err(RecvTimeoutError::Timeout) => {
                        timed_out.store(true, Ordering::Relaxed);
                        isolate.terminate_execution();
                        deadline = None;
                    }
                    Err(RecvTimeoutError::Disconnected) => return, // moteur détruit
                }
            }
        })
        .map_err(|e| SsrError::Load(format!("creating the watchdog thread: {e}")))?;
    Ok(tx)
}

impl JsEngine for V8 {
    fn load(bundle: &str, limits: &EngineLimits) -> Result<Self, SsrError> {
        init();
        let mut params = v8::CreateParams::default();
        if limits.memory_limit > 0 {
            params = params.heap_limits(0, limits.memory_limit);
        }
        let mut isolate = v8::Isolate::new(params);
        let heap = Box::new(Heap {
            exhausted: AtomicBool::new(false),
            isolate: isolate.thread_safe_handle(),
        });
        isolate.add_near_heap_limit_callback(near_heap_limit, (&raw const *heap).cast_mut().cast());
        let timed_out = Arc::new(AtomicBool::new(false));
        let watchdog = watchdog(isolate.thread_safe_handle(), timed_out.clone())?;

        // Un bundle qui boucle à son évaluation ne doit pas bloquer le démarrage.
        let _ = watchdog.send(Instant::now().checked_add(LOAD_TIMEOUT));
        let loaded = {
            v8::scope!(let handles, &mut isolate);
            let context = v8::Context::new(handles, Default::default());
            let scope = &mut v8::ContextScope::new(handles, context);
            load_in(scope, bundle).map(|(render, take_errors)| {
                (
                    v8::Global::new(scope, context),
                    v8::Global::new(scope, render),
                    v8::Global::new(scope, take_errors),
                )
            })
        };
        let _ = watchdog.send(None);
        if heap.exhausted.load(Ordering::Relaxed) {
            return Err(SsrError::OutOfMemory);
        }
        if timed_out.load(Ordering::Relaxed) {
            return Err(SsrError::Load(format!(
                "the bundle did not finish loading within {LOAD_TIMEOUT:?}"
            )));
        }
        let (context, render, take_errors) = loaded?;
        Ok(Self {
            context,
            render,
            take_errors,
            isolate,
            heap,
            watchdog,
            timed_out,
        })
    }

    fn render(&mut self, page_json: &str, timeout: Duration) -> Result<SsrOutput, SsrError> {
        // Échéance tombée juste après la fin du rendu précédent : l'interruption est restée
        // en attente dans l'isolat, il n'est plus utilisable.
        if self.timed_out.load(Ordering::Relaxed) {
            return Err(SsrError::Timeout(timeout));
        }
        // Délai sans fin (`Duration::MAX`) : pas d'échéance représentable, donc pas de garde.
        let _ = self.watchdog.send(Instant::now().checked_add(timeout));
        let result = {
            v8::scope!(let handles, &mut self.isolate);
            let context = v8::Local::new(handles, &self.context);
            let scope = &mut v8::ContextScope::new(handles, context);
            let render = v8::Local::new(scope, &self.render);
            let take_errors = v8::Local::new(scope, &self.take_errors);
            render_in(scope, render, take_errors, page_json)
        };
        let _ = self.watchdog.send(None);
        // Après une interruption, V8 ne dit pas pourquoi : ces drapeaux, si.
        if self.heap.exhausted.load(Ordering::Relaxed) {
            return Err(SsrError::OutOfMemory);
        }
        if self.timed_out.load(Ordering::Relaxed) {
            return Err(SsrError::Timeout(timeout));
        }
        result
    }
}

type Scope<'s, 'i> = v8::PinScope<'s, 'i>;

fn string<'s>(scope: &Scope<'s, '_>, text: &str) -> Result<v8::Local<'s, v8::String>, SsrError> {
    v8::String::new(scope, text)
        .ok_or_else(|| SsrError::Js("string too large for the JavaScript engine".into()))
}

/// Message d'une valeur levée : la pile d'une `Error` (qui, en V8, commence par le message).
fn describe(scope: &Scope<'_, '_>, value: v8::Local<'_, v8::Value>) -> String {
    let stack = v8::Local::<v8::Object>::try_from(value)
        .ok()
        .and_then(|object| object.get(scope, string(scope, "stack").ok()?.into()))
        .filter(|stack| stack.is_string());
    stack.unwrap_or(value).to_rust_string_lossy(scope)
}

/// Erreur d'une opération qui a échoué sous un `TryCatch`.
fn caught(tc: &v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>) -> String {
    match tc.exception() {
        Some(exception) => describe(tc, exception),
        None => "execution interrupted".into(),
    }
}

fn global_function<'s>(
    scope: &Scope<'s, '_>,
    name: &str,
) -> Result<Option<v8::Local<'s, v8::Function>>, SsrError> {
    let global = scope.get_current_context().global(scope);
    let value = global.get(scope, string(scope, name)?.into());
    Ok(value.and_then(|v| v8::Local::<v8::Function>::try_from(v).ok()))
}

fn script_origin<'s>(
    scope: &Scope<'s, '_>,
    name: &str,
    module: bool,
) -> Result<v8::ScriptOrigin<'s>, SsrError> {
    let name = string(scope, name)?.into();
    Ok(v8::ScriptOrigin::new(
        scope, name, 0, 0, false, 0, None, false, false, module, None,
    ))
}

/// `__moltnLog(level, message)` appelé par le prélude.
#[allow(clippy::needless_pass_by_value)] // signature imposée par V8
fn log_callback(
    scope: &mut Scope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    _: v8::ReturnValue,
) {
    let level = args.get(0).to_rust_string_lossy(scope);
    let message = args.get(1).to_rust_string_lossy(scope);
    log_from_js(level, message);
}

/// Prélude, puis le bundle comme module ES ; renvoie `__moltnRender` et `__moltnTakeErrors`.
fn load_in<'s>(
    scope: &mut Scope<'s, '_>,
    bundle: &str,
) -> Result<(v8::Local<'s, v8::Function>, v8::Local<'s, v8::Function>), SsrError> {
    let load = SsrError::Load;
    let log = v8::Function::new(scope, log_callback)
        .ok_or_else(|| load("creating the log function".into()))?;
    let global = scope.get_current_context().global(scope);
    let key = string(scope, "__moltnLog")?.into();
    global.set(scope, key, log.into());

    {
        v8::tc_scope!(let tc, scope);
        let source = string(tc, PRELUDE)?;
        let origin = script_origin(tc, "prelude.js", false)?;
        v8::Script::compile(tc, source, Some(&origin))
            .and_then(|script| script.run(tc))
            .ok_or_else(|| load(format!("prelude: {}", caught(tc))))?;

        let source = string(tc, bundle)?;
        let origin = script_origin(tc, "ssr.js", true)?;
        let mut source = v8::script_compiler::Source::new(source, Some(&origin));
        let module =
            v8::script_compiler::compile_module(tc, &mut source).ok_or_else(|| load(caught(tc)))?;
        // Le bundle n'a aucun import : ce rappel n'est jamais appelé.
        module
            .instantiate_module(tc, |_, _, _, _| None)
            .ok_or_else(|| load(caught(tc)))?;
        module.evaluate(tc).ok_or_else(|| load(caught(tc)))?;
        tc.perform_microtask_checkpoint();
        // Une exception à l'évaluation rejette la promesse du module au lieu d'être levée.
        if module.get_status() == v8::ModuleStatus::Errored {
            return Err(load(describe(tc, module.get_exception())));
        }
    }

    let render = global_function(scope, "__moltnRender")?.ok_or(SsrError::MissingRender)?;
    let take_errors = global_function(scope, "__moltnTakeErrors")?
        .ok_or_else(|| load("prelude: __moltnTakeErrors is missing".into()))?;
    Ok((render, take_errors))
}

fn render_in(
    scope: &mut Scope<'_, '_>,
    render: v8::Local<'_, v8::Function>,
    take_errors: v8::Local<'_, v8::Function>,
    page_json: &str,
) -> Result<SsrOutput, SsrError> {
    v8::tc_scope!(let tc, scope);
    let js = |tc: &v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>| SsrError::Js(caught(tc));
    let undefined: v8::Local<'_, v8::Value> = v8::undefined(tc).into();

    take_errors.call(tc, undefined, &[]).ok_or_else(|| js(tc))?;
    let json = string(tc, page_json)?;
    let page = v8::json::parse(tc, json).ok_or_else(|| js(tc))?;
    let returned = render.call(tc, undefined, &[page]).ok_or_else(|| js(tc))?;
    // Vide toute la file de microtâches : le rendu est fini quand elle l'est, et rien ne
    // déborde sur la requête suivante.
    tc.perform_microtask_checkpoint();

    let output = match v8::Local::<v8::Promise>::try_from(returned) {
        Ok(promise) => match promise.state() {
            v8::PromiseState::Fulfilled => promise.result(tc),
            v8::PromiseState::Rejected => {
                return Err(SsrError::Js(describe(tc, promise.result(tc))));
            }
            v8::PromiseState::Pending => {
                return Err(SsrError::Js(
                    "the render awaits a promise that never resolves".into(),
                ));
            }
        },
        Err(_) => returned,
    };

    let errors = take_errors.call(tc, undefined, &[]).ok_or_else(|| js(tc))?;
    let errors = strings(tc, errors).unwrap_or_default();
    if !errors.is_empty() {
        return Err(SsrError::Js(errors.join("\n")));
    }

    let object = v8::Local::<v8::Object>::try_from(output)
        .map_err(|_| SsrError::InvalidOutput("the render did not return an object".into()))?;
    let field = |name: &str| -> Result<Option<v8::Local<'_, v8::Value>>, SsrError> {
        Ok(object.get(tc, string(tc, name)?.into()))
    };
    let head = field("head")?
        .and_then(|head| strings(tc, head))
        .ok_or_else(|| SsrError::InvalidOutput("`head` must be an array of strings".into()))?;
    let body = field("body")?
        .filter(|body| body.is_string())
        .ok_or_else(|| SsrError::InvalidOutput("`body` must be a string".into()))?
        .to_rust_string_lossy(tc);
    Ok(SsrOutput { head, body })
}

/// Tableau JS de chaînes → `Vec<String>` ; `None` si ce n'en est pas un.
fn strings(scope: &Scope<'_, '_>, value: v8::Local<'_, v8::Value>) -> Option<Vec<String>> {
    let array = v8::Local::<v8::Array>::try_from(value).ok()?;
    (0..array.length())
        .map(|i| {
            let item = array.get_index(scope, i)?;
            item.is_string().then(|| item.to_rust_string_lossy(scope))
        })
        .collect()
}
