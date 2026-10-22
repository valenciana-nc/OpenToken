use super::*;
use serde_json::json;
#[test]
fn devin_keeps_missing_unknown_and_hidden_daily_unmapped() {
    let value = json!({"userStatus":{"planStatus":{"planInfo":{"isDevin":true,"planName":"Max","hideDailyQuota":true},"dailyQuotaRemainingPercent":100,"weeklyQuotaResetAtUnix":"1900000000","overageBalanceMicros":"1234567"}}});
    let report = parse_devin(&value).unwrap();
    assert!(report.quotas.iter().all(|q| q.percent.is_none()));
    assert_eq!(report.quotas[0].value, Some(100.));
    assert_eq!(report.quotas[1].value, Some(1.234567));
    let mut v = value.clone();
    v["userStatus"]["planStatus"]["weeklyQuotaRemainingPercent"] = json!(0);
    let r = parse_devin(&v).unwrap();
    assert_eq!(r.quotas[0].percent, Some(100.));
    assert_eq!(r.quotas[0].resets_at.unwrap().timestamp(), 1900000000);
    v["userStatus"]["planStatus"] =
        json!({"planInfo":{"isDevin":true},"weeklyQuotaResetAtUnix":"1900000000"});
    assert!(parse_devin(&v).is_err());
}
#[test]
fn devin_explicit_daily_weekly_balances_and_entitlement() {
    let mut v = json!({"userStatus":{"planStatus":{"planInfo":{"isDevin":true,"hideDailyQuota":false},"dailyQuotaRemainingPercent":75,"weeklyQuotaRemainingPercent":"10","overageBalanceMicros":0}}});
    let r = parse_devin(&v).unwrap();
    assert_eq!(r.quotas[0].percent, Some(25.));
    assert_eq!(r.quotas[1].percent, Some(90.));
    assert_eq!(r.quotas[2].value, Some(0.));
    v["userStatus"]["planStatus"]["planInfo"]["isDevin"] = json!(false);
    assert!(parse_devin(&v).is_err());
}
#[test]
fn antigravity_namespace_diagnosis_refuses_unapproved_and_isolated_sources() {
    let root = tempfile::tempdir().unwrap();
    let service = Service::new(
        Storage {
            dir: root.path().join("data"),
            user_home: root.path().join("isolated-home"),
        },
        Arc::new(UnavailableStore),
    );
    assert!(service.inspect_antigravity_cli_namespace(false).is_err());
    assert!(service.inspect_antigravity_cli_namespace(true).is_err());
    assert!(!root.path().join("data").exists());
}
#[test]
fn antigravity_summary_never_fills_missing_or_merges_unknown_buckets() {
    let mut v = json!({"response":{"groups":[{"buckets":[{"bucketId":"gemini-5h","remainingFraction":0.25},{"bucketId":"3p-weekly"},{"bucketId":"future-weekly","remainingFraction":1},{"bucketId":"gemini-weekly","remainingFraction":0,"disabled":true}]}]}});
    let r = antigravity::parse_summary(&v).unwrap();
    assert_eq!(r.quotas.len(), 1);
    assert_eq!(r.quotas[0].percent, Some(75.));
    v["response"]["groups"][0]["buckets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"bucketId":"gemini-5h","remainingFraction":0.8}));
    assert!(antigravity::parse_summary(&v).is_err());
    assert!(antigravity::parse_summary(&json!({"groups":[]})).is_err());
}
#[test]
fn muse_preserves_observation_time_overquota_and_expires_without_extrapolation() {
    let now = Utc::now().timestamp_millis();
    let mut v = json!({"observedAtMs":now-60000,"tier":"High","window":{"usedPercent":125,"resetsAtMs":now+60000,"windowDurationMins":300},"weekly":{"usedPercent":42,"resetsAtMs":now+86400000}});
    let r = muse::parse(&v).unwrap();
    assert_eq!(r.observed_at.timestamp_millis(), now - 60000);
    assert_eq!(r.quotas[0].percent, Some(125.));
    v["window"]["resetsAtMs"] = json!(now - 1);
    let r = muse::parse(&v).unwrap();
    assert_eq!(r.quotas.len(), 1);
    assert_eq!(r.quotas[0].percent, Some(42.));
    v["weekly"]["resetsAtMs"] = json!(now - 1);
    assert!(muse::parse(&v).is_err());
    assert!(muse::parse(&Value::Null).is_err());
}
#[tokio::test]
async fn devin_http_lifecycle_needs_no_vault_and_rejects_rotation_or_untrusted_host() {
    use axum::{extract::State, routing::post, Json, Router};
    use std::sync::Mutex;
    let root = tempfile::Builder::new()
        .prefix("local-provider-fixture-")
        .tempdir()
        .unwrap();
    let storage = Storage {
        dir: root.path().join("data"),
        user_home: root.path().join("home"),
    };
    std::fs::create_dir_all(&storage.dir).unwrap();
    let path = root.path().join("credentials.toml");
    let original="windsurf_api_key = 'devin-session-token$fixture-only'\napi_server_url = 'https://server.codeium.com'\n";
    std::fs::write(&path, original).unwrap();
    let rotate = Arc::new(Mutex::new(false));
    let state = (path.clone(), rotate.clone());
    let app=Router::new().route("/exa.seat_management_pb.SeatManagementService/GetUserStatus",post(|State((path,rotate)):State<(PathBuf,Arc<Mutex<bool>>)>,Json(body):Json<Value>|async move{
        assert_eq!(body["metadata"]["ideName"],"devin");assert_eq!(body["metadata"]["extensionName"],"opentoken");assert_eq!(body["metadata"]["ideVersion"],"unknown");assert!(body["metadata"]["apiKey"].as_str().unwrap().contains("fixture-only"));
        if *rotate.lock().unwrap(){std::fs::write(path,"windsurf_api_key = 'changed-fixture-token-only'\n").unwrap();}
        Json(json!({"userStatus":{"userId":"fixture-user","planStatus":{"planInfo":{"isDevin":true,"hideDailyQuota":false,"planName":"Pro"},"dailyQuotaRemainingPercent":80,"weeklyQuotaRemainingPercent":50}}}))
    })).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut service = Service::new(storage.clone(), Arc::new(UnavailableStore));
    Arc::get_mut(&mut service.local).unwrap().devin_base = base;
    let settings = Settings::default();
    let (next, state) = service
        .connect_local_at(&settings, Provider::Devin, &path)
        .await
        .unwrap();
    assert_eq!(state.status, Status::Ready);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let saved = serde_json::to_string(&next).unwrap();
    assert!(!saved.contains("session-token$"));
    assert!(!saved.contains("fixture-only"));
    assert!(service
        .fetch_local_provider(&next.connections[0])
        .await
        .is_ok());
    let disconnected = service.disconnect(&next, &next.connections[0].id).unwrap();
    assert!(disconnected.connections.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    *rotate.lock().unwrap() = true;
    assert_eq!(
        service.read_devin(&path).await.err().unwrap().kind,
        FaultKind::IdentityChanged
    );
    std::fs::write(&path,"windsurf_api_key = 'devin-session-token$fixture-only'\napi_server_url = 'https://elsewhere.invalid'\n").unwrap();
    assert_eq!(
        service.read_devin(&path).await.err().unwrap().kind,
        FaultKind::PermissionDenied
    );
    task.abort();
}
#[cfg(windows)]
#[test]
fn windows_flags_are_quoted_and_duplicate_safe() {
    let a=windows::arguments(r#""C:\Program Files\Antigravity\language_server_windows_x64.exe" --app_data_dir "C:\Users\Fixture Home\Antigravity" --csrf_token=fixture-only"#).unwrap();
    assert_eq!(a.len(), 4);
    assert_eq!(
        a[0].as_str(),
        r"C:\Program Files\Antigravity\language_server_windows_x64.exe"
    );
    assert_eq!(a[2].as_str(), r"C:\Users\Fixture Home\Antigravity");
    assert!(windows::arguments("exe \"unfinished").is_err());
}
