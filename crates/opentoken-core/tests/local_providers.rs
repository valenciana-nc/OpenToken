//! Actual owned process/stdin/TCP/Windows ownership acceptance, with synthetic provider data only.
use opentoken_core::{
    connections::{AntigravityClient, ConnectionMode, Service, UnavailableStore},
    engine::Engine,
    model::{Provider, ReportSource, Status},
    storage::{Settings, Storage},
};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
fn setup() -> (tempfile::TempDir, Storage) {
    let root = tempfile::Builder::new()
        .prefix("provider-fixture-")
        .tempdir()
        .unwrap();
    let storage = Storage {
        dir: root.path().join("app"),
        user_home: root.path().join("home"),
    };
    std::fs::create_dir_all(&storage.dir).unwrap();
    (root, storage)
}
fn muse_profile(storage: &Storage) -> PathBuf {
    let p = storage.user_home.join(".config/muse/auth.json");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p,json!({"providers":{"meta":{"mechanism":"oauth","access_token":"dca:synthetic-fixture-only-not-a-real-token"}}}).to_string()).unwrap();
    let exe =
        storage
            .user_home
            .join(".local/bin")
            .join(if cfg!(windows) { "muse.exe" } else { "muse" });
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-local-provider"), exe).unwrap();
    p
}
#[tokio::test]
async fn official_antigravity_usage_diagnosis_is_explicit_bounded_and_not_a_connection() {
    for mode in ["success", "refused", "old-version"] {
        let (_root, storage) = setup();
        let exe = storage.user_home.join("AppData/Local/agy/bin/agy.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_fake-local-provider"), &exe).unwrap();
        if mode != "success" {
            std::fs::write(storage.user_home.join(format!("fixture-cli-{mode}")), "").unwrap();
        }
        let service = Service::new(storage.clone(), Arc::new(UnavailableStore));
        assert!(service
            .inspect_antigravity_cli_usage(true, false)
            .await
            .is_err());
        assert!(!storage.user_home.join("fixture-cli-invocations").exists());
        let result = service.inspect_antigravity_cli_usage(true, true).await;
        if mode == "success" {
            let report = result.unwrap();
            assert_eq!(report.quota_count, 2);
            assert_eq!(report.buckets[0].percent, Some(25.));
            assert!(!report.identity_verified);
            assert!(report.zero_model_work);
        } else {
            let error = result.err().unwrap();
            assert!(!error.message.contains("fixture-sensitive"));
        }
        let calls =
            std::fs::read_to_string(storage.user_home.join("fixture-cli-invocations")).unwrap();
        assert_eq!(
            calls.lines().count(),
            if mode == "old-version" { 1 } else { 2 }
        );
        assert!(!storage.dir.join("settings.json").exists());
        assert!(!storage.dir.join("state.json").exists());
    }
}
#[tokio::test]
async fn cli_profile_monitor_persists_displays_and_cannot_restore_or_inherit_old_quota() {
    let (_root, storage) = setup();
    let exe = storage.user_home.join("AppData/Local/agy/bin/agy.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-local-provider"), &exe).unwrap();
    let initial = Settings {
        codex_enabled: false,
        claude_enabled: false,
        theme: "dark".into(),
        ..Default::default()
    };
    let engine = Engine::start(storage.clone(), initial.clone());
    assert!(engine.connect_antigravity_cli_usage(false).await.is_err());
    assert!(!storage.user_home.join("fixture-cli-invocations").exists());
    let result = engine.connect_antigravity_cli_usage(true).await.unwrap();
    assert_eq!(result.connected, 1);
    assert_eq!(result.settings.theme, "dark");
    let c = result.settings.connections[0].clone();
    assert_eq!(c.mode, ConnectionMode::AntigravityCliUsage);
    let snapshot = engine.state.borrow().clone();
    let state = snapshot
        .accounts
        .iter()
        .find(|a| a.account.id == c.id)
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert!(state.account.email.is_none());
    assert!(state.account.label.contains("current profile"));
    assert_eq!(
        state.report.as_ref().unwrap().source,
        ReportSource::AntigravityCliUsage
    );
    assert!(state.report.as_ref().unwrap().verified_at.is_none());
    engine.shutdown().await;
    opentoken_core::storage::atomic_json(&storage.dir.join("state.json"), &snapshot).unwrap();
    // A saved grant must not survive corruption recovery without renewed approval.
    storage.save_settings(&result.settings).unwrap();
    let backup: Settings =
        opentoken_core::storage::read_json(&storage.dir.join("settings.json.bak")).unwrap();
    assert!(backup
        .connections
        .iter()
        .all(|x| x.mode != ConnectionMode::AntigravityCliUsage));
    std::fs::write(storage.user_home.join("fixture-cli-invalid"), "").unwrap();
    let restarted = Engine::start(storage.clone(), result.settings);
    for _ in 0..200 {
        if restarted
            .state
            .borrow()
            .accounts
            .iter()
            .any(|a| a.account.id == c.id && a.fault.is_some())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let failed = restarted
        .state
        .borrow()
        .accounts
        .iter()
        .find(|a| a.account.id == c.id)
        .cloned()
        .unwrap();
    assert!(
        failed.report.is_none(),
        "a profile-only observation must not become stale attribution across restart/failure"
    );
    assert_eq!(failed.status, Status::Faulted);
    let next = restarted.disconnect(c.id).await.unwrap();
    assert!(next.connections.is_empty());
    assert!(exe.exists());
    restarted.shutdown().await;
}
#[tokio::test]
async fn muse_owns_only_observation_probe_and_never_creates_a_session_or_mints() {
    let (_root, storage) = setup();
    let profile = muse_profile(&storage);
    let before = std::fs::read(&profile).unwrap();
    let service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    let settings = Settings::default();
    assert!(service
        .connect_local_provider(&settings, Provider::Muse, false)
        .await
        .is_err());
    assert!(!profile
        .parent()
        .unwrap()
        .join("fixture-msp-methods.log")
        .exists());
    let (next, state) = service
        .connect_local_provider(&settings, Provider::Muse, true)
        .await
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert_eq!(next.connections[0].mode, ConnectionMode::MuseCli);
    assert_eq!(state.report.unwrap().source, ReportSource::MuseCli);
    let methods =
        std::fs::read_to_string(profile.parent().unwrap().join("fixture-msp-methods.log")).unwrap();
    assert_eq!(methods, "initialize\ninitialized\nusage/read\n");
    assert_eq!(std::fs::read(&profile).unwrap(), before);
    assert!(!serde_json::to_string(&next).unwrap().contains("dca:"));
    assert!(service.fetch(&next.connections[0]).await.is_ok());
    std::fs::write(profile.parent().unwrap().join("fixture-empty"), "").unwrap();
    let fault = service.fetch(&next.connections[0]).await.unwrap_err();
    assert!(fault.message.contains("no subscription observation"));
    let final_settings = service.disconnect(&next, &next.connections[0].id).unwrap();
    assert!(final_settings.connections.is_empty());
    assert_eq!(std::fs::read(&profile).unwrap(), before);
    assert!(!storage.dir.join("sign-ins").exists());
}
#[tokio::test]
async fn empty_or_expired_muse_observation_never_persists_a_connected_account() {
    for kind in ["fixture-empty", "fixture-expired"] {
        let (_root, storage) = setup();
        let p = muse_profile(&storage);
        std::fs::write(p.parent().unwrap().join(kind), "").unwrap();
        let service = Service::new(storage.clone(), Arc::new(UnavailableStore));
        assert!(service
            .connect_local_provider(&Settings::default(), Provider::Muse, true)
            .await
            .is_err());
        assert!(storage.load_settings().unwrap().connections.is_empty());
    }
}
#[tokio::test]
async fn cancellation_stops_owned_probe_rejects_duplicates_and_cannot_create_a_connection() {
    let (_root, storage) = setup();
    let p = muse_profile(&storage);
    std::fs::write(p.parent().unwrap().join("fixture-wait"), "").unwrap();
    let engine = Engine::start(
        storage.clone(),
        Settings {
            codex_enabled: false,
            claude_enabled: false,
            ..Default::default()
        },
    );
    let read = tokio::spawn({
        let engine = engine.clone();
        async move { engine.connect_existing(Provider::Muse, true).await }
    });
    let log = p.parent().unwrap().join("fixture-msp-methods.log");
    for _ in 0..150 {
        if std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("usage/read")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(std::fs::read_to_string(&log)
        .unwrap()
        .contains("usage/read"));
    assert!(engine
        .connect_existing(Provider::Muse, true)
        .await
        .unwrap_err()
        .to_string()
        .contains("already running"));
    engine.cancel_existing(Provider::Muse);
    let result = read.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        let pid: u32 = std::fs::read_to_string(p.parent().unwrap().join("fixture-msp-pid"))
            .unwrap()
            .parse()
            .unwrap();
        let active = || unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut code = 0;
                let ok = GetExitCodeProcess(handle, &mut code).is_ok();
                let _ = CloseHandle(handle);
                ok && code == 259
            } else {
                false
            }
        };
        for _ in 0..100 {
            if !active() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!active(), "cancel must stop only its owned synthetic host");
    }
    assert!(engine.settings.borrow().connections.is_empty());
    assert!(storage.load_settings().unwrap().connections.is_empty());
    engine.shutdown().await;
}
#[tokio::test]
async fn muse_changed_profile_invalidates_cached_quota_without_bypassing_read_spacing() {
    let (_root, storage) = setup();
    let profile = muse_profile(&storage);
    let engine = Engine::start(
        storage.clone(),
        Settings {
            codex_enabled: false,
            claude_enabled: false,
            ..Default::default()
        },
    );
    engine.connect_existing(Provider::Muse, true).await.unwrap();
    let log = profile.parent().unwrap().join("fixture-msp-methods.log");
    let before = std::fs::read_to_string(&log).unwrap();
    std::fs::write(&profile,json!({"providers":{"meta":{"mechanism":"oauth","access_token":"dca:another-fixture-subscription-login"}}}).to_string()).unwrap();
    for _ in 0..300 {
        if engine
            .state
            .borrow()
            .accounts
            .iter()
            .any(|a| a.account.provider == Provider::Muse && a.report.is_none())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let state = engine.state.borrow().clone();
    assert_eq!(state.accounts.len(), 1);
    assert!(state.accounts[0].report.is_none());
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        before,
        "credential change cannot buy an extra quota read inside spacing"
    );
    engine.shutdown().await;
}
#[cfg(windows)]
#[tokio::test]
async fn antigravity_native_windows_process_sid_lifetime_port_and_rpc_are_verified() {
    for ide in [false, true] {
        let (_root, storage) = setup();
        // Both old/new Roaming folders exist on the affected real installation.
        std::fs::create_dir_all(storage.user_home.join("AppData/Roaming/Antigravity")).unwrap();
        std::fs::create_dir_all(storage.user_home.join("AppData/Roaming/Antigravity IDE")).unwrap();
        let folder = if ide {
            "Antigravity IDE"
        } else {
            "Antigravity"
        };
        let dir = storage
            .user_home
            .join("AppData/Local/Programs")
            .join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(if ide {
            "language_server_windows_x64.exe"
        } else {
            "language_server.exe"
        });
        std::fs::copy(env!("CARGO_BIN_EXE_fake-local-provider"), &exe).unwrap();
        let output = storage.dir.join("fixture-listener.json");
        let mut child = tokio::process::Command::new(&exe)
            .args([
                "--app_data_dir",
                if ide {
                    "antigravity-ide"
                } else {
                    "antigravity"
                },
                "--override_ide_name",
                "antigravity",
                "--csrf_token",
                "synthetic-fixture-csrf-only",
                "--fixture-output",
            ])
            .arg(&output)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        for _ in 0..100 {
            if output.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(output.exists());
        let service = Service::new(storage.clone(), Arc::new(UnavailableStore));
        let (next, state) = service
            .connect_local_provider(&Settings::default(), Provider::Antigravity, true)
            .await
            .unwrap();
        assert_eq!(state.status, Status::Ready);
        assert_eq!(
            state.account.email.as_deref(),
            Some("antigravity@fixture.test")
        );
        let r = state.report.unwrap();
        assert_eq!(r.source, ReportSource::AntigravityLocal);
        assert!(next.connections[0]
            .profile
            .as_deref()
            .unwrap()
            .ends_with(folder));
        assert_eq!(r.quotas.len(), 4);
        assert_eq!(r.quotas[0].percent, Some(25.));
        assert!(service.fetch(&next.connections[0]).await.is_ok());
        if ide {
            let stale = storage
                .user_home
                .join("AppData/Roaming/Antigravity IDE/User/globalStorage/state.vscdb");
            std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
            std::fs::write(stale, b"fixture deliberately unusable saved OAuth database").unwrap();
        }
        let recovery = Service::new(storage.clone(), Arc::new(UnavailableStore));
        let client = if ide {
            AntigravityClient::Ide
        } else {
            AntigravityClient::Desktop
        };
        assert!(
            recovery
                .antigravity_recovery_marker(client, true)
                .await
                .is_ok(),
            "GUI recovery must not depend on old saved-token validity"
        );
        let (verified, recovered) = recovery
            .connect_antigravity_client(&Settings::default(), client, true)
            .await
            .unwrap();
        assert_eq!(verified.connections[0].mode, ConnectionMode::AntigravityApp);
        assert_eq!(
            recovered.report.unwrap().source,
            ReportSource::AntigravityLocal
        );
        std::fs::write(storage.dir.join("fixture-switch"), "").unwrap();
        assert!(service.fetch(&next.connections[0]).await.is_err());
        let next = service.disconnect(&next, &next.connections[0].id).unwrap();
        assert!(next.connections.is_empty());
        let fixture: Value = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(fixture["pid"].as_u64().unwrap(), child.id().unwrap() as u64);
        assert!(
            child.try_wait().unwrap().is_none(),
            "disconnect must not stop the official app"
        );
        child.kill().await.unwrap();
        let _ = child.wait().await;
    }
}
