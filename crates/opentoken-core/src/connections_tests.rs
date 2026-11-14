use super::*;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

const SECRET: &str = "synthetic-secret-token-first";
#[derive(Default)]
pub(super) struct MemoryVault {
    values: Mutex<HashMap<String, String>>,
    fail_write: AtomicBool,
    fail_delete: AtomicBool,
}
impl CredentialStore for MemoryVault {
    fn read(&self, id: &str) -> Result<Option<String>> {
        Ok(self.values.lock().unwrap().get(id).cloned())
    }
    fn write(&self, id: &str, secret: &str) -> Result<()> {
        if self.fail_write.load(Ordering::SeqCst) {
            bail!("Synthetic secure-write failure");
        }
        self.values.lock().unwrap().insert(id.into(), secret.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            bail!("Synthetic secure-delete failure");
        }
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}
#[derive(Clone)]
struct Fixture {
    mode: Arc<Mutex<&'static str>>,
    calls: Arc<Mutex<Vec<String>>>,
    profiles: Arc<AtomicUsize>,
}
async fn handler(
    State(f): State<Fixture>,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let path = uri.path();
    let mode = *f.mode.lock().unwrap();
    f.calls.lock().unwrap().push(uri.to_string());
    assert!(headers.get("authorization").is_some());
    if path.starts_with("/teams") {
        assert!(headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("Basic "));
    } else {
        assert!(headers["authorization"].to_str().unwrap().starts_with(
            if path == "/copilot_internal/user" {
                "token "
            } else {
                "Bearer "
            }
        ));
        assert_eq!(headers["x-github-api-version"], "2026-03-10");
    }
    if mode == "quota_transient"
        && path == "/copilot_internal/user"
        && f.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == path)
            .count()
            == 1
    {
        return (StatusCode::BAD_GATEWAY, SECRET).into_response();
    }
    if mode == "identity_transient"
        && path == "/user"
        && f.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == path)
            .count()
            == 1
    {
        return (StatusCode::SERVICE_UNAVAILABLE, SECRET).into_response();
    }
    match mode {
        "offline" => return (StatusCode::SERVICE_UNAVAILABLE, SECRET).into_response(),
        "server_pause" => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("retry-after", "120")],
                SECRET,
            )
                .into_response()
        }
        "unauthorized" => return (StatusCode::UNAUTHORIZED, SECRET).into_response(),
        "forbidden" => return (StatusCode::FORBIDDEN, SECRET).into_response(),
        "throttled" => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "120")],
                SECRET,
            )
                .into_response()
        }
        "redirect" => {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", "/must-not-follow")],
            )
                .into_response()
        }
        "giant" => return Body::from("x".repeat(2 * 1024 * 1024 + 1)).into_response(),
        "malformed" => return "not-json".into_response(),
        _ => {}
    }
    if path == "/user" {
        let count = f.profiles.fetch_add(1, Ordering::SeqCst) + 1;
        return Json(
            json!({"id":if mode=="switch" && count>=3 { 202 } else {101},"login":"fixture-user"}),
        )
        .into_response();
    }
    if path == "/copilot_internal/user" {
        return Json(if mode == "unlimited" {json!({"copilot_plan":"business","token_based_billing":true,"quota_snapshots":{"premium_interactions":{"unlimited":true,"percent_remaining":100,"credits_used":12}}})} else {json!({"copilot_plan":"pro","quota_reset_date":"2030-01-01","quota_snapshots":{"premium_interactions":{"entitlement":300,"percent_remaining":72},"chat":{"unlimited":true,"percent_remaining":100},"completions":{"entitlement":0,"percent_remaining":100}}})}).into_response();
    }
    if path.contains("/settings/billing/") {
        if mode == "no_support" {
            return StatusCode::NOT_FOUND.into_response();
        }
        let credit = path.contains("ai_credit");
        let now = Utc::now();
        let items = if mode == "empty" {
            vec![]
        } else {
            vec![
                json!({"product":if mode=="other_product" {"Actions"} else {"Copilot"},"unitType":if mode=="wrong_unit" { "tokens" } else if credit { "ai-credits" } else { "requests" },"grossQuantity":if mode=="negative" {-1} else if credit {3} else {42},"netAmount":if credit {0.03} else {0.4}}),
                json!({"product":"Actions","unitType":"minutes","grossQuantity":999,"netAmount":999.0}),
            ]
        };
        return Json(json!({"user":if mode=="wrong_user" {"another-user"} else {"fixture-user"},"timePeriod":{"year":now.year(),"month":if mode=="bad_time" {0} else {now.month()}},"usageItems":items})).into_response();
    }
    if path == "/teams/members" {
        let member = json!({"id":if mode=="wrong_user" {"user_other"} else {"user_fixture"},"email":"member@example.test","isRemoved":mode=="removed"});
        return Json(json!({"teamMembers":if mode=="duplicate" {vec![member.clone(),member]} else {vec![member]}})).into_response();
    }
    if path == "/teams/spend" {
        let request: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(request["searchTerm"], "member@example.test");
        assert_eq!(request["pageSize"], 100);
        let page = request["page"].as_u64().unwrap();
        let exact = !(mode == "paginated" && page == 1);
        let mut row = json!({"userId":if exact {"user_fixture"} else {"user_other"},"email":if exact {"member@example.test"} else {"member+other@example.test"},"spendCents":if mode=="negative" {-1.0} else {2450.125487},"overallSpendCents":3200.750456,"effectivePerUserLimitDollars":if mode=="no_budget" {Value::Null} else {json!(100)}});
        if mode == "missing" {
            row.as_object_mut().unwrap().remove("spendCents");
        }
        return Json(json!({"teamMemberSpend":if mode=="empty" {vec![]} else {vec![row]},"totalPages":if mode=="too_many_pages" {11} else if mode=="paginated" {2} else {1}})).into_response();
    }
    panic!("Unexpected fixture endpoint");
}
struct Harness {
    service: Service,
    vault: Arc<MemoryVault>,
    fixture: Fixture,
    _dir: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn harness(mode: &'static str) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let fixture = Fixture {
        mode: Arc::new(Mutex::new(mode)),
        calls: Default::default(),
        profiles: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .fallback(any(handler))
        .with_state(fixture.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut service = Service::new(
        Storage {
            dir: dir.path().join("data"),
            user_home: dir.path().into(),
        },
        vault.clone(),
    );
    // Private fields: endpoint overrides exist only in this compiled test module, never IPC/env/settings.
    service.cursor_base = base.clone();
    service.github_base = base;
    Harness {
        service,
        vault,
        fixture,
        _dir: dir,
        server,
    }
}
#[tokio::test]
async fn github_cli_connects_real_fixture_quota_without_vault_copy_and_disconnects_monitoring_only()
{
    let mut h = harness("ok").await;
    h.service.github_test_token = Some(Arc::new(Mutex::new(SECRET.into())));
    assert!(h.service.github_sign_in_marker(false).await.is_err());
    assert!(h
        .service
        .connect_github_personal(&Settings::default(), false)
        .await
        .is_err());
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    let (settings, state) = h
        .service
        .connect_github_personal(
            &Settings {
                launch_at_startup: true,
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap();
    assert_eq!(state.account.provider, Provider::Copilot);
    assert_eq!(
        state.report.as_ref().unwrap().source,
        ReportSource::CopilotInternal
    );
    assert_eq!(state.report.as_ref().unwrap().quotas.len(), 1);
    assert_eq!(state.report.as_ref().unwrap().quotas[0].percent, Some(28.));
    assert!(settings.launch_at_startup);
    assert_eq!(settings.connections[0].mode, ConnectionMode::GithubCli);
    assert!(h.vault.values.lock().unwrap().is_empty());
    assert!(!serde_json::to_string(&settings).unwrap().contains(SECRET));
    assert_eq!(
        h.service
            .fetch(&settings.connections[0])
            .await
            .unwrap()
            .quotas[0]
            .percent,
        Some(28.)
    );
    let removed = h
        .service
        .disconnect(&settings, &settings.connections[0].id)
        .unwrap();
    assert!(removed.connections.is_empty());
    assert_eq!(
        h.service
            .github_test_token
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .as_str(),
        SECRET
    );
}
#[tokio::test]
async fn github_transient_identity_and_quota_reads_retry_once_without_another_login_or_write() {
    for (mode, path, calls) in [
        ("identity_transient", "/user", 4),
        ("quota_transient", "/copilot_internal/user", 2),
    ] {
        let mut h = harness(mode).await;
        h.service.github_test_token = Some(Arc::new(Mutex::new(SECRET.into())));
        let (settings, state) = h
            .service
            .connect_github_personal(&Settings::default(), true)
            .await
            .unwrap();
        assert_eq!(state.status, Status::Ready);
        assert_eq!(
            h.fixture
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|p| p.as_str() == path)
                .count(),
            calls
        );
        assert!(h.vault.values.lock().unwrap().is_empty());
        assert_eq!(settings.connections.len(), 1);
    }
}
#[tokio::test]
async fn github_retry_is_bounded_keeps_cache_and_never_retries_rejected_or_paused_requests() {
    let mut h = harness("ok").await;
    h.service.github_test_token = Some(Arc::new(Mutex::new(SECRET.into())));
    let (settings, state) = h
        .service
        .connect_github_personal(&Settings::default(), true)
        .await
        .unwrap();
    let file = h.service.storage.dir.join("settings.json");
    let saved = std::fs::read(&file).unwrap();
    for (mode, kind, count) in [
        ("offline", FaultKind::Offline, 2),
        ("unauthorized", FaultKind::NeedsRelogin, 1),
        ("forbidden", FaultKind::PermissionDenied, 1),
        ("redirect", FaultKind::Offline, 1),
        ("malformed", FaultKind::Unreadable, 1),
        ("server_pause", FaultKind::RateLimited, 1),
    ] {
        *h.fixture.mode.lock().unwrap() = mode;
        h.fixture.calls.lock().unwrap().clear();
        let fault = h.service.fetch(&settings.connections[0]).await.unwrap_err();
        assert_eq!(fault.kind, kind, "{mode}");
        assert!(
            fault.message.starts_with("GitHub identity check:"),
            "{mode}"
        );
        assert!(!fault.message.contains(SECRET));
        assert_eq!(h.fixture.calls.lock().unwrap().len(), count, "{mode}");
        let next = AccountState::apply(state.account.clone(), Err(fault), Some(&state));
        if matches!(kind, FaultKind::NeedsRelogin | FaultKind::PermissionDenied) {
            assert!(next.report.is_none());
        } else {
            assert_eq!(next.status, Status::Stale);
        }
        assert_eq!(std::fs::read(&file).unwrap(), saved);
    }
    // A provider pause suppresses subsequent reads, including manual/reconnect paths.
    h.fixture.calls.lock().unwrap().clear();
    let fault = h.service.fetch(&settings.connections[0]).await.unwrap_err();
    assert_eq!(fault.kind, FaultKind::RateLimited);
    assert!(h.fixture.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn session_retry_cancellation_stops_before_a_second_request_and_rejects_non_get() {
    let h = harness("offline").await;
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        h.service.session_json::<Value>(
            h.service.github_request("/user", SECRET),
            Provider::Copilot,
            "GitHub identity check",
        ),
    )
    .await;
    assert!(result.is_err());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.fixture.calls.lock().unwrap().len(), 1);
    let fault = h
        .service
        .session_json::<Value>(
            h.service
                .http
                .post(format!("{}/user", h.service.github_base)),
            Provider::Copilot,
            "invalid method",
        )
        .await
        .unwrap_err();
    assert_eq!(fault.kind, FaultKind::Unreadable);
    assert_eq!(h.fixture.calls.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn github_cli_identity_switch_and_unauthorized_never_persist() {
    for mode in ["switch", "unauthorized", "forbidden"] {
        let mut h = harness(mode).await;
        h.service.github_test_token = Some(Arc::new(Mutex::new(SECRET.into())));
        assert!(h
            .service
            .connect_github_personal(&Settings::default(), true)
            .await
            .is_err());
        assert!(h.vault.values.lock().unwrap().is_empty());
        assert!(!h.service.storage.dir.join("settings.json").exists());
    }
}
#[tokio::test]
async fn github_unlimited_credits_are_not_invented_percent_left() {
    let mut h = harness("unlimited").await;
    h.service.github_test_token = Some(Arc::new(Mutex::new(SECRET.into())));
    let (_, state) = h
        .service
        .connect_github_personal(&Settings::default(), true)
        .await
        .unwrap();
    let r = state.report.unwrap();
    assert_eq!(r.quotas.len(), 1);
    assert!(r.quotas[0].percent.is_none());
    assert_eq!(r.quotas[0].value, Some(12.));
}
fn input(provider: Provider) -> ConnectInput {
    ConnectInput {
        provider,
        region: String::new(),
        token: SECRET.into(),
        member_email: "member@example.test".into(),
        session: false,
    }
}
#[tokio::test]
async fn personal_copilot_connects_reads_two_separate_units_and_never_serializes_the_secret() {
    let h = harness("ok").await;
    let (settings, state) = h
        .service
        .connect(&Settings::default(), input(Provider::Copilot))
        .await
        .unwrap();
    assert_eq!(state.account.provider, Provider::Copilot);
    assert!(state.account.sources.is_empty());
    let r = state.report.unwrap();
    assert_eq!(r.quotas.len(), 4);
    assert_eq!(r.quotas[0].value, Some(42.0));
    assert_eq!(r.quotas[2].value, Some(3.0));
    assert!(r
        .quotas
        .iter()
        .all(|q| q.percent.is_none() && q.resets_at.is_none()));
    assert!(serde_json::to_string(&settings)
        .unwrap()
        .find(SECRET)
        .is_none());
    assert!(
        !std::fs::read_to_string(h.service.storage.dir.join("settings.json"))
            .unwrap()
            .contains(SECRET)
    );
    let c = &settings.connections[0];
    assert_eq!(c.subject, "101");
    assert_eq!(c.scope, "personal");
    assert_eq!(h.vault.read(&c.id).unwrap().as_deref(), Some(SECRET));
    assert!(h
        .fixture
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|p| p.contains("year=") && p.contains("month=")));
    assert!(h.service.fetch(c).await.is_ok());
    let next = h.service.disconnect(&settings, &c.id).unwrap();
    assert!(next.connections.is_empty());
    assert!(h.vault.read(&c.id).unwrap().is_none());
}
#[tokio::test]
async fn cursor_verifies_exact_member_and_retains_fractional_reported_dollars_and_budget() {
    let h = harness("ok").await;
    let (settings, state) = h
        .service
        .connect(&Settings::default(), input(Provider::Cursor))
        .await
        .unwrap();
    let r = state.report.unwrap();
    assert!((r.quotas[0].percent.unwrap() - 24.50125487).abs() < 1e-8);
    assert!((r.quotas[1].value.unwrap() - 24.50125487).abs() < 1e-8);
    assert_eq!(settings.connections[0].scope, hash(SECRET));
    assert_eq!(state.account.email.as_deref(), Some("member@example.test"));
    assert!(h.service.fetch(&settings.connections[0]).await.is_ok());
}
#[tokio::test]
async fn cursor_pagination_never_uses_a_fuzzy_match() {
    let h = harness("paginated").await;
    let (_, state) = h
        .service
        .connect(&Settings::default(), input(Provider::Cursor))
        .await
        .unwrap();
    assert!(state.report.unwrap().quotas[0].percent.is_some());
    assert_eq!(
        h.fixture
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == "/teams/spend")
            .count(),
        2
    );
}
#[tokio::test]
async fn absent_cursor_limit_never_becomes_an_invented_allowance() {
    let h = harness("no_budget").await;
    let (_, state) = h
        .service
        .connect(&Settings::default(), input(Provider::Cursor))
        .await
        .unwrap();
    let q = state.report.unwrap().quotas;
    assert_eq!(q.len(), 2);
    assert!(q.iter().all(|q| q.percent.is_none()));
}
#[tokio::test]
async fn empty_or_unrelated_copilot_reports_are_not_zero_usage() {
    for mode in ["empty", "other_product"] {
        let h = harness(mode).await;
        let (_, state) = h
            .service
            .connect(&Settings::default(), input(Provider::Copilot))
            .await
            .unwrap();
        assert!(state.report.unwrap().quotas.is_empty());
    }
}
#[tokio::test]
async fn invalid_access_schema_scope_and_units_never_persist_credentials_or_usage() {
    for provider in [Provider::Cursor, Provider::Copilot] {
        let modes = if provider == Provider::Cursor {
            vec![
                "unauthorized",
                "forbidden",
                "throttled",
                "redirect",
                "giant",
                "malformed",
                "negative",
                "wrong_user",
                "duplicate",
                "removed",
                "missing",
                "empty",
                "too_many_pages",
            ]
        } else {
            vec![
                "unauthorized",
                "forbidden",
                "throttled",
                "redirect",
                "giant",
                "malformed",
                "negative",
                "wrong_user",
                "switch",
                "wrong_unit",
                "bad_time",
                "no_support",
            ]
        };
        for mode in modes {
            let h = harness(mode).await;
            let error = h
                .service
                .connect(&Settings::default(), input(provider))
                .await
                .err()
                .unwrap_or_else(|| panic!("Unexpected success: {mode}"));
            assert!(!error.to_string().contains(SECRET));
            assert!(h.vault.values.lock().unwrap().is_empty());
            assert!(!h.service.storage.dir.join("settings.json").exists());
            if mode == "redirect" {
                assert_eq!(h.fixture.calls.lock().unwrap().len(), 1);
            }
            if mode == "throttled" {
                assert_eq!(
                    error.downcast_ref::<Fault>().unwrap().retry_after_secs,
                    Some(120)
                );
            }
        }
    }
}
#[tokio::test]
async fn reconnect_spam_and_provider_backoff_do_not_issue_more_requests() {
    for mode in ["ok", "throttled"] {
        let h = harness(mode).await;
        let _ = h
            .service
            .connect(&Settings::default(), input(Provider::Copilot))
            .await;
        let count = h.fixture.calls.lock().unwrap().len();
        assert!(h
            .service
            .connect(&Settings::default(), input(Provider::Copilot))
            .await
            .is_err());
        assert_eq!(h.fixture.calls.lock().unwrap().len(), count);
    }
}
#[tokio::test]
async fn failed_settings_writes_restore_previous_secure_credentials() {
    let mut h = harness("ok").await;
    let (settings, _) = h
        .service
        .connect(&Settings::default(), input(Provider::Copilot))
        .await
        .unwrap();
    h.service.gates.lock().unwrap().clear();
    let blocked = h._dir.path().join("blocked");
    std::fs::write(&blocked, b"file").unwrap();
    h.service.storage.dir = blocked;
    let mut new = input(Provider::Copilot);
    new.token = "synthetic-secret-token-second".into();
    assert!(h.service.connect(&settings, new).await.is_err());
    assert_eq!(
        h.vault
            .read(&settings.connections[0].id)
            .unwrap()
            .as_deref(),
        Some(SECRET)
    );
    assert!(h
        .service
        .disconnect(&settings, &settings.connections[0].id)
        .is_err());
    assert_eq!(
        h.vault
            .read(&settings.connections[0].id)
            .unwrap()
            .as_deref(),
        Some(SECRET)
    );
}
#[tokio::test]
async fn failed_vault_changes_do_not_save_or_remove_metadata() {
    let h = harness("ok").await;
    h.vault.fail_write.store(true, Ordering::SeqCst);
    assert!(h
        .service
        .connect(&Settings::default(), input(Provider::Copilot))
        .await
        .is_err());
    assert!(!h.service.storage.dir.join("settings.json").exists());
    h.vault.fail_write.store(false, Ordering::SeqCst);
    h.service.gates.lock().unwrap().clear();
    let (settings, _) = h
        .service
        .connect(&Settings::default(), input(Provider::Copilot))
        .await
        .unwrap();
    h.vault.fail_delete.store(true, Ordering::SeqCst);
    assert!(h
        .service
        .disconnect(&settings, &settings.connections[0].id)
        .is_err());
    assert_eq!(
        h.service.storage.load_settings().unwrap().connections,
        settings.connections
    );
}
#[tokio::test]
async fn tampered_identity_missing_credentials_and_revoked_permissions_clear_cached_readings() {
    let h = harness("ok").await;
    let (settings, previous) = h
        .service
        .connect(&Settings::default(), input(Provider::Cursor))
        .await
        .unwrap();
    let c = &settings.connections[0];
    h.vault
        .write(&c.id, "synthetic-different-team-key")
        .unwrap();
    let result = h.service.fetch(c).await;
    assert_eq!(
        result.as_ref().unwrap_err().kind,
        FaultKind::IdentityChanged
    );
    assert!(
        AccountState::apply(c.account(&settings), result, Some(&previous))
            .report
            .is_none()
    );
    h.vault.delete(&c.id).unwrap();
    assert_eq!(
        h.service.fetch(c).await.unwrap_err().kind,
        FaultKind::NotSignedIn
    );
    h.vault.write(&c.id, SECRET).unwrap();
    *h.fixture.mode.lock().unwrap() = "forbidden";
    let result = h.service.fetch(c).await;
    assert!(
        AccountState::apply(c.account(&settings), result, Some(&previous))
            .report
            .is_none()
    );
}
#[test]
fn metadata_validation_and_key_scopes_prevent_ambiguous_connections() {
    let c = Connection::new(
        Provider::Cursor,
        "user_fixture".into(),
        "member@example.test".into(),
        hash(SECRET),
    );
    assert!(validate_connections(std::slice::from_ref(&c)).is_ok());
    assert!(validate_connections(&[c.clone(), c.clone()]).is_err());
    let other = Connection::new(
        Provider::Cursor,
        c.subject.clone(),
        c.login.clone(),
        hash("another-key"),
    );
    assert_ne!(other.id, c.id);
    let mut bad = c;
    bad.provider = Provider::Codex;
    assert!(validate_connections(&[bad]).is_err());
    assert!(Settings {
        codex_enabled: false,
        claude_enabled: false,
        ..Default::default()
    }
    .validate()
    .is_ok());
}
#[tokio::test]
async fn engine_commands_cache_api_restart_and_disconnect_use_the_same_verified_connection() {
    let h = harness("ok").await;
    let initial = Settings {
        codex_enabled: false,
        claude_enabled: false,
        auto_discover: false,
        live_tracking: false,
        ..Default::default()
    };
    let engine = crate::engine::Engine::start_with_service(
        h.service.storage.clone(),
        initial.clone(),
        h.service.clone(),
    );
    let configured = engine.connect(input(Provider::Copilot)).await.unwrap();
    assert_eq!(configured.connections.len(), 1);
    assert_eq!(engine.state.borrow().accounts[0].status, Status::Ready);
    // A stale general preference save cannot silently erase secure connection metadata.
    let mut stale = initial;
    stale.theme = "dark".into();
    engine.save(stale).await.unwrap();
    assert_eq!(engine.settings.borrow().connections.len(), 1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let api_engine = engine.clone();
    let api = tokio::spawn(async move {
        axum::serve(listener, crate::api::router(api_engine))
            .await
            .unwrap();
    });
    let response = reqwest::get(format!("http://{addr}/v1/usage/copilot"))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let public = response.text().await.unwrap();
    assert!(public.contains("42.0"));
    assert!(!public.contains(SECRET));
    assert!(!public.contains("memberEmail"));
    api.abort();
    engine.shutdown().await;
    let saved = h.service.storage.load_settings().unwrap();
    assert_eq!(saved.theme, "dark");
    assert_eq!(saved.connections.len(), 1);
    let restored = crate::engine::Engine::start_with_service(
        h.service.storage.clone(),
        saved,
        h.service.clone(),
    );
    let mut rx = restored.state.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if rx
                .borrow()
                .accounts
                .iter()
                .any(|a| a.status == Status::Ready)
            {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let id = restored.settings.borrow().connections[0].id.clone();
    restored.disconnect(id.clone()).await.unwrap();
    assert!(restored.state.borrow().accounts.is_empty());
    assert!(restored.settings.borrow().connections.is_empty());
    assert!(!restored.settings.borrow().codex_enabled);
    assert!(h.vault.read(&id).unwrap().is_none());
    restored.shutdown().await;
    let cache = std::fs::read_to_string(h.service.storage.dir.join("state.json")).unwrap();
    assert!(!cache.contains(SECRET));
}
#[tokio::test]
async fn verification_hourly_budget_and_shared_provider_pause_are_enforced() {
    let h = harness("ok").await;
    h.service.gates.lock().unwrap().insert(
        Provider::Copilot,
        Gate {
            attempts: std::iter::repeat_n(tokio::time::Instant::now(), 20).collect(),
            ..Default::default()
        },
    );
    assert!(h
        .service
        .connect(&Settings::default(), input(Provider::Copilot))
        .await
        .unwrap_err()
        .to_string()
        .contains("20 attempts"));
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    let c = Connection::new(
        Provider::Copilot,
        "101".into(),
        "fixture-user".into(),
        "personal".into(),
    );
    h.vault.write(&c.id, SECRET).unwrap();
    h.service.pause(
        Provider::Copilot,
        &Fault {
            kind: FaultKind::RateLimited,
            message: "pause".into(),
            retry_after_secs: Some(120),
        },
    );
    assert_eq!(
        h.service.fetch(&c).await.unwrap_err().kind,
        FaultKind::RateLimited
    );
    assert!(h.fixture.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn unsupported_and_invalid_credentials_fail_before_any_network_or_write() {
    let h = harness("ok").await;
    for provider in [
        Provider::Codex,
        Provider::Claude,
        Provider::Cursor,
        Provider::Copilot,
    ] {
        let mut request = input(provider);
        request.token = "short".into();
        assert!(h
            .service
            .connect(&Settings::default(), request)
            .await
            .is_err());
    }
    let mut request = input(Provider::Cursor);
    request.member_email = "not an email".into();
    assert!(h
        .service
        .connect(&Settings::default(), request)
        .await
        .is_err());
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    assert!(h.vault.values.lock().unwrap().is_empty());
}
