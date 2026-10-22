use chrono::Utc;
use opentoken_core::{spending::UsageIndex, storage::Settings};
use serde_json::json;
use std::path::Path;
fn line(path: &Path, value: serde_json::Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{}\n", value)).unwrap();
}
#[test]
fn three_client_readers_work_without_login_keep_subsets_and_deduplicate_imports() {
    let temp = tempfile::tempdir().unwrap();
    let now = Utc::now();
    let muse = temp.path().join("muse");
    let grok = temp.path().join("grok");
    let kimi = temp.path().join("kimi");
    let m = muse.join("2026/10/18/session-m/session.jsonl");
    line(
        &m,
        json!({"schema_version":1,"record_type":"event","payload_type":"runtime.session","payload_schema_version":1,"id":"event-m","recorded_at":now.timestamp_micros(),"payload":{"event":{"kind":"model_completed","model":"unpriced-muse-fixture","usage":{"input_tokens":100,"output_tokens":25,"cached_tokens":40,"reasoning_tokens":10},"prompt":"PRIVATE_PROMPT_NOT_INDEXED"}}}),
    );
    let g = grok.join("sessions/project/session-g/updates.jsonl");
    line(
        &g,
        json!({"timestamp":now.timestamp_millis()as f64/1000.,"params":{"sessionId":"session-g","_meta":{"eventId":"event-g"},"update":{"sessionUpdate":"turn_completed","usage":{"modelUsage":{"unpriced-grok-a":{"inputTokens":100,"outputTokens":20,"cachedReadTokens":30},"unpriced-grok-b":{"inputTokens":100,"outputTokens":30,"reasoningTokens":5}}}}}}),
    );
    let k = kimi.join("sessions/project/session-k/agents/agent-1/wire.jsonl");
    line(
        &k,
        json!({"type":"usage.record","usageScope":"turn","time":now.timestamp_millis(),"model":"unpriced-kimi-fixture","usage":{"inputOther":100,"output":10,"inputCacheRead":30,"inputCacheCreation":5}}),
    );
    let roots = vec![
        ("muse".into(), muse),
        ("grok".into(), grok),
        ("kimi".into(), kimi.clone()),
    ];
    let settings = Settings {
        codex_enabled: false,
        claude_enabled: false,
        ..Default::default()
    };
    let mut index = UsageIndex::open(&temp.path().join("index")).unwrap();
    let a = index.scan(&roots, &settings, &[], now).unwrap();
    assert_eq!(a.periods["Today"].tokens, 520);
    assert_eq!(a.periods["Today"].unpriced_tokens, 520);
    assert!(a.periods["Today"].estimated_usd.is_none());
    assert_eq!(a.periods["Today"].providers.len(), 3);
    assert!(
        index.take_activity().is_empty(),
        "anonymous client records must not overwrite account quotas"
    );
    let copy = kimi.join("sessions/copy/session-k/agents/agent-1/wire.jsonl");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(&k, &copy).unwrap();
    assert_eq!(
        index.scan(&roots, &settings, &[], now).unwrap().periods["Today"].tokens,
        520
    );
    let other = kimi.join("sessions/project/different-session/agents/agent-1/wire.jsonl");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    std::fs::copy(&k, &other).unwrap();
    assert_eq!(
        index.scan(&roots, &settings, &[], now).unwrap().periods["Today"].tokens,
        665
    );
    drop(index);
    let db = rusqlite::Connection::open(temp.path().join("index/usage.sqlite3")).unwrap();
    let rows: String = db
        .query_row(
            "SELECT group_concat(id||model||session,' ') FROM events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!rows.contains("PRIVATE_PROMPT_NOT_INDEXED"));
}
#[test]
fn a_muse_partial_append_is_retried_without_faking_zero_or_double_counting() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("sessions");
    let path = root.join("2026/10/18/session-append/session.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let now = Utc::now();
    let value = json!({"schema_version":1,"record_type":"event","payload_type":"runtime.session","payload_schema_version":1,"id":"append-record","recorded_at":now.timestamp_micros(),"payload":{"event":{"kind":"automated_review_completed","model":{"model_id":"unpriced-review"},"usage":{"input_tokens":10,"output_tokens":5}}}});
    let bytes = serde_json::to_vec(&value).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    let mut index = UsageIndex::open(&temp.path().join("index")).unwrap();
    let roots = vec![("muse".into(), root)];
    let empty = index.scan(&roots, &Settings::default(), &[], now).unwrap();
    assert!(empty.periods["Today"].estimated_usd.is_none());
    std::fs::write(&path, format!("{value}\n")).unwrap();
    assert_eq!(
        index
            .scan(&roots, &Settings::default(), &[], now)
            .unwrap()
            .periods["Today"]
            .tokens,
        15
    );
    assert_eq!(
        index
            .scan(&roots, &Settings::default(), &[], now)
            .unwrap()
            .periods["Today"]
            .tokens,
        15
    );
}
