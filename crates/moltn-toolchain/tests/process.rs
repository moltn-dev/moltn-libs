//! Exécution : arrêt propre de l'arbre de processus, et tests réels (Node, pnpm) ignorés par défaut.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use moltn_toolchain::{DevProcess, JsProject, ReadyProbe, ToolchainError};

const ROLE: &str = "MOLTN_TREE_ROLE";
const PID_FILE: &str = "MOLTN_TREE_PID_FILE";

/// Ce binaire de test relancé en « parent » (qui lance un enfant) ou en « enfant ».
/// Sans variable d'environnement : test vide.
#[test]
fn tree_helper() {
    match std::env::var(ROLE).as_deref() {
        Ok("parent") => {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tree_helper", "--nocapture"])
                .env(ROLE, "child")
                .spawn()
                .unwrap();
            std::fs::write(std::env::var(PID_FILE).unwrap(), child.id().to_string()).unwrap();
            // L'enfant dort 60 s : le parent reste vivant aussi longtemps.
            let _ = child.wait();
        }
        Ok("child") => std::thread::sleep(Duration::from_secs(60)),
        _ => {}
    }
}

fn alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .any(|w| w == pid.to_string())
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    }
}

async fn until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn dropping_dev_process_kills_grandchildren() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "tree_helper", "--nocapture"])
        .env(ROLE, "parent")
        .env(PID_FILE, &pid_file);

    let dev = DevProcess::spawn(command, "arbre").unwrap();
    assert!(
        until(Duration::from_secs(20), || pid_file.exists()).await,
        "le parent n'a pas lancé d'enfant"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let grandchild: u32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(grandchild));

    drop(dev);
    assert!(
        until(Duration::from_secs(10), || !alive(grandchild)).await,
        "petit-enfant {grandchild} toujours vivant"
    );
}

#[tokio::test]
async fn exiting_before_ready_reports_output() {
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "aucun_test", "--nocapture"]);
    let mut dev = DevProcess::spawn(command, "court").unwrap();
    let err = dev
        .wait_ready(
            ReadyProbe::Tcp("127.0.0.1:9".into()),
            Duration::from_secs(20),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolchainError::ExitedEarly { .. }), "{err:?}");
}

// --- Exécution réelle : `cargo test -p moltn-toolchain -- --ignored` (Node et pnpm requis). ---

fn example_web() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/axum-vue/web")
}

#[tokio::test]
#[ignore = "lance un vrai Node et pnpm"]
async fn real_pnpm_script() {
    let tc = JsProject::detect(example_web()).unwrap().resolve().unwrap();
    tc.run_script("typecheck", &[]).await.unwrap();
}

#[tokio::test]
#[ignore = "lance un vrai Node et pnpm"]
async fn real_failing_script_reports_tail() {
    let tc = JsProject::detect(example_web()).unwrap().resolve().unwrap();
    let err = tc.run_script("script-inexistant", &[]).await.unwrap_err();
    assert!(
        matches!(err, ToolchainError::CommandFailed { .. }),
        "{err:?}"
    );
}

#[tokio::test]
#[ignore = "lance un vrai Vite"]
async fn real_vite_dev_server_is_killed() {
    let tc = JsProject::detect(example_web()).unwrap().resolve().unwrap();
    let mut vite = tc
        .spawn_script("dev", &["--port", "5199", "--strictPort"])
        .unwrap();
    vite.wait_ready(
        ReadyProbe::Tcp("localhost:5199".into()),
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    drop(vite);
    let freed = until(Duration::from_secs(10), || {
        std::net::TcpStream::connect("localhost:5199").is_err()
    })
    .await;
    assert!(freed, "Vite écoute encore après destruction");
}
