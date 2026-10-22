use opentoken_core::{
    engine::Engine,
    storage::{Settings, Storage},
};
use std::time::Duration;
#[tokio::test]
async fn saves_and_coalesces_refreshes_without_touching_real_homes() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("data"),
        user_home: temp.path().join("user"),
    };
    let settings = Settings {
        auto_discover: false,
        codex_homes: vec![temp
            .path()
            .join("not-signed-in")
            .to_string_lossy()
            .into_owned()],
        ..Default::default()
    };
    let engine = Engine::start(storage.clone(), settings.clone());
    let mut rx = engine.state.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx.changed().await.unwrap();
            if rx.borrow().scanned_at.is_some() && !rx.borrow().refreshing {
                break;
            }
        }
    })
    .await
    .unwrap();
    let start = rx.borrow().revision;
    for _ in 0..10 {
        engine.refresh().await;
    }
    let settings = Settings {
        refresh_minutes: 15,
        ..settings
    };
    engine.save(settings).await.unwrap();
    assert_eq!(storage.load_settings().unwrap().refresh_minutes, 15);
    assert_eq!(engine.settings.borrow().refresh_minutes, 15);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx.changed().await.unwrap();
            if rx.borrow().revision > start && !rx.borrow().refreshing {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(engine.state.borrow().accounts.is_empty());
    let revision = engine.state.borrow().revision;
    let order = vec!["account-b".to_string(), "account-a".to_string()];
    engine.save_account_order(order.clone()).await.unwrap();
    assert_eq!(storage.load_settings().unwrap().account_order, order);
    assert_eq!(engine.settings.borrow().refresh_minutes, 15);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        engine.state.borrow().revision,
        revision,
        "reordering must not cause extra quota reads"
    );
    assert!(engine
        .save_account_order(vec!["duplicate".into(), "duplicate".into()])
        .await
        .is_err());
    assert!(!engine.settings.borrow().dashboard_pinned);
    engine.save_dashboard_pin(true).await.unwrap();
    assert!(storage.load_settings().unwrap().dashboard_pinned);
    engine.save_dashboard_pin(false).await.unwrap();
    assert!(!storage.load_settings().unwrap().dashboard_pinned);
    assert_eq!(engine.settings.borrow().refresh_minutes, 15);
    assert_eq!(engine.settings.borrow().account_order, order);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        engine.state.borrow().revision,
        revision,
        "pinning must not trigger quota reads"
    );
    engine.shutdown().await;
}
#[tokio::test]
async fn failed_pin_persistence_keeps_the_previous_settings() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("data"),
        user_home: temp.path().join("user"),
    };
    let settings = Settings {
        auto_discover: false,
        codex_homes: vec![temp.path().join("missing-home").to_string_lossy().into()],
        ..Default::default()
    };
    let engine = Engine::start(storage.clone(), settings);
    let mut rx = engine.state.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if rx.borrow_and_update().scanned_at.is_some() {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    std::fs::create_dir(storage.dir.join("settings.json")).unwrap();
    assert!(engine.save_dashboard_pin(true).await.is_err());
    assert!(!engine.settings.borrow().dashboard_pinned);
    engine.shutdown().await;
}
