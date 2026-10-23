use super::*;
use crate::connections::grok_renewal::{Owner, Renewal};
use std::sync::atomic::{AtomicUsize, Ordering};
struct MockOwner {
    calls: AtomicUsize,
    behavior: &'static str,
}
#[async_trait::async_trait]
impl Owner for MockOwner {
    async fn renew(&self, home: &std::path::Path) -> Result<(), Fault> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.behavior == "fail" {
            return Err(Fault::offline());
        }
        if self.behavior == "wait" {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        write_auth(
            home,
            "synthetic-grok-token-renewed",
            Utc::now().timestamp() + 3600,
            false,
        );
        if self.behavior == "switch" {
            let mut data: Value =
                serde_json::from_slice(&std::fs::read(home.join("auth.json")).unwrap()).unwrap();
            data["https://auth.x.ai::official-client"]["user_id"] = json!("other-fixture");
            std::fs::write(home.join("auth.json"), serde_json::to_vec(&data).unwrap()).unwrap();
        }
        Ok(())
    }
}
fn saved(f: &Fixture, approved: bool) -> Settings {
    let mut c = Connection::new(
        Provider::Grok,
        "user-fixture".into(),
        "grok@example.test".into(),
        format!("grok:{}", hash(&crate::storage::path_key(&f.home))),
    );
    c.mode = if approved {
        ConnectionMode::GrokCliRenewal
    } else {
        ConnectionMode::GrokCli
    };
    c.profile = Some(f.home.to_string_lossy().into_owned());
    Settings {
        connections: vec![c],
        launch_at_startup: true,
        dashboard_pinned: true,
        ..Default::default()
    }
}
fn owner(f: &mut Fixture, behavior: &'static str) -> Arc<MockOwner> {
    let mock = Arc::new(MockOwner {
        calls: AtomicUsize::new(0),
        behavior,
    });
    f.service.grok_renewal = Arc::new(Renewal::with_owner(mock.clone()));
    mock
}
#[tokio::test]
async fn renewal_requires_a_separate_grant_and_a_saved_identity() {
    let mut f = fixture("renewable").await;
    let mock = owner(&mut f, "ok");
    let settings = saved(&f, false);
    let before = std::fs::read(f.home.join("auth.json")).unwrap();
    assert!(f
        .service
        .connect_grok_cli_renewal(&settings, false)
        .await
        .is_err());
    assert!(f.requests.lock().unwrap().is_empty());
    assert_eq!(
        f.service
            .fetch(&settings.connections[0])
            .await
            .unwrap_err()
            .kind,
        FaultKind::NeedsRelogin
    );
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(f.home.join("auth.json")).unwrap(), before);
    assert!(f
        .service
        .connect_grok_cli_renewal(&Settings::default(), true)
        .await
        .is_err());
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn approved_renewal_verifies_same_identity_persists_without_secrets_and_does_not_run_when_valid(
) {
    let mut f = fixture("renewable").await;
    let mock = owner(&mut f, "ok");
    let settings = saved(&f, false);
    f.service.storage.save_settings(&settings).unwrap();
    let (next, state) = f
        .service
        .connect_grok_cli_renewal(&settings, true)
        .await
        .unwrap();
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.status, Status::Ready);
    assert_eq!(
        state.report.as_ref().unwrap().source,
        ReportSource::GrokCliRenewal
    );
    assert_eq!(state.report.as_ref().unwrap().quotas[0].percent, Some(25.));
    assert_eq!(next.connections[0].id, settings.connections[0].id);
    assert!(next.launch_at_startup && next.dashboard_pinned);
    for _ in 0..3 {
        assert!(f.service.fetch(&next.connections[0]).await.is_ok());
    }
    assert_eq!(
        mock.calls.load(Ordering::SeqCst),
        1,
        "no owner operation on a valid bearer"
    );
    f.service.storage.save_settings(&next).unwrap();
    let raw = std::fs::read_to_string(f.service.storage.dir.join("settings.json")).unwrap();
    assert!(!raw.contains("synthetic-grok-token") && !raw.contains("refresh_token"));
    let backup: Settings =
        crate::storage::read_json(&f.service.storage.dir.join("settings.json.bak")).unwrap();
    assert!(
        backup.connections.is_empty(),
        "backup must not resurrect the owner-operation grant"
    );
    let current = std::fs::read(f.home.join("auth.json")).unwrap();
    let disconnected = f
        .service
        .disconnect(&next, &next.connections[0].id)
        .unwrap();
    assert!(disconnected.connections.is_empty());
    assert_eq!(std::fs::read(f.home.join("auth.json")).unwrap(), current);
}
#[tokio::test]
async fn renewal_never_runs_for_permission_failures_invalid_profiles_or_provider_pauses() {
    for mode in ["forbidden", "unknown", "ok"] {
        let mut f = fixture(mode).await;
        let mock = owner(&mut f, "ok");
        let settings = saved(&f, true);
        let _ = f.service.fetch(&settings.connections[0]).await;
        assert_eq!(mock.calls.load(Ordering::SeqCst), 0, "{mode}");
    }
    let mut f = fixture("renewable").await;
    let mock = owner(&mut f, "ok");
    let settings = saved(&f, true);
    f.service.pause(
        Provider::Grok,
        &Fault {
            kind: FaultKind::RateLimited,
            message: "fixture pause".into(),
            retry_after_secs: Some(120),
        },
    );
    assert_eq!(
        f.service
            .fetch(&settings.connections[0])
            .await
            .unwrap_err()
            .kind,
        FaultKind::RateLimited
    );
    assert!(f.requests.lock().unwrap().is_empty());
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    for kind in ["issuer", "subject", "missing"] {
        let mut f = fixture("renewable").await;
        let mock = owner(&mut f, "ok");
        let settings = saved(&f, true);
        let path = f.home.join("auth.json");
        let mut data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if kind == "missing" {
            std::fs::remove_file(&path).unwrap();
        } else {
            data["https://auth.x.ai::official-client"][if kind == "issuer" {
                "oidc_issuer"
            } else {
                "user_id"
            }] = json!("different");
            std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        }
        assert!(f.service.fetch(&settings.connections[0]).await.is_err());
        assert_eq!(mock.calls.load(Ordering::SeqCst), 0, "{kind}");
    }
}
#[tokio::test]
async fn failed_or_switched_renewal_never_saves_a_grant_or_reuses_rejected_quota() {
    for behavior in ["fail", "switch"] {
        let mut f = fixture("renewable").await;
        let mock = owner(&mut f, behavior);
        let settings = saved(&f, false);
        f.service.storage.save_settings(&settings).unwrap();
        let before = std::fs::read(f.service.storage.dir.join("settings.json")).unwrap();
        assert!(f
            .service
            .connect_grok_cli_renewal(&settings, true)
            .await
            .is_err());
        assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read(f.service.storage.dir.join("settings.json")).unwrap(),
            before
        );
    }
    let f = fixture("ok").await;
    let settings = saved(&f, true);
    let report = f.service.fetch(&settings.connections[0]).await.unwrap();
    let previous =
        AccountState::apply(settings.connections[0].account(&settings), Ok(report), None);
    let failed = AccountState::apply(
        previous.account.clone(),
        Err(Fault::offline()),
        Some(&previous),
    );
    assert!(
        failed.report.is_none(),
        "an owner-maintained session must not reuse quota after failed verification"
    );
}
#[tokio::test]
async fn renewal_is_throttled_serialized_with_interactive_login_and_cancellable() {
    let mut f = fixture("renewable").await;
    let mock = owner(&mut f, "fail");
    let settings = saved(&f, true);
    let guard = f.service.grok_owner.clone().lock_owned().await;
    assert!(f.service.fetch(&settings.connections[0]).await.is_err());
    assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    drop(guard);
    for _ in 0..3 {
        assert!(f.service.fetch(&settings.connections[0]).await.is_err());
    }
    assert_eq!(
        mock.calls.load(Ordering::SeqCst),
        1,
        "bounded owner retry, not every quota poll"
    );
    let mock = owner(&mut f, "wait");
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        f.service.connect_grok_cli_renewal(&settings, true),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    assert!(f.service.grok_owner.try_lock().is_ok());
    assert!(!f.service.storage.dir.join("settings.json").exists());
    assert_eq!(
        read(&f.home).unwrap().token.as_str(),
        "synthetic-grok-token-first"
    );
}
#[tokio::test]
async fn engine_restart_keeps_only_primary_grant_and_rechecks_quota() {
    let mut f = fixture("renewable").await;
    let mock = owner(&mut f, "ok");
    let settings = saved(&f, true);
    f.service.storage.save_settings(&settings).unwrap();
    let engine = crate::engine::Engine::start_with_service(
        f.service.storage.clone(),
        settings,
        f.service.clone(),
    );
    let mut state = engine.state.clone();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if state
                .borrow()
                .accounts
                .iter()
                .any(|a| a.account.provider == Provider::Grok && a.status == Status::Ready)
            {
                break;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(mock.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.settings.borrow().connections[0].mode,
        ConnectionMode::GrokCliRenewal
    );
    engine.shutdown().await;
}
