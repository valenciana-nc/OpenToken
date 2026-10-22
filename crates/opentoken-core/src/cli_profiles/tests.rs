use super::*;
use chrono::Utc;
fn fixture() -> (tempfile::TempDir, Storage) {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage {
        dir: temp.path().join("app"),
        user_home: temp.path().join("home"),
    };
    fs::create_dir_all(&storage.user_home).unwrap();
    (temp, storage)
}
fn account(profile: &Profile) -> AccountState {
    AccountState {
        account: Account {
            id: format!("{}-fixture", profile.provider.key()),
            provider: profile.provider.provider(),
            email: None,
            identity: None,
            label: "test".into(),
            sources: vec![Source {
                path: profile.home.clone(),
                store: Store::File,
            }],
        },
        status: Status::Ready,
        report: Some(Report {
            plan: None,
            observed_at: Utc::now(),
            quotas: vec![],
            source: ReportSource::Provider,
            verified_at: None,
        }),
        fault: None,
    }
}
#[test]
fn prepare_reuses_pending_without_copying_credentials() {
    let (_temp, storage) = fixture();
    let first = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    assert!(first.home.is_dir());
    assert!(!first.home.join("auth.json").exists());
    let second = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    assert_eq!(first.home, second.home);
    assert!(first.local_marker(&storage).unwrap().is_none());
}
#[test]
fn commit_requires_verified_matching_account() {
    let (_temp, storage) = fixture();
    let profile = Profile::prepare(&storage, CliProvider::Claude, &Settings::default()).unwrap();
    let mut state = account(&profile);
    state.status = Status::Faulted;
    assert!(profile
        .commit(&storage, &Settings::default(), &state)
        .is_err());
    state.status = Status::Ready;
    state.account.sources[0].path = storage.user_home.clone();
    assert!(profile
        .commit(&storage, &Settings::default(), &state)
        .is_err());
    assert!(!storage.dir.join("settings.json").exists());
}
#[test]
fn commit_keeps_preferences_and_allocates_separate_next_account() {
    let (_temp, storage) = fixture();
    let settings = Settings {
        launch_at_startup: true,
        dashboard_pinned: false,
        claude_enabled: false,
        theme: "dark".into(),
        ..Default::default()
    };
    let profile = Profile::prepare(&storage, CliProvider::Claude, &settings).unwrap();
    fs::write(
        profile.home.join(".credentials.json"),
        "provider-owned-synthetic-secret",
    )
    .unwrap();
    let next = profile
        .commit(&storage, &settings, &account(&profile))
        .unwrap();
    assert!(next.claude_enabled && next.launch_at_startup);
    assert!(!next.dashboard_pinned);
    assert_eq!(next.theme, "dark");
    assert_eq!(
        fs::read(profile.home.join(".credentials.json")).unwrap(),
        b"provider-owned-synthetic-secret"
    );
    let second = Profile::prepare(&storage, CliProvider::Claude, &next).unwrap();
    assert_ne!(profile.home, second.home);
    assert!(!second.home.join(".credentials.json").exists());
}
#[test]
fn arbitrary_home_and_corrupt_pending_are_rejected() {
    let (_temp, storage) = fixture();
    let outside = Profile {
        provider: CliProvider::Codex,
        home: storage.user_home.clone(),
    };
    assert!(outside.validate(&storage).is_err());
    let profile = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    fs::write(
        profile.home.parent().unwrap().join("pending.json"),
        r#"{"id":"../../outside"}"#,
    )
    .unwrap();
    assert!(Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).is_err());
}
#[test]
fn committed_pending_left_after_failed_cleanup_does_not_reuse_account() {
    let (_temp, storage) = fixture();
    let profile = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    let mut next = Settings::default();
    next.codex_homes
        .push(profile.home.to_string_lossy().into_owned());
    let other = Profile::prepare(&storage, CliProvider::Codex, &next).unwrap();
    assert_ne!(profile.home, other.home);
}
#[test]
fn stale_preferences_keep_the_new_connection_and_preexisting_sources() {
    let (_temp, storage) = fixture();
    let profile = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    let current = Settings {
        codex_homes: vec![
            profile.home.to_string_lossy().into_owned(),
            "C:/legacy".into(),
        ],
        ..Default::default()
    };
    let mut stale = Settings {
        codex_enabled: false,
        ..Default::default()
    };
    preserve_managed(&storage, &current, &mut stale);
    assert_eq!(stale.codex_homes, current.codex_homes);
    assert!(stale.codex_enabled);
}
#[test]
fn stale_preferences_cannot_restore_disconnected_default_accounts() {
    let (_temp, storage) = fixture();
    let current = Settings {
        ignored_homes: vec![storage
            .user_home
            .join(".claude")
            .to_string_lossy()
            .into_owned()],
        ..Default::default()
    };
    let mut stale = Settings::default();
    preserve_managed(&storage, &current, &mut stale);
    assert_eq!(stale.ignored_homes, current.ignored_homes);
}
#[test]
fn enabling_a_new_provider_does_not_add_an_unmonitored_default_profile() {
    let (_temp, storage) = fixture();
    fs::create_dir_all(storage.user_home.join(".claude")).unwrap();
    let before = Settings::default();
    let profile = Profile::prepare(&storage, CliProvider::Claude, &before).unwrap();
    let next = profile
        .commit(&storage, &before, &account(&profile))
        .unwrap();
    assert_eq!(
        next.claude_homes,
        vec![profile.home.to_string_lossy().into_owned()]
    );
}
#[test]
fn existing_implicit_default_is_preserved_when_adding_another_account() {
    let (_temp, storage) = fixture();
    fs::create_dir_all(storage.user_home.join(".claude")).unwrap();
    let before = Settings {
        claude_enabled: true,
        ..Default::default()
    };
    let profile = Profile::prepare(&storage, CliProvider::Claude, &before).unwrap();
    let next = profile
        .commit(&storage, &before, &account(&profile))
        .unwrap();
    assert!(next.claude_homes.contains(
        &storage
            .user_home
            .join(".claude")
            .to_string_lossy()
            .into_owned()
    ));
    assert!(next
        .claude_homes
        .contains(&profile.home.to_string_lossy().into_owned()));
}
#[test]
fn disconnect_excludes_all_sources_without_deleting_credentials() {
    let (_temp, storage) = fixture();
    let profile = Profile::prepare(&storage, CliProvider::Claude, &Settings::default()).unwrap();
    let path = profile.home.join(".credentials.json");
    fs::write(&path, "synthetic-provider-owned").unwrap();
    let next = profile
        .commit(&storage, &Settings::default(), &account(&profile))
        .unwrap();
    let disconnected = disconnect(&storage, &next, &account(&profile).account).unwrap();
    assert!(!disconnected
        .claude_homes
        .iter()
        .any(|p| p == &profile.home.to_string_lossy()));
    assert!(disconnected
        .ignored_homes
        .iter()
        .any(|p| p == &profile.home.to_string_lossy()));
    assert_eq!(fs::read(&path).unwrap(), b"synthetic-provider-owned");
    let claude = crate::claude::ClaudeAdapter {
        http: reqwest::Client::new(),
        user_home: storage.user_home.clone(),
    };
    assert!(!claude
        .accounts(&disconnected)
        .iter()
        .any(|a| a.sources.iter().any(|s| s.path == profile.home)));
}
#[test]
fn cli_commands_use_only_official_subscription_authentication() {
    assert_eq!(CliProvider::Codex.arguments(), ["login"]);
    assert_eq!(
        CliProvider::Claude.arguments(),
        ["auth", "login", "--claudeai"]
    );
    assert_eq!(CliProvider::Claude.environment(), "CLAUDE_CONFIG_DIR");
    assert!(serde_json::from_str::<CliProvider>("\"copilot\"").is_err());
}
#[cfg(windows)]
#[test]
fn rejects_junction_and_oversized_auth_metadata() {
    let (_temp, storage) = fixture();
    let profile = Profile::prepare(&storage, CliProvider::Codex, &Settings::default()).unwrap();
    let auth = profile.home.join("auth.json");
    let file = fs::File::create(&auth).unwrap();
    file.set_len(2 * 1024 * 1024 + 1).unwrap();
    assert!(profile.local_marker(&storage).is_err());
    fs::remove_file(auth).unwrap();
    let target = storage.user_home.join("target");
    fs::create_dir_all(&target).unwrap();
    let link = profile.home.join("linked");
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(safe_directory(&link).is_err());
    fs::remove_dir(link).unwrap();
}
