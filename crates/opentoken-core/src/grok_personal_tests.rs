use super::*;
use serde_json::json;
fn write_auth(home: &std::path::Path, token: &str, expiry: i64, team: bool) {
    std::fs::create_dir_all(home).unwrap();
    std::fs::write(home.join("auth.json"),serde_json::to_vec(&json!({"https://auth.x.ai::official-client":{"key":token,"refresh_token":"provider-owned-never-used","expires_at":expiry,"user_id":"user-fixture","principal_type":if team{"team"}else{"user"}}})).unwrap()).unwrap();
}
#[test]
fn missing_usage_is_not_an_invented_zero() {
    assert!(parse(&json!({"config":{}})).is_err());
    let period =
        parse(&json!({"config":{"currentPeriod":{"end":"2030-01-01T00:00:00Z"}}})).unwrap();
    assert!(period.quotas.is_empty());
    let zero = parse(&json!({"config":{"creditUsagePercent":0}})).unwrap();
    assert_eq!(zero.quotas[0].percent, Some(0.));
}
#[tokio::test]
async fn provider_acceptance_not_cached_expiry_decides_usage_access() {
    for expiry in [Some(json!(1)), None] {
        let f = fixture("ok").await;
        let mut data: Value =
            serde_json::from_slice(&std::fs::read(f.home.join("auth.json")).unwrap()).unwrap();
        if let Some(expiry) = expiry {
            data["https://auth.x.ai::official-client"]["expires_at"] = expiry;
        } else {
            data["https://auth.x.ai::official-client"]
                .as_object_mut()
                .unwrap()
                .remove("expires_at");
        }
        std::fs::write(f.home.join("auth.json"), serde_json::to_vec(&data).unwrap()).unwrap();
        let (_, state) = f
            .service
            .connect_grok_personal(&Settings::default(), true)
            .await
            .unwrap();
        assert_eq!(state.status, Status::Ready);
    }
    let f = fixture("rejected").await;
    let result = f
        .service
        .connect_grok_personal(&Settings::default(), true)
        .await
        .err()
        .unwrap();
    assert_eq!(
        result.downcast_ref::<Fault>().unwrap().kind,
        FaultKind::NeedsRelogin
    );
    assert!(!f.service.storage.dir.join("settings.json").exists());
}
#[test]
fn expiry_hints_understand_seconds_milliseconds_and_unknown_without_authentication_claims() {
    assert_eq!(expiry_hint(Some(&json!(1900000000))), Some(1900000000));
    assert_eq!(
        expiry_hint(Some(&json!(1900000000000i64))),
        Some(1900000000)
    );
    assert_eq!(expiry_hint(Some(&json!("1900000000000"))), Some(1900000000));
    assert_eq!(expiry_hint(None), None);
    assert_eq!(expiry_hint(Some(&json!("unsupported"))), None);
}
#[test]
fn on_demand_budget_and_balance_are_not_included_subscription_allowance() {
    let r=parse(&json!({"config":{"creditUsagePercent":20,"onDemandCap":{"val":10000},"onDemandUsed":{"val":1250},"prepaidBalance":{"val":1234}}})).unwrap();
    assert_eq!(r.quotas.len(), 3);
    assert_eq!(r.quotas[1].label, "On-demand budget");
    assert_eq!(r.quotas[1].percent, Some(12.5));
    assert_eq!(r.quotas[2].value, Some(12.34));
    assert!(r.quotas[2].percent.is_none());
    assert_eq!(r.source, ReportSource::GrokCli);
}
#[test]
fn credentials_are_read_only_and_team_or_ambiguous_sessions_fail() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    write_auth(
        home,
        "synthetic-grok-token-first",
        Utc::now().timestamp() + 3600,
        false,
    );
    let before = std::fs::read(home.join("auth.json")).unwrap();
    assert_eq!(
        read(home).unwrap().token.as_str(),
        "synthetic-grok-token-first"
    );
    assert_eq!(std::fs::read(home.join("auth.json")).unwrap(), before);
    write_auth(home, "synthetic-grok-token-first", 1, false);
    assert_eq!(read(home).unwrap().expires, Some(1)); // provider, not cache metadata, decides acceptance
    write_auth(
        home,
        "synthetic-grok-token-first",
        Utc::now().timestamp() + 3600,
        true,
    );
    assert!(read(home).is_err());
    write_auth(
        home,
        "synthetic-grok-token-first",
        Utc::now().timestamp() + 3600,
        false,
    );
    let mut v: Value =
        serde_json::from_slice(&std::fs::read(home.join("auth.json")).unwrap()).unwrap();
    v["https://auth.x.ai::another"] = v["https://auth.x.ai::official-client"].clone();
    std::fs::write(home.join("auth.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(read(home).is_err());
}
struct Fixture {
    service: Service,
    home: std::path::PathBuf,
    _dir: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
    requests: Arc<std::sync::Mutex<Vec<String>>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn fixture(mode: &'static str) -> Fixture {
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    };
    #[derive(Clone)]
    struct Shared {
        home: std::path::PathBuf,
        mode: &'static str,
        reads: Arc<std::sync::atomic::AtomicUsize>,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
    }
    async fn endpoint(
        State(s): State<Shared>,
        headers: HeaderMap,
        uri: axum::http::Uri,
    ) -> Response {
        let count = {
            let mut requests = s.requests.lock().unwrap();
            requests.push(uri.path().to_string());
            requests.iter().filter(|p| p.as_str() == uri.path()).count()
        };
        assert_eq!(
            headers["authorization"],
            "Bearer synthetic-grok-token-first"
        );
        if s.mode == "transient" && uri.path() == "/v1/billing" && count == 1 {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        if s.mode == "forbidden" {
            return StatusCode::FORBIDDEN.into_response();
        }
        if s.mode == "rejected" {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        if uri.path() == "/oauth2/userinfo" {
            let n = s.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Json(json!({"sub":if s.mode=="switch"&&n>=2{"other-user"}else{"user-fixture"},"email":"grok@example.test"})).into_response();
        }
        assert_eq!(headers["x-xai-token-auth"], "xai-grok-cli");
        if s.mode == "rotate" {
            write_auth(
                &s.home,
                "synthetic-grok-token-changed",
                Utc::now().timestamp() + 3600,
                false,
            );
        }
        Json(if s.mode=="unknown"{json!({"config":{}})}else{json!({"config":{"creditUsagePercent":25,"subscriptionTier":"SuperGrok","currentPeriod":{"end":"2030-01-01T00:00:00Z"}}})}).into_response()
    }
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("grok");
    write_auth(
        &home,
        "synthetic-grok-token-first",
        Utc::now().timestamp() + 3600,
        false,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let shared = Shared {
        home: home.clone(),
        mode,
        reads: Default::default(),
        requests: requests.clone(),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/{*path}", get(endpoint))
                .with_state(shared),
        )
        .await
        .unwrap()
    });
    let mut service = Service::new(
        Storage {
            dir: dir.path().join("app"),
            user_home: dir.path().into(),
        },
        Arc::new(UnavailableStore),
    );
    service.grok_identity_base = base.clone();
    service.grok_proxy_base = base;
    service.grok_test_home = Some(home.clone());
    Fixture {
        service,
        home,
        _dir: dir,
        server,
        requests,
    }
}
#[tokio::test]
async fn actual_http_connect_refresh_and_disconnect_do_not_copy_or_modify_grok_credentials() {
    let h = fixture("ok").await;
    let before = std::fs::read(h.home.join("auth.json")).unwrap();
    assert!(h.service.grok_sign_in_marker(false).await.is_err());
    assert!(h
        .service
        .connect_grok_personal(&Settings::default(), false)
        .await
        .is_err());
    let (settings, state) = h
        .service
        .connect_grok_personal(
            &Settings {
                launch_at_startup: true,
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap();
    assert_eq!(state.account.email.as_deref(), Some("grok@example.test"));
    assert_eq!(state.report.unwrap().quotas[0].percent, Some(25.));
    assert!(settings.launch_at_startup);
    assert_eq!(settings.connections[0].mode, ConnectionMode::GrokCli);
    assert_eq!(
        h.service
            .fetch(&settings.connections[0])
            .await
            .unwrap()
            .source,
        ReportSource::GrokCli
    );
    assert!(!serde_json::to_string(&settings)
        .unwrap()
        .contains("synthetic-grok-token-first"));
    let next = h
        .service
        .disconnect(&settings, &settings.connections[0].id)
        .unwrap();
    assert!(next.connections.is_empty());
    assert_eq!(std::fs::read(h.home.join("auth.json")).unwrap(), before);
}
#[tokio::test]
async fn grok_temporary_quota_failure_retries_but_rejected_auth_never_does() {
    let f = fixture("transient").await;
    let before = std::fs::read(f.home.join("auth.json")).unwrap();
    let (settings, state) = f
        .service
        .connect_grok_personal(&Settings::default(), true)
        .await
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert_eq!(
        f.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == "/v1/billing")
            .count(),
        2
    );
    assert_eq!(settings.connections.len(), 1);
    assert_eq!(std::fs::read(f.home.join("auth.json")).unwrap(), before);
    let f = fixture("rejected").await;
    assert!(f
        .service
        .connect_grok_personal(&Settings::default(), true)
        .await
        .is_err());
    assert_eq!(f.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn grok_source_changes_are_local_only_ignore_partial_writes_and_expiry_hints() {
    let f = fixture("ok").await;
    let (settings, state) = f
        .service
        .connect_grok_personal(&Settings::default(), true)
        .await
        .unwrap();
    let calls = f.requests.lock().unwrap().len();
    assert!(f.service.grok_scope_changes(&settings).await.is_empty());
    write_auth(&f.home, "synthetic-grok-token-first", 1, false);
    assert!(f.service.grok_scope_changes(&settings).await.is_empty());
    std::fs::write(f.home.join("auth.json"), "{partial").unwrap();
    assert!(f.service.grok_scope_changes(&settings).await.is_empty());
    write_auth(&f.home, "synthetic-grok-token-new-owner", 1, false);
    assert!(f
        .service
        .grok_scope_changes(&settings)
        .await
        .contains(&state.account.id));
    assert!(f.service.grok_scope_changes(&settings).await.is_empty());
    assert_eq!(f.requests.lock().unwrap().len(), calls);
    assert_eq!(settings.connections.len(), 1);
}
#[tokio::test]
async fn identity_changes_rotated_credentials_auth_errors_and_unknown_quota_never_persist() {
    for mode in ["switch", "rotate", "forbidden", "unknown"] {
        let h = fixture(mode).await;
        assert!(
            h.service
                .connect_grok_personal(&Settings::default(), true)
                .await
                .is_err(),
            "{mode}"
        );
        assert!(!h.service.storage.dir.join("settings.json").exists());
    }
}
