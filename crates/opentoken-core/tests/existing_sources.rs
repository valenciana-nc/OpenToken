use opentoken_core::{
    engine::Engine,
    model::Provider,
    storage::{Settings, Storage},
};
#[tokio::test]
async fn local_history_is_independent_of_quota_login_and_has_targeted_durable_controls() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    let settings = Settings {
        codex_enabled: false,
        claude_enabled: false,
        launch_at_startup: true,
        dashboard_pinned: false,
        ..Default::default()
    };
    let engine = Engine::start(storage.clone(), settings);
    assert!(engine
        .set_history_source(Provider::Muse, true, false)
        .await
        .is_err());
    assert!(!storage.dir.join("sign-ins").exists());
    let result = engine
        .set_history_source(Provider::Muse, true, true)
        .await
        .unwrap();
    assert!(result.history_enabled);
    assert_eq!(result.connected, 0);
    assert!(!result.settings.claude_enabled);
    assert!(!result.settings.codex_enabled);
    assert!(result.settings.launch_at_startup);
    assert!(!result.settings.dashboard_pinned);
    assert!(!storage.dir.join("sign-ins").exists());
    assert!(engine.state.borrow().accounts.is_empty());
    let mut stale = result.settings.clone();
    stale.history_sources.clear();
    stale.theme = "dark".into();
    engine.save(stale).await.unwrap();
    assert_eq!(
        engine
            .settings
            .borrow()
            .history_sources
            .get(&Provider::Muse),
        Some(&true)
    );
    let stopped = engine
        .set_history_source(Provider::Muse, false, true)
        .await
        .unwrap();
    assert!(!stopped.history_enabled);
    assert_eq!(
        storage
            .load_settings()
            .unwrap()
            .history_sources
            .get(&Provider::Muse),
        Some(&false)
    );
    engine.shutdown().await;
}
#[tokio::test]
async fn unsupported_readers_do_not_turn_login_into_fabricated_usage() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    let engine = Engine::start(
        storage.clone(),
        Settings {
            codex_enabled: false,
            claude_enabled: false,
            ..Default::default()
        },
    );
    for p in [Provider::Antigravity, Provider::Devin] {
        assert!(engine.connect_existing(p, true).await.is_err());
        assert!(engine.set_history_source(p, true, true).await.is_err());
    }
    assert!(engine.state.borrow().accounts.is_empty());
    assert!(!storage.dir.join("sign-ins").exists());
    engine.shutdown().await;
}
#[tokio::test]
async fn denied_existing_session_consent_cannot_create_profiles_or_enable_accounts() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    let settings = Settings {
        codex_enabled: false,
        claude_enabled: false,
        ..Default::default()
    };
    let engine = Engine::start(storage.clone(), settings);
    for p in [
        Provider::Codex,
        Provider::Claude,
        Provider::Cursor,
        Provider::Copilot,
        Provider::Grok,
    ] {
        assert!(engine.connect_existing(p, false).await.is_err());
    }
    assert!(engine.state.borrow().accounts.is_empty());
    assert!(!engine.settings.borrow().codex_enabled);
    assert!(!engine.settings.borrow().claude_enabled);
    assert!(!storage.dir.join("sign-ins").exists());
    engine.shutdown().await;
}
