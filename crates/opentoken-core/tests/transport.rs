use chrono::Utc;
use fs2::FileExt;
use opentoken_core::{
    codex::{CliClient, CodexCli},
    model::FaultKind,
    parser,
};
use std::{fs::OpenOptions, time::Duration};
fn client() -> CliClient {
    CliClient {
        executable: Some(env!("CARGO_BIN_EXE_fake-codex").into()),
    }
}
#[tokio::test]
async fn real_stdio_ignores_notifications_and_validates_identity() {
    let t = tempfile::tempdir().unwrap();
    // The child deliberately reports 99% if any overriding token leaks through.
    let names = [
        "CODEX_API_KEY",
        "CODEX_ACCESS_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_SQLITE_HOME",
    ];
    let old: Vec<_> = names.iter().map(std::env::var_os).collect();
    for name in names {
        std::env::set_var(name, "fixture-secret-must-not-be-inherited");
    }
    let reading = client().read(t.path()).await;
    for (name, value) in names.into_iter().zip(old) {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
    let r = reading.unwrap();
    assert_eq!(r.identity.workspace, "fixture");
    assert_eq!(
        parser::codex_cli(&r.usage, r.plan, Utc::now())
            .unwrap()
            .quotas[0]
            .percent,
        Some(25.0)
    );
}
#[tokio::test]
async fn workspace_switch_during_read_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("switch"), "true").unwrap();
    assert_eq!(
        client().read(t.path()).await.unwrap_err().kind,
        FaultKind::IdentityChanged
    );
}
#[tokio::test]
async fn cancellation_terminates_the_child() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("stall"), "true").unwrap();
    let c = client();
    let read = c.read(t.path());
    assert!(tokio::time::timeout(Duration::from_millis(600), read)
        .await
        .is_err());
    assert!(t.path().join("started").exists());
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(t.path().join("child.lock"))
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if lock.try_lock_exclusive().is_ok() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "child still alive after cancelled read"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
