//! Running commands: output relayed to `tracing`, process tree killed at the end.

use std::collections::VecDeque;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};

use crate::ToolchainError;

const TAIL_LINES: usize = 30;

/// Last lines of output, for error messages.
#[derive(Clone, Default)]
struct Tail(Arc<Mutex<VecDeque<String>>>);

impl Tail {
    fn push(&self, line: String) {
        if let Ok(mut lines) = self.0.lock() {
            if lines.len() == TAIL_LINES {
                lines.pop_front();
            }
            lines.push_back(line);
        }
    }

    fn text(&self) -> String {
        self.0
            .lock()
            .map(|l| l.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }
}

fn describe(command: &Command) -> String {
    let std = command.as_std();
    std::iter::once(std.get_program())
        .chain(std.get_args())
        .map(|s| s.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Receives every output line of started processes: `(label, line)`.
/// By default, lines go to `tracing` (which escapes color codes);
/// a command-line tool can print them as they are.
pub type OutputFn = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Start the command, relay stdout/stderr to `output` or to `tracing` (`[label] …`).
fn spawn_logged(
    command: &mut Command,
    label: &str,
    output: Option<&OutputFn>,
) -> Result<(Child, Tail), ToolchainError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|source| ToolchainError::Spawn {
        command: describe(command),
        source,
    })?;
    let tail = Tail::default();
    if let Some(out) = child.stdout.take() {
        tokio::spawn(relay(out, label.to_string(), tail.clone(), output.cloned()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(relay(err, label.to_string(), tail.clone(), output.cloned()));
    }
    Ok((child, tail))
}

async fn relay(
    stream: impl AsyncRead + Unpin,
    label: String,
    tail: Tail,
    output: Option<OutputFn>,
) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match &output {
            Some(output) => output(&label, &line),
            None => tracing::info!(target: "moltn_toolchain::output", "[{label}] {line}"),
        }
        tail.push(line);
    }
}

/// Run a command to completion; non-zero exit → error with the last lines.
pub(crate) async fn run(
    mut command: Command,
    label: &str,
    output: Option<&OutputFn>,
) -> Result<(), ToolchainError> {
    let description = describe(&command);
    let (mut child, tail) = spawn_logged(&mut command, label, output)?;
    let status = child.wait().await.map_err(|source| ToolchainError::Spawn {
        command: description.clone(),
        source,
    })?;
    // Laisse les relais finir de lire la sortie.
    tokio::time::sleep(Duration::from_millis(50)).await;
    if status.success() {
        return Ok(());
    }
    let tail = tail.text();
    if let Some(packages) = ignored_builds(&tail) {
        return Err(ToolchainError::IgnoredBuilds { packages });
    }
    Err(ToolchainError::CommandFailed {
        command: description,
        code: status.code(),
        tail,
    })
}

/// `[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: core-js@3.51.0, esbuild@0.25.0`
fn ignored_builds(output: &str) -> Option<Vec<String>> {
    let line = output
        .lines()
        .find(|l| l.contains("Ignored build scripts:"))?;
    let list = line.split_once("Ignored build scripts:")?.1;
    let packages: Vec<String> = list
        .split(',')
        .map(|p| {
            let p = p.trim().trim_end_matches('.');
            // `@scope/nom@1.0.0` : le `@` de version est le dernier, pas le premier.
            match p.rfind('@') {
                Some(i) if i > 0 => p[..i].to_string(),
                _ => p.to_string(),
            }
        })
        .filter(|p| !p.is_empty())
        .collect();
    (!packages.is_empty()).then_some(packages)
}

/// How to tell that a dev process is ready.
#[derive(Debug, Clone)]
pub enum ReadyProbe {
    /// A TCP connection succeeds (`"localhost:5173"`).
    Tcp(String),
}

/// A long-running process (dev server) whose whole tree is killed when the
/// value is dropped: process group on Unix, Job Object on Windows.
pub struct DevProcess {
    child: Child,
    label: String,
    tail: Tail,
    // Déclaré après `child` : détruit en dernier, il tue les petits-enfants.
    _tree: tree::Guard,
}

impl DevProcess {
    /// Start `command` under supervision; `label` prefixes its output in the logs.
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), moltn_toolchain::ToolchainError> {
    /// let mut command = tokio::process::Command::new("cargo");
    /// command.arg("run");
    /// let mut app = moltn_toolchain::DevProcess::spawn(command, "app")?;
    /// let status = app.wait().await?;
    /// # let _ = status; Ok(()) }
    /// ```
    pub fn spawn(command: Command, label: &str) -> Result<Self, ToolchainError> {
        Self::spawn_with_output(command, label, None)
    }

    /// Like [`Self::spawn`], with output sent to `output` instead of `tracing`.
    ///
    /// ```no_run
    /// # fn run() -> Result<(), moltn_toolchain::ToolchainError> {
    /// use std::sync::Arc;
    /// let print: moltn_toolchain::OutputFn = Arc::new(|label, line| println!("[{label}] {line}"));
    /// let app = moltn_toolchain::DevProcess::spawn_with_output(tokio::process::Command::new("cargo"), "app", Some(&print))?;
    /// # let _ = app; Ok(()) }
    /// ```
    pub fn spawn_with_output(
        mut command: Command,
        label: &str,
        output: Option<&OutputFn>,
    ) -> Result<Self, ToolchainError> {
        tree::prepare(&mut command);
        let (child, tail) = spawn_logged(&mut command, label, output)?;
        let guard = tree::Guard::attach(&child);
        Ok(Self {
            child,
            label: label.into(),
            tail,
            _tree: guard,
        })
    }

    /// Wait for `probe` to succeed; fails if the process stops first or `timeout` elapses.
    pub async fn wait_ready(
        &mut self,
        probe: ReadyProbe,
        timeout: Duration,
    ) -> Result<(), ToolchainError> {
        let start = Instant::now();
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                tokio::time::sleep(Duration::from_millis(50)).await;
                return Err(ToolchainError::ExitedEarly {
                    label: self.label.clone(),
                    status,
                    tail: self.tail.text(),
                });
            }
            let ReadyProbe::Tcp(addr) = &probe;
            if tokio::net::TcpStream::connect(addr.as_str()).await.is_ok() {
                return Ok(());
            }
            if start.elapsed() > timeout {
                return Err(ToolchainError::NotReady {
                    label: self.label.clone(),
                    waited: timeout,
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Wait for the process to end.
    pub async fn wait(&mut self) -> Result<ExitStatus, ToolchainError> {
        self.child
            .wait()
            .await
            .map_err(|source| ToolchainError::Spawn {
                command: self.label.clone(),
                source,
            })
    }

    /// ID of the started process (not of its children).
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Last lines of output.
    pub fn tail(&self) -> String {
        self.tail.text()
    }

    /// Stop the process and its children, and wait until they are all gone (at most 5 s).
    /// Useful before a restart: on Windows, an executable that is still running cannot be
    /// overwritten by `cargo build`.
    pub async fn stop(mut self) {
        self._tree.kill();
        let start = Instant::now();
        while !self._tree.is_empty() && start.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Unix : un enfant qui ignore SIGTERM ferait attendre sans fin.
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

#[cfg(windows)]
mod tree {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    };

    pub(super) fn prepare(_command: &mut tokio::process::Command) {}

    /// "Kill on close" Job Object: if moltn dies, even abruptly, Windows
    /// closes the handle and kills every process in the job.
    pub(super) struct Guard(HANDLE);

    // SAFETY: un HANDLE de job est utilisable depuis n'importe quel thread ; seul ce Guard le ferme.
    unsafe impl Send for Guard {}
    unsafe impl Sync for Guard {}

    impl Guard {
        pub(super) fn attach(child: &tokio::process::Child) -> Self {
            // SAFETY: appels Win32 sur des handles valides ; la structure est initialisée à zéro
            // comme le demande la documentation de SetInformationJobObject.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    tracing::warn!("Job Object unavailable: child processes might survive");
                    return Self(job);
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                // shortcut: un petit-enfant créé avant cette ligne échappe au job ; fenêtre de
                // quelques µs, bien plus courte que le démarrage de npm ou de Vite.
                if let Some(handle) = child.raw_handle() {
                    AssignProcessToJobObject(job, handle as HANDLE);
                }
                Self(job)
            }
        }

        pub(super) fn kill(&self) {
            if !self.0.is_null() {
                // SAFETY: handle de job valide tant que le Guard existe.
                unsafe {
                    TerminateJobObject(self.0, 1);
                }
            }
        }

        /// No process left alive in the job.
        pub(super) fn is_empty(&self) -> bool {
            if self.0.is_null() {
                return true;
            }
            // SAFETY: structure de sortie de la bonne taille, initialisée à zéro.
            unsafe {
                let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
                let ok = QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    (&raw mut info).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                );
                ok == 0 || info.ActiveProcesses == 0
            }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if self.0.is_null() {
                return;
            }
            self.kill();
            // SAFETY: handle de job créé par `attach`, fermé une seule fois ici.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(unix)]
mod tree {
    use std::time::Duration;

    /// New process group: the whole group is killed at once.
    pub(super) fn prepare(command: &mut tokio::process::Command) {
        command.process_group(0);
    }

    pub(super) struct Guard(Option<i32>);

    impl Guard {
        pub(super) fn attach(child: &tokio::process::Child) -> Self {
            Self(child.id().and_then(|id| i32::try_from(id).ok()))
        }

        pub(super) fn kill(&self) {
            if let Some(pgid) = self.0 {
                // SAFETY: kill(2) sur un groupe que nous avons créé ; aucun pointeur.
                unsafe {
                    libc::kill(-pgid, libc::SIGTERM);
                }
            }
        }

        /// No process left in the group (kill(…, 0) fails with ESRCH).
        pub(super) fn is_empty(&self) -> bool {
            // SAFETY: signal 0 : simple test d'existence.
            self.0
                .is_none_or(|pgid| unsafe { libc::kill(-pgid, 0) } != 0)
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            let Some(pgid) = self.0 else { return };
            self.kill();
            // Laisse le temps de s'arrêter proprement, puis force.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(2));
                // SAFETY: idem ; ESRCH (groupe déjà parti) est sans effet.
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ignored_builds;

    #[test]
    fn parses_pnpm_ignored_builds() {
        let out =
            "[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: core-js@3.51.0, @scope/native@1.0.0.";
        assert_eq!(
            ignored_builds(out),
            Some(vec!["core-js".into(), "@scope/native".into()])
        );
        assert_eq!(ignored_builds("autre erreur"), None);
    }
}
