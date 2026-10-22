use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use opentoken_core::{
    activity::{Activity, LogQuota, LogWindow},
    model::*,
};
use serde_json::json;
use std::{fs, path::Path};
fn auth(home: &Path, workspace: &str) {
    let claims = json!({"email":"fixture@example.test","https://api.openai.com/auth":{"chatgpt_account_id":workspace}});
    let token = format!(
        "e30.{}.sig",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    );
    fs::write(
        home.join("auth.json"),
        json!({"tokens":{"access_token":token,"account_id":workspace}}).to_string(),
    )
    .unwrap();
}
fn fixture(home: &Path) -> (AccountState, Activity) {
    auth(home, "fixture-workspace");
    let now = Utc::now();
    let reset = chrono::DateTime::from_timestamp(now.timestamp() + 3600, 0).unwrap();
    let identity = Identity {
        workspace: "fixture-workspace".into(),
        email: Some("fixture@example.test".into()),
        user: None,
    };
    let state = AccountState {
        account: Account {
            id: identity.id(),
            provider: Provider::Codex,
            label: "fixture".into(),
            email: identity.email.clone(),
            identity: Some(identity.clone()),
            sources: vec![Source {
                path: home.into(),
                store: Store::File,
            }],
        },
        status: Status::Ready,
        fault: None,
        report: Some(Report {
            plan: Some("Plus".into()),
            quotas: vec![
                Quota::window("session", "Session", 25., Some(reset)),
                Quota::value("credits", "Credits", 50., "credits"),
            ],
            observed_at: now - Duration::seconds(10),
            source: ReportSource::Provider,
            verified_at: Some(now - Duration::seconds(10)),
        }),
    };
    let activity = Activity {
        home: home.into(),
        provider: Provider::Codex,
        observed_at: now - Duration::seconds(1),
        identity: Some(identity),
        quota: Some(LogQuota {
            primary: Some(LogWindow {
                used_percent: 30.,
                window_minutes: 300,
                resets_at: reset.timestamp(),
            }),
            secondary: None,
            limit_id: Some("codex".into()),
        }),
    };
    (state, activity)
}
#[test]
fn explicit_identity_updates_only_quota_windows_and_preserves_verification_and_credits() {
    let t = tempfile::tempdir().unwrap();
    let (mut state, activity) = fixture(t.path());
    let verified = state.report.as_ref().unwrap().observed_at;
    assert!(activity.apply(&mut state, Utc::now()));
    let report = state.report.unwrap();
    assert_eq!(report.source, ReportSource::CliLog);
    assert_eq!(report.verified_at, Some(verified));
    assert_eq!(report.quotas[0].percent, Some(30.));
    assert_eq!(report.quotas[1].value, Some(50.));
}
#[test]
fn anonymous_wrong_identity_stale_faulted_and_wrong_home_records_never_change_badges() {
    let t = tempfile::tempdir().unwrap();
    let (state, activity) = fixture(t.path());
    for case in 0..7 {
        let mut current = state.clone();
        let mut sample = activity.clone();
        match case {
            0 => sample.identity = None,
            1 => sample.identity.as_mut().unwrap().workspace = "other".into(),
            2 => sample.home = t.path().join("unrelated"),
            3 => current.status = Status::Stale,
            4 => current.status = Status::Faulted,
            5 => sample.observed_at = Utc::now() - Duration::minutes(3),
            _ => sample.observed_at = Utc::now() + Duration::minutes(1),
        }
        let before = serde_json::to_value(&current).unwrap();
        assert!(!sample.apply(&mut current, Utc::now()), "case {case}");
        assert_eq!(serde_json::to_value(current).unwrap(), before);
    }
}
#[test]
fn changed_auth_decreases_and_guessed_resets_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let (state, activity) = fixture(t.path());
    for case in 0..3 {
        let mut current = state.clone();
        let mut sample = activity.clone();
        auth(t.path(), "fixture-workspace");
        match case {
            0 => auth(t.path(), "switched-workspace"),
            1 => {
                sample
                    .quota
                    .as_mut()
                    .unwrap()
                    .primary
                    .as_mut()
                    .unwrap()
                    .used_percent = 20.
            }
            _ => {
                sample
                    .quota
                    .as_mut()
                    .unwrap()
                    .primary
                    .as_mut()
                    .unwrap()
                    .resets_at += 300
            }
        }
        assert!(!sample.apply(&mut current, Utc::now()));
        assert_eq!(current.report.unwrap().quotas[0].percent, Some(25.));
    }
}
