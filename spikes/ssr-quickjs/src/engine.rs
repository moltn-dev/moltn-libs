//! Spike : pool de workers QuickJS qui exécutent `globalThis.__moltnRender(page)`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rquickjs::{CatchResultExt, Context, Function, Module, Object, Promise, Runtime, Value};
use serde_json::Value as Json;
use tokio::sync::oneshot;
use veer::{SsrClient, SsrPayload, ssr::SsrError};

/// Environnement minimal fourni avant le bundle.
const PRELUDE: &str = r#"
globalThis.process = { env: { NODE_ENV: 'production' } };
// Classes seulement testées par instanceof pendant le rendu.
for (const n of ['FormData', 'Blob', 'File', 'FileList', 'DOMException', 'Document', 'Element', 'HTMLElement', 'HTMLTitleElement', 'SVGElement', 'MathMLElement', 'ShadowRoot']) {
  if (typeof globalThis[n] === 'undefined') globalThis[n] = class {};
}
// react-dom/server.browser crée un MessageChannel au chargement (ordonnanceur du rendu en flux).
globalThis.MessageChannel ??= class {
  constructor() {
    this.port1 = { onmessage: null };
    this.port2 = { postMessage: (data) => queueMicrotask(() => this.port1.onmessage?.({ data })) };
  }
};
globalThis.console = Object.fromEntries(['log', 'info', 'warn', 'error', 'debug'].map(
  (l) => [l, (...a) => __moltnLog(l, a.map((x) => x instanceof Error ? (String(x) + '\n' + (x.stack || '')) : typeof x === 'string' ? x : JSON.stringify(x)).join(' '))]
));
"#;

type Job = (String, oneshot::Sender<Result<SsrPayload, String>>);

pub struct Engine {
    tx: SyncSender<Job>,
}

impl Engine {
    /// Démarre `workers` threads, chacun charge le bundle. Échoue si un chargement échoue.
    pub fn new(bundle: String, workers: usize, timeout: Duration, mem_mb: usize) -> Result<Self, String> {
        let (tx, rx) = sync_channel::<Job>(workers * 8);
        let rx = Arc::new(Mutex::new(rx));
        let bundle = Arc::new(bundle);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        for i in 0..workers {
            let (rx, bundle, ready_tx) = (rx.clone(), bundle.clone(), ready_tx.clone());
            std::thread::Builder::new()
                .name(format!("moltn-ssr-{i}"))
                .spawn(move || worker(&bundle, &rx, timeout, mem_mb, ready_tx))
                .map_err(|e| e.to_string())?;
        }
        for _ in 0..workers {
            ready_rx.recv().map_err(|e| e.to_string())??;
        }
        Ok(Self { tx })
    }

    pub async fn render_json(&self, page_json: String) -> Result<SsrPayload, String> {
        let (otx, orx) = oneshot::channel();
        match self.tx.try_send((page_json, otx)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err("file pleine".into()),
            Err(TrySendError::Disconnected(_)) => return Err("pool arrêté".into()),
        }
        orx.await.map_err(|_| "worker mort".to_string())?
    }
}

#[async_trait]
impl SsrClient for Engine {
    async fn render(&self, page: &Json) -> Result<SsrPayload, SsrError> {
        let t = Instant::now();
        let r = self.render_json(page.to_string()).await.map_err(SsrError::Transport);
        eprintln!("[ssr] {} en {:?}", page["component"], t.elapsed());
        r
    }
}

fn worker(
    bundle: &str,
    rx: &Mutex<Receiver<Job>>,
    timeout: Duration,
    mem_mb: usize,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) {
    // Recrée le moteur après une erreur fatale (délai dépassé, mémoire).
    let mut first = true;
    loop {
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        let epoch = Instant::now();
        let (rt, ctx) = match load(bundle, mem_mb, &deadline, epoch) {
            Ok(x) => x,
            Err(e) => {
                if first {
                    let _ = ready.send(Err(e));
                }
                return;
            }
        };
        if first {
            let _ = ready.send(Ok(()));
            first = false;
        }
        loop {
            let job = rx.lock().ok().and_then(|r| r.recv().ok());
            let Some((page, reply)) = job else { return };
            let start = epoch.elapsed();
            deadline.store((start + timeout).as_millis() as u64, Ordering::Relaxed);
            let (res, fatal) = render(&ctx, &page);
            deadline.store(u64::MAX, Ordering::Relaxed);
            // Délai atteint : contexte interrompu en plein rendu, on ne le réutilise pas.
            let fatal = fatal || epoch.elapsed() - start >= timeout;
            let _ = reply.send(res);
            if fatal {
                eprintln!("[ssr] worker recréé");
                break;
            }
        }
        drop(ctx);
        drop(rt);
    }
}

fn load(bundle: &str, mem_mb: usize, deadline: &Arc<AtomicU64>, epoch: Instant) -> Result<(Runtime, Context), String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    rt.set_memory_limit(mem_mb * 1024 * 1024);
    let d = deadline.clone();
    rt.set_interrupt_handler(Some(Box::new(move || {
        epoch.elapsed().as_millis() as u64 > d.load(Ordering::Relaxed)
    })));
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
    ctx.with(|ctx| -> Result<(), String> {
        let log = Function::new(ctx.clone(), |level: String, msg: String| eprintln!("[js:{level}] {msg}"))
            .map_err(|e| e.to_string())?;
        ctx.globals().set("__moltnLog", log).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(PRELUDE).catch(&ctx).map_err(|e| e.to_string())?;
        Module::evaluate(ctx.clone(), "ssr.js", bundle)
            .catch(&ctx)
            .and_then(|p| p.finish::<()>().catch(&ctx))
            .map_err(|e| format!("chargement du bundle : {e}"))?;
        let f: Value = ctx.globals().get("__moltnRender").map_err(|e| e.to_string())?;
        if !f.is_function() {
            return Err("le bundle ne définit pas globalThis.__moltnRender".into());
        }
        Ok(())
    })?;
    Ok((rt, ctx))
}

/// Renvoie (résultat, erreur fatale → recréer le worker).
fn render(ctx: &Context, page_json: &str) -> (Result<SsrPayload, String>, bool) {
    ctx.with(|ctx| {
        let out = (|| -> Result<SsrPayload, String> {
            let page = ctx.json_parse(page_json).catch(&ctx).map_err(|e| e.to_string())?;
            let f: Function = ctx.globals().get("__moltnRender").map_err(|e| e.to_string())?;
            let p: Promise = f.call((page,)).catch(&ctx).map_err(|e| e.to_string())?;
            let o: Object = p.finish().catch(&ctx).map_err(|e| e.to_string())?;
            let s = ctx
                .json_stringify(o)
                .map_err(|e| e.to_string())?
                .ok_or("résultat non sérialisable")?
                .to_string()
                .map_err(|e| e.to_string())?;
            let v: Json = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            let head = v["head"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let body = v["body"].as_str().ok_or("body manquant")?.to_string();
            Ok(SsrPayload { head, body })
        })();
        let fatal = matches!(&out, Err(e) if e.contains("interrupted") || e.contains("out of memory") || e.contains("WouldBlock") || e.contains("would block"));
        if let Err(e) = &out {
            eprintln!("[ssr] erreur : {e}");
        }
        (out, fatal)
    })
}
