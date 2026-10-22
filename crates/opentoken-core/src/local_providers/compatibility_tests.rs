use super::antigravity_cli::SessionStore;
use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, Uri},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
fn store() -> (tempfile::TempDir, Storage) {
    let root = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: root.path().join("data"),
        user_home: root.path().join("home"),
    };
    std::fs::create_dir_all(&storage.dir).unwrap();
    (root, storage)
}
#[tokio::test]
async fn devin_desktop_wal_read_identity_rotation_and_monitoring_only_disconnect() {
    let (_root, storage) = store();
    let path = storage
        .user_home
        .join("AppData/Roaming/Devin/User/globalStorage/state.vscdb");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "journal_mode", "wal").unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT);")
        .unwrap();
    let set = |token: &str| {
        db.execute("INSERT OR REPLACE INTO ItemTable VALUES('windsurfAuthStatus',?1)",[json!({"apiKey":token,"userStatusJson":"untrusted cached metadata is never account proof"}).to_string()]).unwrap();
    };
    set("devin-session-token$fixture-desktop-only");
    let initial = std::fs::read(&path).unwrap();
    let wal = std::fs::read(crate::source_db::sidecar(&path, "-wal")).unwrap();
    let app=Router::new().route("/exa.seat_management_pb.SeatManagementService/GetUserStatus",post(|Json(body):Json<Value>|async move{
        assert_eq!(body["metadata"]["apiKey"],"devin-session-token$fixture-desktop-only");assert_eq!(body["metadata"]["extensionName"],"opentoken");
        Json(json!({"userStatus":{"userId":"fixture-user","planStatus":{"planInfo":{"isDevin":true},"dailyQuotaRemainingPercent":80,"weeklyQuotaRemainingPercent":60}}}))
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    Arc::get_mut(&mut service.local).unwrap().devin_base = base;
    assert_eq!(profile(&storage, Provider::Devin), path);
    let (next, state) = service
        .connect_local_provider(&Settings::default(), Provider::Devin, true)
        .await
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert_eq!(state.report.unwrap().source, ReportSource::DevinApp);
    assert_eq!(next.connections[0].mode, ConnectionMode::DevinApp);
    assert_eq!(std::fs::read(&path).unwrap(), initial);
    assert_eq!(
        std::fs::read(crate::source_db::sidecar(&path, "-wal")).unwrap(),
        wal
    );
    let saved = serde_json::to_string(&next).unwrap();
    assert!(!saved.contains("fixture-desktop-only"));
    assert!(!saved.contains("untrusted cached"));
    assert!(service.local_scope_changes(&next).await.is_empty());
    db.execute(
        "INSERT INTO ItemTable VALUES('codeium.windsurf',?1)",
        [json!({"apiServerUrl":"https://private-tenant.invalid"}).to_string()],
    )
    .unwrap();
    assert_eq!(
        service
            .fetch(&next.connections[0])
            .await
            .err()
            .unwrap()
            .kind,
        FaultKind::PermissionDenied
    );
    db.execute("DELETE FROM ItemTable WHERE key='codeium.windsurf'", [])
        .unwrap();
    set("devin-session-token$fixture-switched-only");
    assert!(service
        .local_scope_changes(&next)
        .await
        .contains(&next.connections[0].id));
    let stopped = service.disconnect(&next, &next.connections[0].id).unwrap();
    assert!(stopped.connections.is_empty());
    assert!(path.exists());
    server.abort();
}
#[test]
fn generic_database_rejects_unpaired_sidecars_and_does_not_create_them() {
    let (_root, storage) = store();
    let p = storage.user_home.join("db/state.vscdb");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&p).unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value);")
        .unwrap();
    db.execute(
        "INSERT INTO ItemTable VALUES('windsurfAuthStatus',?1)",
        [json!({"apiKey":"fixture-only-token-for-tests"}).to_string()],
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&p).unwrap();
    assert!(crate::source_db::read(&p, "windsurfAuthStatus", 32768).is_ok());
    assert_eq!(before, std::fs::read(&p).unwrap());
    assert!(!crate::source_db::sidecar(&p, "-wal").exists());
    std::fs::write(crate::source_db::sidecar(&p, "-wal"), b"unpaired fixture").unwrap();
    assert!(crate::source_db::read(&p, "windsurfAuthStatus", 32768).is_err());
    assert!(!crate::source_db::sidecar(&p, "-shm").exists());
}
struct FixtureCredential(Mutex<Vec<u8>>);
impl antigravity_cli::SessionStore for FixtureCredential {
    fn read(&self) -> Result<Zeroizing<Vec<u8>>, Fault> {
        Ok(Zeroizing::new(self.0.lock().unwrap().clone()))
    }
}
fn google_fixture_token(token: &str) -> Vec<u8> {
    json!({"token":{"access_token":token,"expiry":(Utc::now()+chrono::Duration::hours(1)).to_rfc3339(),"refresh_token":"fixture-refresh-never-used"}}).to_string().into_bytes()
}
#[tokio::test]
async fn antigravity_cli_fixed_scope_checks_identity_and_does_not_save_or_renew() {
    let (_root, storage) = store();
    let p = storage.user_home.join("AppData/Local/agy/bin/agy.exe");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"fixture executable metadata only; never launched").unwrap();
    let credential = Arc::new(FixtureCredential(Mutex::new(google_fixture_token(
        "fixture-only-google-access-token",
    ))));
    let rotate = Arc::new(AtomicBool::new(false));
    let state = (credential.clone(), rotate.clone());
    let app=Router::new().route("/userinfo",get(|headers:HeaderMap|async move{assert_eq!(headers["authorization"],"Bearer fixture-only-google-access-token");Json(json!({"sub":"fixture-user","email":"fixture@example.test"}))})).route("/daily-quota",post(|State((credential,rotate)):State<(Arc<FixtureCredential>,Arc<AtomicBool>)>,uri:Uri,Json(body):Json<Value>|async move{
        assert_eq!(uri.path(),"/daily-quota");assert_eq!(body,json!({}));if rotate.load(Ordering::SeqCst){*credential.0.lock().unwrap()=google_fixture_token("fixture-only-google-other-token");}
        Json(json!({"groups":[{"buckets":[{"bucketId":"gemini-5h","remainingFraction":0.8}]}]}))
    })).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    let local = Arc::get_mut(&mut service.local).unwrap();
    local.credential = credential;
    local.google_identity_url = format!("{base}/userinfo");
    local.google_quota_url = format!("{base}/daily-quota");
    let reading = service.read_antigravity_cli(&p).await.unwrap();
    assert_eq!(reading.report.source, ReportSource::AntigravityCli);
    assert_eq!(reading.report.quotas.len(), 1);
    assert!((reading.report.quotas[0].percent.unwrap() - 20.).abs() < 1e-9);
    assert!(!storage.dir.join("settings.json").exists());
    rotate.store(true, Ordering::SeqCst);
    assert_eq!(
        service.read_antigravity_cli(&p).await.err().unwrap().kind,
        FaultKind::IdentityChanged
    );
    server.abort();
}
#[tokio::test]
async fn antigravity_ide_persisted_topic_uses_verified_identity_without_database_copy_or_renewal() {
    let (_root, storage) = store();
    let path = antigravity_stored::profile(&storage);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT);")
        .unwrap();
    db.execute(
        "INSERT INTO ItemTable VALUES('antigravityUnifiedStateSync.oauthToken',?1)",
        [antigravity_stored::fixture_topic(
            "fixture-only-google-access-token",
            false,
        )],
    )
    .unwrap();
    let original = std::fs::read(&path).unwrap();
    let app=Router::new().route("/userinfo",get(|headers:HeaderMap|async move{assert_eq!(headers["authorization"],"Bearer fixture-only-google-access-token");Json(json!({"sub":"fixture-user","email":"fixture@example.test"}))})).route("/quota",post(||async{Json(json!({"groups":[{"buckets":[{"bucketId":"gemini-5h","remainingFraction":0.7}]}]}))})).route("/context",post(||async{Json(json!({"currentTier":{"id":"standard-tier"},"paidTier":{"id":"g1-pro-tier","usesGcpTos":false}}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    let local = Arc::get_mut(&mut service.local).unwrap();
    local.google_identity_url = format!("{base}/userinfo");
    local.google_quota_url = format!("{base}/quota");
    local.google_context_url = format!("{base}/context");
    let reading = service.read_antigravity_stored(&path).await.unwrap();
    assert_eq!(reading.report.source, ReportSource::AntigravityStored);
    let mut prior_report = reading.report.clone();
    prior_report.source = ReportSource::AntigravityLocal;
    let (previous, _) = service
        .persist_local(
            &Settings::default(),
            Provider::Antigravity,
            path.ancestors().nth(3).unwrap(),
            Reading {
                subject: "fixture@example.test".into(),
                login: "fixture@example.test".into(),
                report: prior_report,
            },
            ConnectionMode::AntigravityApp,
        )
        .unwrap();
    let (next, state) = service
        .persist_local(
            &previous,
            Provider::Antigravity,
            &path,
            reading,
            ConnectionMode::AntigravityStored,
        )
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert_eq!(
        next.connections.len(),
        1,
        "a verified source transition within one IDE profile must not duplicate monitoring"
    );
    assert_eq!(next.connections[0].mode, ConnectionMode::AntigravityStored);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!crate::source_db::sidecar(&path, "-wal").exists());
    assert!(!serde_json::to_string(&next)
        .unwrap()
        .contains("fixture-only-google-access-token"));
    assert!(service.local_scope_changes(&next).await.is_empty());
    db.execute(
        "UPDATE ItemTable SET value=?1 WHERE key='antigravityUnifiedStateSync.oauthToken'",
        [antigravity_stored::fixture_topic(
            "fixture-only-other-google-token",
            false,
        )],
    )
    .unwrap();
    assert!(service
        .local_scope_changes(&next)
        .await
        .contains(&next.connections[0].id));
    assert!(service
        .disconnect(&next, &next.connections[0].id)
        .unwrap()
        .connections
        .is_empty());
    assert!(path.exists());
    task.abort();
}
#[test]
fn disconnected_session_permissions_do_not_return_from_recovery_backups() {
    let (_root, storage) = store();
    let p = storage.user_home.join(".grok");
    let mut c = Connection::new(
        Provider::Grok,
        "fixture-user".into(),
        "fixture@example.test".into(),
        format!("grok:{}", hash(&crate::storage::path_key(&p))),
    );
    c.mode = ConnectionMode::GrokCli;
    c.profile = Some(p.to_string_lossy().into_owned());
    let settings = Settings {
        connections: vec![c.clone()],
        ..Settings::default()
    };
    storage.save_settings(&settings).unwrap();
    let service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    let next = service.disconnect(&settings, &c.id).unwrap();
    assert!(next.connections.is_empty());
    std::fs::write(
        storage.dir.join("settings.json"),
        b"deliberate corruption fixture",
    )
    .unwrap();
    assert!(storage.load_settings().unwrap().connections.is_empty());
}
#[test]
fn isolated_source_homes_never_read_the_real_vendor_vault() {
    let result = antigravity_cli::NativeStore { allowed: false }.read();
    assert_eq!(result.err().unwrap().kind, FaultKind::PermissionDenied);
}
