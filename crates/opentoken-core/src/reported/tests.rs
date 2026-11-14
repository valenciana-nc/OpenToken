use super::*;
use serde_json::json;
#[test]
fn deepseek_balances_keep_currencies_separate_without_quota_or_email() {
    let r=parse(Provider::Deepseek,&json!({"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"12.50","granted_balance":"2.50","topped_up_balance":"10"},{"currency":"CNY","total_balance":"2","granted_balance":"0","topped_up_balance":"2"}]})).unwrap();
    assert_eq!(r.quotas.len(), 6);
    assert!(r.quotas.iter().all(|q| q.percent.is_none()));
    assert_eq!(r.quotas[0].value, Some(12.5));
}
#[test]
fn balance_missing_negative_duplicate_and_unknown_are_rejected() {
    for value in [
        json!({}),
        json!({"is_available":true,"balance_infos":[]}),
        json!({"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"-1","granted_balance":"0","topped_up_balance":"0"}]}),
    ] {
        assert!(parse(Provider::Deepseek, &value).is_err());
    }
}
#[test]
fn zero_balance_is_measured_and_not_a_percent() {
    let r=parse(Provider::Deepseek,&json!({"is_available":false,"balance_infos":[{"currency":"USD","total_balance":"0","granted_balance":"0","topped_up_balance":"0"}]})).unwrap();
    assert_eq!(r.quotas[0].value, Some(0.));
    assert!(r.quotas[0].percent.is_none());
}
#[test]
fn zai_counts_override_percentage_and_mcp_stays_separate() {
    let r=parse(Provider::Zai,&json!({"success":true,"code":200,"data":{"planName":"Pro","limits":[{"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":5,"usage":200,"currentValue":50},{"type":"TIME_LIMIT","unit":1,"number":30,"percentage":7}]}})).unwrap();
    assert_eq!(r.plan.as_deref(), Some("Pro"));
    assert_eq!(r.quotas[0].percent, Some(25.));
    assert_eq!(r.quotas[0].label, "5-hour quota");
    assert!(r.quotas[1].label.starts_with("MCP"));
}
#[test]
fn zai_missing_limits_or_denied_is_not_zero() {
    for value in [
        json!({"success":false,"code":401}),
        json!({"success":true,"code":200,"data":{"limits":[]}}),
        json!({"success":true,"code":200,"data":{"limits":[{"type":"UNKNOWN"}]}}),
    ] {
        assert!(parse(Provider::Zai, &value).is_err());
    }
}
#[test]
fn kimi_ratio_pools_and_counts_are_not_double_counted() {
    let r=parse(Provider::Kimi,&json!({"usages":{"limit_5h":{"used_ratio":0.35,"reset_time":"2030-01-01T00:00:00Z"},"limit_7d":{"used_ratio":0.2,"reset_time":"2030-01-01T00:00:00Z"},"limit_month_total":{"used_ratio":0.8}},"usage":{"limit":"100","used":"25","resetTime":"2030-01-01T00:00:00Z"},"limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},"detail":{"limit":"100","used":"40","resetTime":"2030-01-01T00:00:01Z"}}]})).unwrap();
    assert_eq!(r.quotas.len(), 3);
    assert_eq!(r.quotas[0].percent, Some(40.));
    assert_eq!(r.quotas[1].percent, Some(25.));
    assert_eq!(r.quotas[2].percent, Some(80.));
}
#[test]
fn kimi_legacy_counts_are_supported_and_missing_counters_fail() {
    let r = parse(
        Provider::Kimi,
        &json!({"usage":{"limit":"2048","used":"214","remaining":"1834"}}),
    )
    .unwrap();
    assert!(r.quotas[0].percent.unwrap() > 10.);
    assert!(parse(Provider::Kimi, &json!({"usage":{"limit":"100"}})).is_err());
    assert!(parse(
        Provider::Kimi,
        &json!({"usages":{"limit_5h":{"used_ratio":-1}}})
    )
    .is_err());
}
#[test]
fn opencode_percent_one_is_one_not_one_hundred() {
    let r=parse(Provider::Opencode,&json!({"usage":{"rolling":{"percent":1},"weekly":{"percent":0.5},"monthly":{"percent":92,"resetInSec":1200}}})).unwrap();
    assert_eq!(r.quotas[0].percent, Some(1.));
    assert_eq!(r.quotas[1].percent, Some(0.5));
    assert!(r.quotas[2].resets_at.is_some());
    assert!(parse(Provider::Opencode, &json!({"usage":{}})).is_err());
}
#[test]
fn regions_are_explicit_and_scope_is_key_bound() {
    for p in [
        Provider::Deepseek,
        Provider::Zai,
        Provider::Kimi,
        Provider::Opencode,
    ] {
        let (a, _, scope) = key_scope(p, "synthetic-api-key", "").unwrap();
        assert!(valid_scope(p, &scope, &a));
        let (b, _, other) = key_scope(p, "different-api-key", "").unwrap();
        assert_ne!(scope, other);
        assert!(!valid_scope(p, &scope, &b));
    }
    assert!(region(Provider::Zai, "https://untrusted.example").is_err());
    assert_ne!(
        key_scope(Provider::Kimi, "synthetic-api-key", "cn")
            .unwrap()
            .2,
        key_scope(Provider::Kimi, "synthetic-api-key", "international")
            .unwrap()
            .2
    );
}
#[tokio::test]
async fn real_http_connect_storage_refresh_restart_and_disconnect_all_four() {
    use axum::{http::HeaderMap, routing::get, Json, Router};
    async fn endpoint(headers: HeaderMap, uri: axum::http::Uri) -> Json<Value> {
        assert_eq!(
            headers["authorization"],
            "Bearer synthetic-provider-api-key"
        );
        let value = match uri.path() {
            "/user/balance" => {
                json!({"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"9","granted_balance":"1","topped_up_balance":"8"}]})
            }
            "/coding/v1/usages" => json!({"usages":{"limit_5h":{"used_ratio":0.2}}}),
            "/zen/go/v1/usage" => json!({"usage":{"rolling":{"percent":20}}}),
            _ => {
                json!({"success":true,"code":200,"data":{"limits":[{"type":"CREDIT_LIMIT","unit":3,"number":5,"percentage":20}]}})
            }
        };
        Json(value)
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/{*path}", get(endpoint)))
            .await
            .unwrap()
    });
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    let vault = Arc::new(super::super::tests::MemoryVault::default());
    let mut service = Service::new(storage.clone(), vault.clone());
    service.reported = Reported {
        zai: base.clone(),
        zai_cn: base.clone(),
        kimi: base.clone(),
        kimi_intl: base.clone(),
        deepseek: base.clone(),
        opencode: base,
    };
    let mut settings = Settings {
        launch_at_startup: true,
        theme: "dark".into(),
        ..Default::default()
    };
    for provider in [
        Provider::Deepseek,
        Provider::Zai,
        Provider::Kimi,
        Provider::Opencode,
    ] {
        let (next, state) = service
            .connect(
                &settings,
                ConnectInput {
                    provider,
                    token: "synthetic-provider-api-key".into(),
                    member_email: String::new(),
                    session: false,
                    region: String::new(),
                },
            )
            .await
            .unwrap();
        assert_eq!(state.status, Status::Ready);
        assert_eq!(state.account.provider, provider);
        assert!(state.account.email.is_none());
        assert!(state.account.sources.is_empty());
        settings = next;
        let c = settings.connections.last().unwrap();
        assert!(!service.fetch(c).await.unwrap().quotas.is_empty());
    }
    assert!(settings.launch_at_startup);
    assert_eq!(settings.theme, "dark");
    assert_eq!(storage.load_settings().unwrap().connections.len(), 4);
    let serialized = serde_json::to_string(&settings).unwrap();
    assert!(!serialized.contains("synthetic-provider-api-key"));
    for c in settings.clone().connections {
        settings = service.disconnect(&settings, &c.id).unwrap();
        assert!(vault.read(&c.id).unwrap().is_none());
    }
    assert!(settings.connections.is_empty());
    server.abort();
}
