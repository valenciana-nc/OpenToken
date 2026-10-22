use chrono::Utc;
use opentoken_core::{
    engine::Command,
    model::Snapshot,
    spending::ModelPrice,
    storage::{Settings, Storage},
    tracking::{Tracker, TrackingState},
};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::Duration,
};
use tokio::sync::{mpsc, watch};
fn settings(home: &Path) -> Settings {
    let mut s = Settings {
        auto_discover: false,
        codex_homes: vec![home.to_string_lossy().into()],
        ..Default::default()
    };
    s.model_prices.insert(
        "fixture-model".into(),
        ModelPrice {
            input: 2.,
            cached: 0.1,
            output: 10.,
            cache_write: 2.5,
            cache_write_hour: 4.,
        },
    );
    s
}
fn header() -> String {
    format!(
        "{}\n{}\n",
        json!({"type":"session_meta","payload":{"id":"live-session"}}),
        json!({"type":"turn_context","payload":{"model":"fixture-model"}})
    )
}
fn usage(id: &str) -> String {
    json!({"type":"token_usage_record","timestamp":Utc::now(),"payload":{"session_id":"live-session","response_id":id,"usage":{"input_tokens":1000,"output_tokens":10}}}).to_string()
}
fn append(path: &Path, text: &str) {
    let mut f = OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    f.flush().unwrap();
}
async fn wait(rx: &mut watch::Receiver<TrackingState>, predicate: impl Fn(&TrackingState) -> bool) {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if predicate(&rx.borrow_and_update()) {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("background tracker did not publish in time");
}
#[tokio::test]
async fn pushes_appends_partial_lines_and_restart_without_ipc_or_manual_refresh() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let logs = home.join("sessions/2026/10/15");
    fs::create_dir_all(&logs).unwrap();
    let file = logs.join("live.jsonl");
    fs::write(&file, header()).unwrap();
    let storage = Storage {
        dir: t.path().join("data"),
        user_home: t.path().join("unrelated-user"),
    };
    let (cfg, settings_rx) = watch::channel(settings(&home));
    let (_state, state_rx) = watch::channel(Snapshot::default());
    let (tx, mut commands) = mpsc::channel(16);
    let tracker = Tracker::start(
        storage.clone(),
        settings_rx.clone(),
        state_rx.clone(),
        tx.clone(),
    );
    tracker.set_visible(true);
    let mut rx = tracker.state.clone();
    wait(&mut rx, |s| s.mode == "watching" && s.data.is_some()).await;
    let start = tokio::time::Instant::now();
    append(&file, &(usage("one") + "\n"));
    wait(&mut rx, |s| {
        s.data
            .as_ref()
            .is_some_and(|d| d.periods["Today"].tokens == 1010)
    })
    .await;
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "local push should not wait for the old 30-second poll"
    );
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), commands.recv())
            .await
            .unwrap(),
        Some(Command::Activity(_))
    ));
    let second = usage("two");
    append(&file, &second);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(
        rx.borrow().data.as_ref().unwrap().periods["Today"].tokens,
        1010
    );
    append(&file, "\n");
    wait(&mut rx, |s| {
        s.data
            .as_ref()
            .is_some_and(|d| d.periods["Today"].tokens == 2020)
    })
    .await;
    let mut next = cfg.borrow().clone();
    next.live_tracking = false;
    cfg.send_replace(next);
    wait(&mut rx, |s| s.mode == "off").await;
    append(&file, &(usage("three") + "\n"));
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        rx.borrow().data.as_ref().unwrap().periods["Today"].tokens,
        2020
    );
    tracker.shutdown().await;
    let restarted = Tracker::start(storage, settings_rx, state_rx, tx);
    let mut latest = restarted.state.clone();
    wait(&mut latest, |s| {
        s.data
            .as_ref()
            .is_some_and(|d| d.periods["Today"].tokens == 3030)
    })
    .await;
    restarted.shutdown().await;
}
#[tokio::test]
async fn discovers_new_history_directories_and_stops_importing_removed_roots() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let storage = Storage {
        dir: t.path().join("data"),
        user_home: t.path().join("unrelated"),
    };
    let (cfg, settings_rx) = watch::channel(settings(&home));
    let (_state, state_rx) = watch::channel(Snapshot::default());
    let (tx, _commands) = mpsc::channel(16);
    let tracker = Tracker::start(storage, settings_rx, state_rx, tx);
    let mut rx = tracker.state.clone();
    wait(&mut rx, |s| s.mode == "watching" && s.data.is_some()).await;
    let logs = home.join("sessions/new/day");
    fs::create_dir_all(&logs).unwrap();
    let file = logs.join("new.jsonl");
    fs::write(&file, header() + &usage("new") + "\n").unwrap();
    wait(&mut rx, |s| {
        s.data
            .as_ref()
            .is_some_and(|d| d.periods["Today"].tokens == 1010)
    })
    .await;
    let other = t.path().join("other");
    fs::create_dir_all(&other).unwrap();
    let mut s = settings(&other);
    s.ignored_homes.push(home.to_string_lossy().into());
    cfg.send_replace(s);
    let previous = rx.borrow().revision;
    wait(&mut rx, |s| s.revision > previous).await;
    append(&file, &(usage("excluded") + "\n"));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        rx.borrow().data.as_ref().unwrap().periods["Today"].tokens,
        1010
    );
    tracker.shutdown().await;
}
#[tokio::test]
async fn claude_project_revisions_push_without_double_counting() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("claude");
    let logs = home.join("projects/work/subagents");
    fs::create_dir_all(&logs).unwrap();
    let file = logs.join("agent.jsonl");
    fs::write(&file, "").unwrap();
    let cfg = Settings {
        codex_enabled: false,
        claude_enabled: true,
        claude_homes: vec![home.to_string_lossy().into()],
        auto_discover: false,
        ..Default::default()
    };
    let (_cfg, settings_rx) = watch::channel(cfg);
    let (_state, state_rx) = watch::channel(Snapshot::default());
    let (tx, mut commands) = mpsc::channel(8);
    let tracker = Tracker::start(
        Storage {
            dir: t.path().join("data"),
            user_home: t.path().join("unrelated"),
        },
        settings_rx,
        state_rx,
        tx,
    );
    tracker.set_visible(true);
    let mut rx = tracker.state.clone();
    wait(&mut rx, |s| s.data.is_some() && s.mode == "watching").await;
    for output in [20, 40] {
        append(&file, &(json!({"type":"assistant","timestamp":Utc::now(),"message":{"id":"streamed-message","model":"claude-fixture","usage":{"input_tokens":100,"output_tokens":output}}}).to_string() + "\n"));
        wait(&mut rx, |s| {
            s.data
                .as_ref()
                .is_some_and(|d| d.periods["Today"].tokens == 100 + output)
        })
        .await;
    }
    assert!(
        matches!(commands.recv().await, Some(Command::Activity(a)) if a.provider == opentoken_core::model::Provider::Claude)
    );
    tracker.shutdown().await;
}
#[tokio::test]
async fn index_failure_is_disclosed_and_recovers_without_restarting() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let data = t.path().join("not-a-directory");
    fs::write(&data, "fixture").unwrap();
    let (_cfg, settings_rx) = watch::channel(settings(&home));
    let (_state, state_rx) = watch::channel(Snapshot::default());
    let (tx, _commands) = mpsc::channel(8);
    let tracker = Tracker::start(
        Storage {
            dir: data.clone(),
            user_home: t.path().join("other"),
        },
        settings_rx,
        state_rx,
        tx,
    );
    let mut rx = tracker.state.clone();
    wait(&mut rx, |s| s.error.is_some()).await;
    assert!(rx.borrow().data.is_none());
    fs::remove_file(&data).unwrap();
    tracker.set_visible(true);
    wait(&mut rx, |s| s.error.is_none() && s.data.is_some()).await;
    tracker.shutdown().await;
}
