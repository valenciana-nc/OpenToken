use chrono::{Duration, Local, Utc};
use opentoken_core::{
    model::{Account, AccountState, Provider, Quota, Report, Status},
    spending::{ModelPrice, UsageIndex},
    storage::Settings,
};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
fn price() -> ModelPrice {
    ModelPrice {
        input: 2.0,
        cached: 0.1,
        output: 10.0,
        cache_write: 2.5,
        cache_write_hour: 4.0,
    }
}
fn setup() -> (tempfile::TempDir, Settings, Vec<(String, PathBuf)>) {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    fs::create_dir_all(home.join("sessions")).unwrap();
    let mut s = Settings {
        auto_discover: false,
        codex_homes: vec![home.to_string_lossy().into()],
        ..Default::default()
    };
    s.model_prices.insert("fixture-model".into(), price());
    (t, s, vec![("codex".into(), home)])
}
fn usage() -> Value {
    json!({"input_tokens":1500,"cached_input_tokens":500,"output_tokens":100,"reasoning_output_tokens":50})
}
fn header() -> Vec<Value> {
    vec![
        json!({"type":"session_meta","payload":{"id":"session-fixture"}}),
        json!({"type":"turn_context","payload":{"model":"fixture-model","turn_id":"fixture-turn"}}),
    ]
}
fn direct(id: &str, at: chrono::DateTime<Utc>) -> Value {
    json!({"type":"token_usage_record","timestamp":at.to_rfc3339(),"payload":{"session_id":"session-fixture","response_id":id,"usage":usage()}})
}
fn fallback(at: chrono::DateTime<Utc>, total: Value) -> Value {
    json!({"type":"event_msg","timestamp":at.to_rfc3339(),"payload":{"type":"token_count","info":{"total_token_usage":total,"last_token_usage":usage()}}})
}
fn write(path: &Path, records: &[Value]) {
    let mut f = fs::File::create(path).unwrap();
    for r in records {
        writeln!(f, "{r}").unwrap();
    }
}
#[test]
fn cached_tokens_reasoning_and_copied_codex_echoes_are_not_double_counted() {
    let (t, s, roots) = setup();
    let at = Utc::now() - Duration::seconds(5);
    let mut records = header();
    records.push(direct("response-1", at));
    records.push(fallback(at + Duration::milliseconds(20), usage()));
    records.push(fallback(at + Duration::seconds(3), usage()));
    records
        .push(json!({"type":"response_item","payload":{"content":"PRIVATE_PROMPT_DO_NOT_STORE"}}));
    let p = roots[0].1.join("sessions/a.jsonl");
    write(&p, &records);
    fs::copy(p, roots[0].1.join("sessions/copy.jsonl")).unwrap();
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let h = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert_eq!(h.periods["30 Days"].tokens, 1600);
    assert!((h.periods["30 Days"].estimated_usd.unwrap() - 0.00305).abs() < 1e-10);
    assert!(!h.indexing);
    let again = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert_eq!(again.periods["30 Days"].tokens, 1600);
    drop(index);
    for f in fs::read_dir(t.path().join("data")).unwrap().flatten() {
        assert!(!String::from_utf8_lossy(&fs::read(f.path()).unwrap())
            .contains("PRIVATE_PROMPT_DO_NOT_STORE"));
    }
}
#[test]
fn legacy_counters_are_deduplicated_and_unknown_rates_can_be_overridden() {
    let (t, mut s, roots) = setup();
    s.model_prices.clear();
    let at = Utc::now() - Duration::seconds(10);
    let mut r = header();
    r.push(fallback(at, usage()));
    r.push(fallback(at + Duration::seconds(1), usage()));
    write(&roots[0].1.join("sessions/legacy.jsonl"), &r);
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let h = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert_eq!(h.periods["30 Days"].tokens, 1600);
    assert_eq!(h.periods["30 Days"].estimated_usd, None);
    assert_eq!(h.periods["30 Days"].unpriced_tokens, 1600);
    assert_eq!(h.unpriced_models, ["fixture-model"]);
    s.model_prices.insert("fixture-model".into(), price());
    let h = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert_eq!(h.periods["30 Days"].unpriced_tokens, 0);
    assert!(h.periods["30 Days"].estimated_usd.unwrap() > 0.0);
}
#[test]
fn incomplete_appends_and_restart_cursors_do_not_lose_or_repeat_usage() {
    let (t, s, roots) = setup();
    let p = roots[0].1.join("sessions/partial.jsonl");
    let at = Utc::now() - Duration::seconds(10);
    let mut r = header();
    r.push(direct("one", at));
    write(&p, &r);
    let second = format!("{}\n", direct("two", at + Duration::seconds(1)));
    let half = second.len() / 2;
    OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(&second.as_bytes()[..half])
        .unwrap();
    {
        let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
        let h = index.scan(&roots, &s, &[], Utc::now()).unwrap();
        assert_eq!(h.periods["30 Days"].tokens, 1600);
        assert!(!h.indexing);
    }
    OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(&second.as_bytes()[half..])
        .unwrap();
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let h = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert_eq!(h.periods["30 Days"].tokens, 3200);
}
#[test]
fn claude_streaming_updates_and_cache_write_durations_are_counted_once() {
    let (t, mut s, _) = setup();
    s.model_prices.insert("claude-fixture".into(), price());
    let home = t.path().join("claude");
    fs::create_dir_all(home.join("projects/a/subagents")).unwrap();
    let at = Utc::now() - Duration::seconds(10);
    let record = |output| json!({"type":"assistant","timestamp":at.to_rfc3339(),"message":{"id":"message-id","model":"claude-fixture","content":"SECRET","usage":{"input_tokens":100,"cache_read_input_tokens":200,"cache_creation_input_tokens":50,"cache_creation":{"ephemeral_1h_input_tokens":20,"ephemeral_5m_input_tokens":30},"output_tokens":output}}});
    write(
        &home.join("projects/a/subagents/agent.jsonl"),
        &[record(10), record(20), record(20)],
    );
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let h = index
        .scan(&[("claude".into(), home)], &s, &[], Utc::now())
        .unwrap();
    assert_eq!(h.periods["30 Days"].tokens, 370);
    assert!((h.periods["30 Days"].estimated_usd.unwrap() - 0.000575).abs() < 1e-10);
}
#[test]
fn periods_use_local_calendar_days_and_known_long_context_rates() {
    let (t, s, roots) = setup();
    let now = Utc::now();
    let yesterday = Local::now()
        .date_naive()
        .pred_opt()
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap()
        .with_timezone(&Utc);
    let mut r = header();
    r.push(direct("yesterday", yesterday));
    r.push(direct("today", now - Duration::seconds(1)));
    write(&roots[0].1.join("sessions/days.jsonl"), &r);
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let h = index.scan(&roots, &s, &[], now).unwrap();
    assert_eq!(h.periods["Today"].tokens, 1600);
    assert_eq!(h.periods["Yesterday"].tokens, 1600);
    assert_eq!(h.periods["30 Days"].tokens, 3200);
    let mut r = header();
    r[1] = json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}});
    r.push(json!({"type":"token_usage_record","timestamp":now.to_rfc3339(),"payload":{"response_id":"long-context","usage":{"input_tokens":300000,"cached_input_tokens":200000,"output_tokens":1000}}}));
    write(&roots[0].1.join("sessions/long.jsonl"), &r);
    let h = index.scan(&roots, &s, &[], now).unwrap();
    assert!((h.periods["30 Days"].estimated_usd.unwrap() - (0.455 + 0.0061)).abs() < 1e-8);
}
#[test]
fn large_nonusage_records_are_bounded_and_do_not_pollute_the_index() {
    let (t, s, roots) = setup();
    let p = roots[0].1.join("sessions/large.jsonl");
    write(&p, &header());
    let mut f = OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(b"{\"type\":\"response_item\",\"payload\":\"")
        .unwrap();
    f.write_all(&vec![b'x'; 9 * 1024 * 1024]).unwrap();
    f.write_all(b"\"}\n").unwrap();
    writeln!(
        f,
        "{}",
        direct("after-large", Utc::now() - Duration::seconds(2))
    )
    .unwrap();
    drop(f);
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    let a = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert!(a.indexing);
    assert_eq!(a.periods["30 Days"].tokens, 0);
    let b = index.scan(&roots, &s, &[], Utc::now()).unwrap();
    assert!(!b.indexing);
    assert_eq!(b.periods["30 Days"].tokens, 1600);
    assert_eq!(b.skipped_records, 0);
}
#[test]
fn credit_decreases_and_topups_are_separate_and_stale_balances_are_ignored() {
    let (t, s, roots) = setup();
    let now = Utc::now();
    let mut account = AccountState {
        account: Account {
            id: "verified-id".into(),
            provider: Provider::Codex,
            label: "fixture".into(),
            email: None,
            identity: None,
            sources: vec![],
        },
        status: Status::Ready,
        report: Some(Report {
            plan: None,
            observed_at: now,
            source: opentoken_core::model::ReportSource::Provider,
            verified_at: Some(now),
            quotas: vec![Quota {
                id: "credits".into(),
                label: "Credits".into(),
                percent: None,
                value: Some(100.0),
                unit: Some("credits".into()),
                resets_at: None,
            }],
        }),
        fault: None,
    };
    let mut index = UsageIndex::open(&t.path().join("data")).unwrap();
    index
        .scan(&roots, &s, &[account.clone()], now - Duration::minutes(2))
        .unwrap();
    account.report.as_mut().unwrap().quotas[0].value = Some(90.0);
    index
        .scan(&roots, &s, &[account.clone()], now - Duration::minutes(1))
        .unwrap();
    account.report.as_mut().unwrap().quotas[0].value = Some(110.0);
    let h = index.scan(&roots, &s, &[account.clone()], now).unwrap();
    let changes = &h.periods["30 Days"].credit_changes;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].decrease, 10.0);
    assert_eq!(changes[0].increase, 20.0);
    account.status = Status::Stale;
    account.report.as_mut().unwrap().quotas[0].value = Some(0.0);
    let h = index.scan(&roots, &s, &[account], now).unwrap();
    assert_eq!(h.periods["30 Days"].credit_changes[0].decrease, 10.0);
}
#[test]
fn explicit_sources_and_ignored_homes_are_respected() {
    let (t, mut s, _) = setup();
    let roots = opentoken_core::spending::sources(&s, &[], &t.path().join("unrelated-user"));
    assert_eq!(roots.len(), 1);
    assert!(roots[0].1.ends_with("home"));
    s.ignored_homes = s.codex_homes.clone();
    assert!(opentoken_core::spending::sources(&s, &[], t.path()).is_empty());
    s.model_prices.insert(
        "bad".into(),
        ModelPrice {
            input: f64::NAN,
            ..price()
        },
    );
    assert!(s.validate().is_err());
}
