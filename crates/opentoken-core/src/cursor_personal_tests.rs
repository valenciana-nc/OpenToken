use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use serde_json::json;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
fn token(subject: &str, expires: i64) -> String {
    format!(
        "e30.{}.signature",
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({"sub":format!("auth0|{subject}"),"exp":expires})).unwrap()
        )
    )
}
fn live_token() -> String {
    token("user_fixture", Utc::now().timestamp() + 3600)
}
fn database(path: &Path, token: &str, wal: bool) -> Database {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = Database::open(path).unwrap();
    if wal {
        db.pragma_update(None, "journal_mode", "wal").unwrap();
    }
    db.execute_batch("CREATE TABLE IF NOT EXISTS ItemTable(key TEXT PRIMARY KEY,value);")
        .unwrap();
    db.execute(
        "INSERT OR REPLACE INTO ItemTable VALUES ('cursorAuth/accessToken',?1)",
        [token],
    )
    .unwrap();
    db
}
struct NoVault(AtomicUsize);
impl CredentialStore for NoVault {
    fn read(&self, _: &str) -> Result<Option<String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        bail!("The Cursor app connector must not read the vault")
    }
    fn write(&self, _: &str, _: &str) -> Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        bail!("No vault writes")
    }
    fn delete(&self, _: &str) -> Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        bail!("No vault deletion")
    }
}
#[derive(Clone)]
struct Fixture {
    mode: Arc<Mutex<&'static str>>,
    calls: Arc<Mutex<Vec<String>>>,
    identities: Arc<AtomicUsize>,
    path: PathBuf,
}
async fn handler(State(f): State<Fixture>, uri: Uri, headers: HeaderMap) -> Response {
    f.calls.lock().unwrap().push(uri.path().to_owned());
    assert!(headers.get("authorization").is_none());
    assert!(headers["cookie"]
        .to_str()
        .unwrap()
        .starts_with("WorkosCursorSessionToken=user_fixture%3A%3A"));
    let mode = *f.mode.lock().unwrap();
    match mode {
        "401" => {
            return (StatusCode::UNAUTHORIZED, "secret response must not leak").into_response()
        }
        "403" => return StatusCode::FORBIDDEN.into_response(),
        "429" => return (StatusCode::TOO_MANY_REQUESTS, [("retry-after", "120")]).into_response(),
        "redirect" => {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", "/do-not-follow")],
            )
                .into_response()
        }
        "giant" => return "x".repeat(2 * 1024 * 1024 + 1).into_response(),
        "malformed" => return "invalid-json".into_response(),
        _ => {}
    }
    match uri.path() {
        "/api/auth/me" => {
            let n = f.identities.fetch_add(1, Ordering::SeqCst);
            let mut body = json!({"sub":if mode=="wrong_user" || mode=="remote_switch" && n>=1 {"user_other"} else {"user_fixture"},"email":if mode=="bad_email" {"not an email"} else {"personal@example.test"}});
            if mode == "numeric_id" {
                body["id"] = json!(417);
            }
            Json(body).into_response()
        }
        "/api/usage-summary" => {
            if mode == "local_switch" {
                drop(database(
                    &f.path,
                    &token("user_other", Utc::now().timestamp() + 3600),
                    false,
                ));
            }
            if mode == "local_rotation" {
                drop(database(
                    &f.path,
                    &token("user_fixture", Utc::now().timestamp() + 7200),
                    false,
                ));
            }
            let mut body = json!({"membershipType":"pro","billingCycleStart":"2026-10-01T00:00:00Z","billingCycleEnd":"2026-11-01T00:00:00Z","individualUsage":{"plan":{"totalPercentUsed":25.0,"autoPercentUsed":10.0,"apiPercentUsed":42.0}}});
            if mode == "negative" {
                body["individualUsage"]["plan"]["apiPercentUsed"] = json!(-1);
            }
            if mode == "bad_date" {
                body["billingCycleEnd"] = json!("not-a-date");
            }
            if mode == "unknown" {
                body = json!({"error":"private-schema-changed"});
            }
            if mode == "empty" {
                body = json!({"membershipType":"pro","teamUsage":{"pooled":{"used":5,"limit":10}}});
            }
            Json(body).into_response()
        }
        _ => panic!("Unexpected private endpoint"),
    }
}
struct Harness {
    service: Service,
    fixture: Fixture,
    vault: Arc<NoVault>,
    dir: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn harness(mode: &'static str) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("roaming/Cursor/User/globalStorage/state.vscdb");
    drop(database(&path, &live_token(), false));
    let fixture = Fixture {
        mode: Arc::new(Mutex::new(mode)),
        calls: Default::default(),
        identities: Default::default(),
        path: path.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .fallback(any(handler))
        .with_state(fixture.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let vault = Arc::new(NoVault(AtomicUsize::new(0)));
    let mut service = Service::new(
        Storage {
            dir: dir.path().join("opentoken"),
            user_home: dir.path().into(),
        },
        vault.clone(),
    );
    service.personal = Arc::new(Personal {
        path,
        base,
        fingerprint: Mutex::new(None),
        checked: Mutex::new(None),
    });
    Harness {
        service,
        fixture,
        vault,
        dir,
        server,
    }
}
#[test]
fn readonly_lookup_preserves_database_and_does_not_create_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    let jwt = live_token();
    drop(database(&path, &jwt, true));
    assert!(!sidecar(&path, "-wal").exists());
    let before = std::fs::read(&path).unwrap();
    assert_eq!(read_token(&path).unwrap().as_str(), jwt);
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert!(!sidecar(&path, "-wal").exists());
    assert!(!sidecar(&path, "-shm").exists());
}
#[test]
fn live_wal_reads_current_committed_token_without_copying_or_checkpointing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    let writer = database(&path, &live_token(), true);
    let newer = token("user_new", Utc::now().timestamp() + 3600);
    writer
        .execute(
            "UPDATE ItemTable SET value=?1 WHERE key='cursorAuth/accessToken'",
            [&newer],
        )
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let wal = std::fs::read(sidecar(&path, "-wal")).unwrap();
    assert_eq!(read_token(&path).unwrap().as_str(), newer);
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert_eq!(wal, std::fs::read(sidecar(&path, "-wal")).unwrap());
}
#[test]
fn sqlite_text_utf8_and_utf16_blobs_are_supported_without_reading_other_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    let jwt = live_token();
    let db = database(&path, &jwt, false);
    db.execute(
        "INSERT INTO ItemTable VALUES ('unrelated-private-content',zeroblob(1000000))",
        [],
    )
    .unwrap();
    drop(db);
    assert_eq!(read_token(&path).unwrap().as_str(), jwt);
    for bytes in [
        jwt.as_bytes().to_vec(),
        jwt.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ] {
        let db = Database::open(&path).unwrap();
        db.execute(
            "UPDATE ItemTable SET value=?1 WHERE key='cursorAuth/accessToken'",
            [bytes],
        )
        .unwrap();
        drop(db);
        assert_eq!(read_token(&path).unwrap().as_str(), jwt);
    }
}
#[test]
fn absent_corrupt_oversized_or_incomplete_sources_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    assert_eq!(read_token(&path).unwrap_err().kind, FaultKind::NotSignedIn);
    drop(database(&path, &"x".repeat(40000), false));
    assert!(read_token(&path).is_err());
    std::fs::write(&path, b"not a sqlite database").unwrap();
    assert_eq!(read_token(&path).unwrap_err().kind, FaultKind::Unreadable);
    std::fs::remove_file(&path).unwrap();
    drop(database(&path, &live_token(), false));
    std::fs::write(sidecar(&path, "-wal"), b"unpaired").unwrap();
    assert!(read_token(&path).is_err());
    assert!(!sidecar(&path, "-shm").exists());
    assert!(read_token(Path::new("relative/state.vscdb")).is_err());
}
#[test]
fn active_rollback_journals_are_not_read_as_immutable_committed_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    let writer = database(&path, &live_token(), false);
    writer.execute_batch("BEGIN IMMEDIATE; UPDATE ItemTable SET value='uncommitted' WHERE key='cursorAuth/accessToken';").unwrap();
    assert!(sidecar(&path, "-journal").exists());
    assert_eq!(read_token(&path).unwrap_err().kind, FaultKind::Unreadable);
    writer.execute_batch("ROLLBACK").unwrap();
    assert!(read_token(&path).is_ok());
}
#[test]
fn old_api_metadata_defaults_correctly_and_duplicate_personal_profiles_are_rejected() {
    let old = Connection::new(
        Provider::Cursor,
        "user_enterprise".into(),
        "member@example.test".into(),
        hash("enterprise-key"),
    );
    let mut value = serde_json::to_value(&old).unwrap();
    value.as_object_mut().unwrap().remove("mode");
    let restored: Connection = serde_json::from_value(value).unwrap();
    assert_eq!(restored.mode, ConnectionMode::Api);
    assert!(restored.profile.is_none());
    assert!(super::super::validate_connections(&[restored]).is_ok());
    let profile = std::env::temp_dir().join("fixture/Cursor/User/globalStorage/state.vscdb");
    let path = profile.to_string_lossy().into_owned();
    let scope = format!("app:{}", hash(&crate::storage::path_key(&profile)));
    let mut a = Connection::new(
        Provider::Cursor,
        "user_a".into(),
        "a@example.test".into(),
        scope.clone(),
    );
    a.mode = ConnectionMode::CursorApp;
    a.profile = Some(path.clone());
    let mut b = Connection::new(
        Provider::Cursor,
        "user_b".into(),
        "b@example.test".into(),
        scope,
    );
    b.mode = ConnectionMode::CursorApp;
    b.profile = Some(path);
    assert!(super::super::validate_connections(&[a.clone()]).is_ok());
    assert!(super::super::validate_connections(&[a, b]).is_err());
}
#[test]
fn jwt_expiry_subject_encoding_and_injection_are_validated_without_trusting_claims_as_auth() {
    assert!(Session::parse(Zeroizing::new(live_token())).is_ok());
    for bad in [
        token("user_fixture", Utc::now().timestamp() + 59),
        token("user_fixture", 0),
        token("bad\r\nsubject", Utc::now().timestamp() + 3600),
        "a.b.c".into(),
        format!("{}\r\nCookie: injected", live_token()),
        "x".repeat(17000),
    ] {
        assert!(Session::parse(Zeroizing::new(bad)).is_err());
    }
}
#[tokio::test]
async fn explicit_consent_precedes_local_session_access_and_network() {
    let h = harness("ok").await;
    assert!(h
        .service
        .connect_cursor_personal(&Settings::default(), false)
        .await
        .is_err());
    assert!(h.service.cursor_sign_in_marker(false).await.is_err());
    assert!(h.service.personal.fingerprint.lock().unwrap().is_none());
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
    assert!(!h.service.storage.dir.exists());
}
#[tokio::test]
async fn personal_connection_verifies_identity_around_usage_and_persists_only_metadata() {
    let h = harness("ok").await;
    let before = std::fs::read(&h.service.personal.path).unwrap();
    let (settings, state) = h
        .service
        .connect_cursor_personal(&Settings::default(), true)
        .await
        .unwrap();
    let c = &settings.connections[0];
    assert_eq!(c.mode, ConnectionMode::CursorApp);
    assert_eq!(c.subject, "user_fixture");
    assert_eq!(c.login, "personal@example.test");
    let report = state.report.unwrap();
    assert_eq!(report.source, ReportSource::CursorDashboard);
    assert_eq!(
        report.quotas.iter().map(|q| q.percent).collect::<Vec<_>>(),
        vec![Some(25.0), Some(10.0), Some(42.0)]
    );
    assert!(report
        .quotas
        .iter()
        .all(|q| q.resets_at.is_some() && q.value.is_none()));
    assert!(state.account.sources.is_empty());
    assert_eq!(
        *h.fixture.calls.lock().unwrap(),
        vec!["/api/auth/me", "/api/usage-summary", "/api/auth/me"]
    );
    let saved = std::fs::read_to_string(h.service.storage.dir.join("settings.json")).unwrap();
    assert!(!saved.contains(&live_token()));
    assert!(!saved.contains("accessToken"));
    assert_eq!(before, std::fs::read(&h.service.personal.path).unwrap());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
    assert!(h.service.fetch(c).await.is_ok());
    let next = h.service.disconnect(&settings, &c.id).unwrap();
    assert!(next.connections.is_empty());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
    assert!(h.service.personal.path.exists());
}
#[tokio::test]
async fn expired_signed_out_and_cross_account_sessions_never_make_a_usage_request() {
    let h = harness("ok").await;
    let (settings, previous) = h
        .service
        .connect_cursor_personal(&Settings::default(), true)
        .await
        .unwrap();
    let c = &settings.connections[0];
    h.fixture.calls.lock().unwrap().clear();
    for jwt in [
        token("user_other", Utc::now().timestamp() + 3600),
        token("user_fixture", Utc::now().timestamp() + 10),
    ] {
        drop(database(&h.service.personal.path, &jwt, false));
        let result = h.service.fetch(c).await;
        assert!(result.is_err());
        assert!(
            AccountState::apply(c.account(&settings), result, Some(&previous))
                .report
                .is_none()
        );
    }
    std::fs::remove_file(&h.service.personal.path).unwrap();
    assert_eq!(
        h.service.fetch(c).await.unwrap_err().kind,
        FaultKind::NotSignedIn
    );
    assert!(h.fixture.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn rejection_malformed_private_data_and_mid_read_identity_changes_never_save_connections() {
    for mode in [
        "401",
        "403",
        "429",
        "redirect",
        "giant",
        "malformed",
        "wrong_user",
        "remote_switch",
        "local_switch",
        "local_rotation",
        "bad_email",
        "negative",
        "bad_date",
        "unknown",
    ] {
        let h = harness(mode).await;
        let e = h
            .service
            .connect_cursor_personal(&Settings::default(), true)
            .await
            .err()
            .unwrap_or_else(|| panic!("Unexpected success: {mode}"));
        assert!(!e.to_string().contains("secret response"));
        assert!(!e.to_string().contains(&live_token()));
        assert!(!h.service.storage.dir.exists());
        assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
        if mode == "redirect" {
            assert_eq!(h.fixture.calls.lock().unwrap().len(), 1);
        }
        if mode == "429" {
            assert_eq!(
                e.downcast_ref::<Fault>().unwrap().retry_after_secs,
                Some(120)
            );
        }
    }
}
#[tokio::test]
async fn missing_individual_allowance_never_borrows_a_team_budget_or_invents_a_reset() {
    let h = harness("empty").await;
    let (_, state) = h
        .service
        .connect_cursor_personal(&Settings::default(), true)
        .await
        .unwrap();
    assert!(state.report.unwrap().quotas.is_empty());
}
#[tokio::test]
async fn personal_reconnect_is_scoped_and_does_not_overwrite_enterprise_or_unrelated_preferences() {
    let h = harness("ok").await;
    let enterprise = Connection::new(
        Provider::Cursor,
        "user_enterprise".into(),
        "member@example.test".into(),
        hash("synthetic-enterprise-key"),
    );
    let initial = Settings {
        theme: "dark".into(),
        connections: vec![enterprise.clone()],
        ..Default::default()
    };
    let (settings, _) = h
        .service
        .connect_cursor_personal(&initial, true)
        .await
        .unwrap();
    h.service.gates.lock().unwrap().clear();
    let (again, _) = h
        .service
        .connect_cursor_personal(&settings, true)
        .await
        .unwrap();
    assert_eq!(again.connections.len(), 2);
    assert!(again.connections.contains(&enterprise));
    assert_eq!(again.theme, "dark");
    assert_eq!(
        again
            .connections
            .iter()
            .map(|c| &c.id)
            .collect::<HashSet<_>>()
            .len(),
        2
    );
}
#[tokio::test]
async fn wrong_profile_metadata_and_failed_settings_writes_leave_cursor_untouched() {
    let mut h = harness("ok").await;
    let (settings, _) = h
        .service
        .connect_cursor_personal(&Settings::default(), true)
        .await
        .unwrap();
    let mut c = settings.connections[0].clone();
    c.profile = Some(
        h.dir
            .path()
            .join("other-profile/state.vscdb")
            .to_string_lossy()
            .into_owned(),
    );
    h.fixture.calls.lock().unwrap().clear();
    assert_eq!(
        h.service.fetch(&c).await.unwrap_err().kind,
        FaultKind::IdentityChanged
    );
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    h.service.gates.lock().unwrap().clear();
    let blocked = h.dir.path().join("blocked");
    std::fs::write(&blocked, b"file").unwrap();
    h.service.storage.dir = blocked;
    let before = std::fs::read(&h.service.personal.path).unwrap();
    assert!(h
        .service
        .connect_cursor_personal(&settings, true)
        .await
        .is_err());
    assert!(h
        .service
        .disconnect(&settings, &settings.connections[0].id)
        .is_err());
    assert_eq!(before, std::fs::read(&h.service.personal.path).unwrap());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn only_authentication_changes_advance_the_local_watcher_fingerprint() {
    let h = harness("ok").await;
    let (settings, _) = h
        .service
        .connect_cursor_personal(&Settings::default(), true)
        .await
        .unwrap();
    assert!(!h.service.cursor_auth_changed(&settings).await);
    let db = Database::open(&h.service.personal.path).unwrap();
    db.execute(
        "INSERT INTO ItemTable VALUES('unrelated-setting','changed')",
        [],
    )
    .unwrap();
    drop(db);
    *h.service.personal.checked.lock().unwrap() = None;
    assert!(!h.service.cursor_auth_changed(&settings).await);
    drop(database(
        &h.service.personal.path,
        &token("user_new", Utc::now().timestamp() + 3600),
        false,
    ));
    *h.service.personal.checked.lock().unwrap() = None;
    assert!(h.service.cursor_auth_changed(&settings).await);
    assert!(!h.service.cursor_auth_changed(&Settings::default()).await);
}
#[tokio::test]
async fn engine_restore_watches_real_account_switch_and_disconnects_without_touching_cursor() {
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
        initial,
        h.service.clone(),
    );
    let settings = engine.connect_cursor_personal(true).await.unwrap();
    assert_eq!(engine.state.borrow().accounts.len(), 1);
    engine.shutdown().await;
    let restored = crate::engine::Engine::start_with_service(
        h.service.storage.clone(),
        settings.clone(),
        h.service.clone(),
    );
    let mut rx = restored.state.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if rx
                .borrow()
                .accounts
                .iter()
                .any(|s| s.status == Status::Ready)
            {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    drop(database(
        &h.service.personal.path,
        &token("user_new", Utc::now().timestamp() + 3600),
        false,
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if rx
                .borrow()
                .accounts
                .iter()
                .any(|s| s.report.is_none() && s.status == Status::Faulted)
            {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let before = std::fs::read(&h.service.personal.path).unwrap();
    restored
        .disconnect(settings.connections[0].id.clone())
        .await
        .unwrap();
    assert!(restored.state.borrow().accounts.is_empty());
    restored.shutdown().await;
    assert_eq!(before, std::fs::read(&h.service.personal.path).unwrap());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn single_action_coordinator_opens_a_synthetic_signin_then_verifies_and_saves_through_the_engine(
) {
    use crate::cursor_login::{self, Flow, Host, Options, Phase, Status};
    struct Login {
        engine: crate::engine::Engine,
        path: PathBuf,
        opened: AtomicUsize,
        events: Mutex<Vec<Status>>,
    }
    #[async_trait::async_trait]
    impl Host for Login {
        async fn probe(&self) -> Result<String, Fault> {
            self.engine.cursor_sign_in_marker(true).await
        }
        async fn connect(&self) -> Result<(), Fault> {
            self.engine
                .connect_cursor_personal(true)
                .await
                .map(|_| ())
                .map_err(|e| {
                    e.downcast_ref::<Fault>()
                        .cloned()
                        .unwrap_or_else(|| Fault::new(FaultKind::Unknown, e.to_string()))
                })
        }
        async fn open_cursor(&self) -> Result<(), Fault> {
            self.opened.fetch_add(1, Ordering::SeqCst);
            drop(database(&self.path, &live_token(), false));
            Ok(())
        }
        fn publish(&self, status: Status) {
            self.events.lock().unwrap().push(status);
        }
    }
    let h = harness("ok").await;
    std::fs::remove_file(&h.service.personal.path).unwrap();
    let engine = crate::engine::Engine::start_with_service(
        h.service.storage.clone(),
        Settings {
            codex_enabled: false,
            claude_enabled: false,
            auto_discover: false,
            live_tracking: false,
            ..Default::default()
        },
        h.service.clone(),
    );
    let host = Arc::new(Login {
        engine: engine.clone(),
        path: h.service.personal.path.clone(),
        opened: AtomicUsize::new(0),
        events: Mutex::new(vec![]),
    });
    let flow = Arc::new(Flow::default());
    let run = flow.begin().unwrap();
    cursor_login::run(
        flow.clone(),
        run,
        host.clone(),
        Options {
            waiting_timeout: Duration::from_secs(5),
            poll_interval: Duration::from_millis(2),
            verification_spacing: Duration::from_millis(15),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(flow.status().phase, Phase::Connected);
    assert_eq!(host.opened.load(Ordering::SeqCst), 1);
    assert_eq!(engine.settings.borrow().connections.len(), 1);
    assert_eq!(
        engine.state.borrow().accounts[0]
            .report
            .as_ref()
            .unwrap()
            .source,
        ReportSource::CursorDashboard
    );
    assert!(!serde_json::to_string(&*host.events.lock().unwrap())
        .unwrap()
        .contains("accessToken"));
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
    engine.shutdown().await;
}
#[test]
fn provider_verified_subject_is_usable_without_inventing_an_email() {
    let user: User = serde_json::from_value(json!({"sub":"user_fixture","email":null})).unwrap();
    assert_eq!(user.identity().unwrap(), "user_fixture");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cursor/User/globalStorage/state.vscdb");
    let mut connection = Connection::new(
        Provider::Cursor,
        "user_fixture".into(),
        "user_fixture".into(),
        format!("app:{}", hash(&crate::storage::path_key(&path))),
    );
    connection.mode = ConnectionMode::CursorApp;
    connection.profile = Some(path.to_string_lossy().into_owned());
    assert!(super::super::validate_connections(std::slice::from_ref(&connection)).is_ok());
    assert!(connection.account(&Settings::default()).email.is_none());
}
#[test]
fn canonical_oidc_subject_survives_unrelated_numeric_database_id() {
    let user: User = serde_json::from_value(
        json!({"sub":"auth0|user_fixture","id":417,"email":"personal@example.test"}),
    )
    .unwrap();
    assert_eq!(user.identity().unwrap(), "user_fixture");
    let no_subject: User =
        serde_json::from_value(json!({"id":417,"email":"personal@example.test"})).unwrap();
    assert!(no_subject.identity().is_err());
    let conflicting: User =
        serde_json::from_value(json!({"sub":"user_fixture","id":"user_other"})).unwrap();
    assert!(conflicting.identity().is_err());
}
#[tokio::test]
async fn diagnosis_is_explicit_and_does_not_save_connections_or_use_the_vault() {
    let h = harness("numeric_id").await;
    assert!(h
        .service
        .inspect_existing_source(Provider::Cursor, false)
        .await
        .is_err());
    assert!(h
        .service
        .inspect_existing_source(Provider::Deepseek, true)
        .await
        .is_err());
    assert!(h.fixture.calls.lock().unwrap().is_empty());
    let result = h
        .service
        .inspect_existing_source(Provider::Cursor, true)
        .await
        .unwrap();
    assert_eq!(result.source, ReportSource::CursorDashboard);
    assert_eq!(result.percentage_count, 3);
    assert!(!h.service.storage.dir.exists());
    assert_eq!(h.vault.0.load(Ordering::SeqCst), 0);
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("personal@example.test"));
    assert!(!serialized.contains("signature"));
    assert!(!serialized.contains("user_fixture"));
}
#[test]
fn summary_uses_reported_values_only_and_keeps_unknown_and_zero_distinct() {
    for (body, expected) in [
        (
            json!({"membershipType":"pro","individualUsage":{"plan":{"totalPercentUsed":0}}}),
            vec![Some(0.0)],
        ),
        (json!({"membershipType":"hobby","isUnlimited":true}), vec![]),
        (
            json!({"membershipType":"pro","individualUsage":{"plan":{"autoPercentUsed":10,"apiPercentUsed":60}}}),
            vec![Some(10.0), Some(60.0)],
        ),
    ] {
        let r = report(serde_json::from_value(body).unwrap()).unwrap();
        assert_eq!(
            r.quotas.iter().map(|q| q.percent).collect::<Vec<_>>(),
            expected
        );
        assert!(r.quotas.iter().all(|q| q.resets_at.is_none()));
    }
}
#[test]
fn an_expired_cursor_token_says_when_it_expired_instead_of_a_generic_error() {
    let jwt = token("user_fixture", 1_700_000_000);
    let fault = Session::parse(Zeroizing::new(jwt)).err().unwrap();
    assert_eq!(fault.kind, FaultKind::NeedsRelogin);
    assert!(
        fault.message.contains("expired on Nov 14, 2023"),
        "{}",
        fault.message
    );
    let garbled = Session::parse(Zeroizing::new("not-a-jwt-at-all-xxxxxxxx".into()))
        .err()
        .unwrap();
    assert!(garbled.message.contains("expired or invalid"));
}
#[test]
fn an_encrypted_cursor_sign_in_is_reported_as_unsupported_not_expired() {
    let fault = Session::parse(Zeroizing::new(
        "djEw:aXZpdg:Y2lwaGVydGV4dGNpcGhlcnRleHQ=".into(),
    ))
    .err()
    .unwrap();
    assert_eq!(fault.kind, FaultKind::PermissionDenied);
    assert!(fault.message.contains("encrypts its saved sign-in"));
    assert!(!fault.message.contains("expired"));
}
