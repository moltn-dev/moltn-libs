//! Pool de workers : un thread OS par moteur, file bornée, réponse par oneshot.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use crate::engine::{EngineLimits, JsEngine};
use crate::{SsrError, SsrOutput};

type Reply = oneshot::Sender<Result<SsrOutput, SsrError>>;

#[derive(Debug)]
pub(crate) struct Pool {
    jobs: SyncSender<(String, Reply)>,
}

pub(crate) struct PoolConfig {
    pub workers: usize,
    pub queue_capacity: usize,
    pub timeout: Duration,
    pub limits: EngineLimits,
}

impl Pool {
    /// Démarre les workers et attend que chacun ait chargé le bundle (bloquant).
    pub fn start<E: JsEngine>(bundle: Arc<str>, config: PoolConfig) -> Result<Self, SsrError> {
        let (jobs, rx) = sync_channel(config.queue_capacity);
        let rx = Arc::new(Mutex::new(rx));
        let (ready_tx, ready_rx) = channel();
        for i in 0..config.workers {
            let (bundle, rx, ready) = (bundle.clone(), rx.clone(), ready_tx.clone());
            let (limits, timeout) = (config.limits, config.timeout);
            std::thread::Builder::new()
                .name(format!("moltn-ssr-{i}"))
                .spawn(move || worker::<E>(&bundle, &rx, limits, timeout, ready))
                .map_err(|e| SsrError::Load(format!("creating the thread: {e}")))?;
        }
        drop(ready_tx);
        for _ in 0..config.workers {
            ready_rx.recv().map_err(|_| SsrError::WorkerUnavailable)??;
        }
        Ok(Self { jobs })
    }

    /// Ne bloque jamais : file pleine → erreur immédiate.
    pub async fn render(&self, page_json: String) -> Result<SsrOutput, SsrError> {
        let (reply, response) = oneshot::channel();
        self.jobs
            .try_send((page_json, reply))
            .map_err(|e| match e {
                TrySendError::Full(_) => SsrError::QueueFull,
                TrySendError::Disconnected(_) => SsrError::WorkerUnavailable,
            })?;
        response.await.map_err(|_| SsrError::WorkerUnavailable)?
    }
}

fn worker<E: JsEngine>(
    bundle: &str,
    jobs: &Mutex<Receiver<(String, Reply)>>,
    limits: EngineLimits,
    timeout: Duration,
    ready: Sender<Result<(), SsrError>>,
) {
    let mut ready = Some(ready);
    // Boucle externe : une itération par moteur ; on en recrée un après une erreur fatale.
    loop {
        let mut engine = match E::load(bundle, &limits) {
            Ok(engine) => engine,
            Err(e) => {
                match ready.take() {
                    Some(ready) => {
                        let _ = ready.send(Err(e));
                    }
                    None => {
                        tracing::error!(error = %e, "could not reload the SSR bundle, worker stopped")
                    }
                }
                return;
            }
        };
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(()));
        }
        loop {
            // Le verrou est relâché dès la réception, avant le rendu.
            let job = match jobs.lock() {
                Ok(rx) => rx.recv(),
                Err(_) => return,
            };
            let Ok((page_json, reply)) = job else {
                return; // pool détruit
            };
            // Appelant parti (client déconnecté, délai côté HTTP) : rendu inutile.
            if reply.is_closed() {
                continue;
            }
            let result = catch_unwind(AssertUnwindSafe(|| engine.render(&page_json, timeout)))
                .unwrap_or(Err(SsrError::Panic));
            let restart = result
                .as_ref()
                .err()
                .is_some_and(SsrError::requires_restart);
            let _ = reply.send(result);
            if restart {
                tracing::warn!("SSR engine recreated after a fatal error");
                break;
            }
        }
    }
}
