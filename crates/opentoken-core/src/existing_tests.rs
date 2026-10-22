use super::*;
use std::sync::Arc;
struct ExistingCli;
#[async_trait::async_trait]
impl crate::codex::CodexCli for ExistingCli {
    fn available(&self) -> bool {
        true
    }
    async fn read(&self, _: &std::path::Path) -> std::result::Result<crate::codex::Reading, Fault> {
        Ok(crate::codex::Reading {
            identity: Identity {
                workspace: "existing-fixture".into(),
                email: Some("existing@example.test".into()),
                user: None,
            },
            plan: Some("plus".into()),
            usage: serde_json::json!({"rateLimits":{"primary":{"usedPercent":22,"windowDurationMins":300}}}),
        })
    }
}
fn fixture() -> (
    tempfile::TempDir,
    Storage,
    Settings,
    Service,
    CodexAdapter,
    ClaudeAdapter,
) {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    let home = storage.user_home.join(".codex");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        "cli_auth_credentials_store = \"keyring\"\n",
    )
    .unwrap();
    let settings = Settings {
        codex_enabled: false,
        claude_enabled: false,
        auto_discover: false,
        launch_at_startup: true,
        theme: "dark".into(),
        ..Default::default()
    };
    let service = Service::new(
        storage.clone(),
        Arc::new(crate::connections::UnavailableStore),
    );
    let http = reqwest::Client::new();
    let mut codex = CodexAdapter::new(storage.user_home.clone(), http.clone());
    codex.cli = Box::new(ExistingCli);
    let claude = ClaudeAdapter {
        http,
        user_home: storage.user_home.clone(),
    };
    (temp, storage, settings, service, codex, claude)
}
#[tokio::test]
async fn an_existing_keyring_account_connects_without_creating_a_new_profile_or_touching_provider_config(
) {
    let (_dir, storage, settings, service, mut codex, claude) = fixture();
    let path = storage.user_home.join(".codex/config.toml");
    let before = std::fs::read(&path).unwrap();
    let (next, accounts) = connect(
        &storage,
        &settings,
        &service,
        &mut codex,
        &claude,
        Provider::Codex,
        true,
    )
    .await
    .unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(
        accounts[0].report.as_ref().unwrap().quotas[0].percent,
        Some(22.)
    );
    assert!(next.codex_enabled);
    assert!(!next.claude_enabled);
    assert!(next.launch_at_startup);
    assert_eq!(next.theme, "dark");
    assert!(!next.dashboard_pinned);
    assert_eq!(next.codex_homes.len(), 1);
    assert_eq!(
        path_key(std::path::Path::new(&next.codex_homes[0])),
        path_key(&storage.user_home.join(".codex"))
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!storage.dir.join("sign-ins").exists());
}
#[tokio::test]
async fn existing_account_is_not_connected_when_metadata_persistence_fails() {
    let (_dir, storage, settings, service, mut codex, claude) = fixture();
    std::fs::create_dir_all(storage.dir.join("settings.json")).unwrap();
    assert!(connect(
        &storage,
        &settings,
        &service,
        &mut codex,
        &claude,
        Provider::Codex,
        true
    )
    .await
    .is_err());
    assert!(!storage.dir.join("sign-ins").exists());
}
#[tokio::test]
async fn expired_claude_is_not_refreshed_during_existing_session_connection() {
    let (_dir, storage, mut settings, service, mut codex, claude) = fixture();
    let home = storage.user_home.join(".claude");
    std::fs::create_dir_all(&home).unwrap();
    let path = home.join(".credentials.json");
    let bytes=serde_json::to_vec(&serde_json::json!({"claudeAiOauth":{"accessToken":"synthetic-stale-access","refreshToken":"provider-owned-never-used","expiresAt":1}})).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    settings.claude_token_refresh = true;
    assert!(connect(
        &storage,
        &settings,
        &service,
        &mut codex,
        &claude,
        Provider::Claude,
        true
    )
    .await
    .is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert!(!home.join(".opentoken-refresh.lock").exists());
    assert!(!storage.dir.join("sign-ins").exists());
}
