use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub refresh_minutes: u32,
    pub live_tracking: bool,
    pub history_sources: BTreeMap<crate::model::Provider, bool>,
    pub launch_at_startup: bool,
    pub codex_enabled: bool,
    pub claude_enabled: bool,
    pub auto_discover: bool,
    pub codex_homes: Vec<String>,
    pub claude_homes: Vec<String>,
    pub ignored_homes: Vec<String>,
    pub aliases: BTreeMap<String, String>,
    pub account_order: Vec<String>,
    pub connections: Vec<crate::connections::Connection>,
    pub model_prices: BTreeMap<String, crate::spending::ModelPrice>,
    pub tray_style: String,
    pub tray_metric: String,
    pub flyout_position: String,
    pub dashboard_pinned: bool,
    pub theme: String,
    pub api_enabled: bool,
    pub api_port: u16,
    pub claude_token_refresh: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            refresh_minutes: 5,
            live_tracking: true,
            history_sources: BTreeMap::new(),
            launch_at_startup: false,
            codex_enabled: true,
            claude_enabled: false,
            auto_discover: true,
            codex_homes: vec![],
            claude_homes: vec![],
            ignored_homes: vec![],
            aliases: BTreeMap::new(),
            account_order: vec![],
            connections: vec![],
            model_prices: BTreeMap::new(),
            tray_style: "badge".into(),
            tray_metric: "auto".into(),
            flyout_position: "bottom_right".into(),
            dashboard_pinned: false,
            theme: "light".into(),
            api_enabled: true,
            api_port: 6737,
            claude_token_refresh: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("Unsupported settings version. Update OpenToken before editing this file.");
        }
        if ![1, 5, 15, 30].contains(&self.refresh_minutes) {
            bail!("Refresh interval must be 1, 5, 15 or 30 minutes.");
        }
        if !["badge", "provider_pair"].contains(&self.tray_style.as_str()) {
            bail!("Invalid tray style.");
        }
        if !["auto", "session", "weekly"].contains(&self.tray_metric.as_str()) {
            bail!("Invalid tray metric.");
        }
        if !["bottom_left", "bottom_right", "tray"].contains(&self.flyout_position.as_str()) {
            bail!("Invalid dashboard position.");
        }
        if !["light", "dark", "system"].contains(&self.theme.as_str()) {
            bail!("Invalid theme.");
        }
        if self.api_port < 1024 {
            bail!("Use an API port from 1024 to 65535.");
        }
        if self.history_sources.len() > 12 {
            bail!("Invalid local-history source selection.");
        }
        validate_account_order(&self.account_order)?;
        crate::connections::validate_connections(&self.connections)?;
        if self.model_prices.len() > 100 {
            bail!("At most 100 custom model rates can be configured.");
        }
        for (model, price) in &self.model_prices {
            if model.trim().is_empty() || model.len() > 128 {
                bail!("Enter a model name of at most 128 characters.");
            }
            price.validate()?;
        }
        if self.codex_homes.len() + self.claude_homes.len() > 100 {
            bail!("At most 100 login folders can be configured.");
        }
        Ok(())
    }
}

pub fn validate_account_order(order: &[String]) -> Result<()> {
    let mut unique = std::collections::HashSet::new();
    if order.len() > 1000
        || order
            .iter()
            .any(|id| id.is_empty() || id.len() > 128 || !unique.insert(id))
    {
        bail!("Invalid account order: use unique account IDs (at most 1000).");
    }
    Ok(())
}

#[derive(Clone)]
pub struct Storage {
    pub dir: PathBuf,
    pub user_home: PathBuf,
}
impl Storage {
    pub fn system() -> Self {
        let user_home = directories::UserDirs::new()
            .map(|u| u.home_dir().to_path_buf())
            .unwrap_or_default();
        let dir = std::env::var_os("OPENTOKEN_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| user_home.join(".local/share"))
                    .join("OpenToken")
            });
        Self { dir, user_home }
    }
    pub fn load_settings(&self) -> Result<Settings> {
        let path = self.dir.join("settings.json");
        if !path.exists() {
            return Ok(Settings::default());
        }
        let settings: Settings = read_json(&path)
            .or_else(|_| read_json(&self.dir.join("settings.json.bak")))
            .context(
                "Settings and their backup could not be read; neither file has been overwritten.",
            )?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join("settings.json");
        if let Ok(mut old) = read_json::<Settings>(&path) {
            if old.validate().is_ok() {
                // Automatic corruption recovery must not resurrect permission to issue vendor keys.
                // The primary retains an explicitly approved mode; backups require fresh approval.
                old.connections.retain(|c| {
                    !matches!(
                        c.mode,
                        crate::connections::ConnectionMode::MuseDevice
                            | crate::connections::ConnectionMode::AntigravityCliUsage
                    ) && settings.connections.contains(c)
                });
                atomic_json(&self.dir.join("settings.json.bak"), &old)?;
            }
        }
        atomic_json(&path, settings)
    }
    pub fn import_quotatray(&self) -> Result<Settings> {
        let parent = self
            .dir
            .parent()
            .context("Missing application data parent")?;
        let v: serde_json::Value = read_json(&parent.join("QuotaTray/settings.json"))?;
        let mut s = Settings {
            refresh_minutes: u32::try_from(v["RefreshMinutes"].as_u64().unwrap_or(5))
                .context("Invalid refresh interval")?,
            codex_enabled: v["CodexEnabled"].as_bool().unwrap_or(true),
            claude_enabled: v["ClaudeEnabled"].as_bool().unwrap_or(false),
            auto_discover: v["AutoDiscoverCodexAccounts"].as_bool().unwrap_or(true),
            ..Default::default()
        };
        if let Some(homes) = v["CodexAccounts"].as_array() {
            for a in homes {
                if let Some(p) = a["HomePath"].as_str().filter(|s| !s.is_empty()) {
                    s.codex_homes.push(p.to_string());
                }
            }
        }
        if let Some(homes) = v["IgnoredCodexHomes"].as_array() {
            s.ignored_homes = homes
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        s.validate()?;
        Ok(s) // No writes and no changes to QuotaTray or Windows startup.
    }
}
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let meta = fs::metadata(path)?;
    if meta.len() > 4 * 1024 * 1024 {
        bail!("File exceeds the 4 MB safety limit");
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic_bytes(path, &serde_json::to_vec_pretty(value)?)
}
pub fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
pub fn normalize(path: &str, user: &Path) -> PathBuf {
    let expanded = if path.is_empty() {
        user.join(".codex").to_string_lossy().into_owned()
    } else if path == "~" {
        user.to_string_lossy().into_owned()
    } else if path.starts_with("~/") || path.starts_with("~\\") {
        user.join(&path[2..]).to_string_lossy().into_owned()
    } else {
        let mut s = path.to_string();
        for (key, value) in std::env::vars() {
            s = s.replace(&format!("%{key}%"), &value);
        }
        s
    };
    let p = PathBuf::from(expanded);
    let p = if p.is_absolute() { p } else { user.join(p) };
    fs::canonicalize(&p).unwrap_or(p)
}
pub fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .trim_end_matches(['/', '\\'])
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_settings_get_the_fluent_badge_without_losing_preferences() {
        let mut settings: Settings = serde_json::from_str(
            r#"{"theme":"dark","apiPort":6738,"flyoutPosition":"bottom_left"}"#,
        )
        .unwrap();
        assert_eq!(settings.tray_style, "badge");
        assert!(
            !settings.dashboard_pinned,
            "profiles without a pin preference start unpinned"
        );
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.api_port, 6738);
        assert_eq!(settings.flyout_position, "bottom_left");
        assert!(settings.validate().is_ok());
        settings.tray_style = "unrecognized".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn explicit_pin_choices_are_preserved_instead_of_reset_to_the_default() {
        assert!(!Settings::default().dashboard_pinned);
        let temp = tempfile::tempdir().unwrap();
        let store = Storage {
            dir: temp.path().into(),
            user_home: temp.path().into(),
        };
        for pinned in [true, false] {
            let settings: Settings = serde_json::from_value(
                serde_json::json!({"dashboardPinned":pinned,"theme":"dark"}),
            )
            .unwrap();
            store.save_settings(&settings).unwrap();
            let restored = store.load_settings().unwrap();
            assert_eq!(restored.dashboard_pinned, pinned);
            assert_eq!(restored.theme, "dark");
        }
    }
    #[test]
    fn backup_recovers_without_overwriting_corrupt_file() {
        let t = tempfile::tempdir().unwrap();
        let store = Storage {
            dir: t.path().into(),
            user_home: t.path().into(),
        };
        let mut s = Settings::default();
        store.save_settings(&s).unwrap();
        s.refresh_minutes = 15;
        store.save_settings(&s).unwrap();
        fs::write(t.path().join("settings.json"), "{bad").unwrap();
        assert_eq!(store.load_settings().unwrap().refresh_minutes, 5);
        assert_eq!(
            fs::read_to_string(t.path().join("settings.json")).unwrap(),
            "{bad"
        );
    }
    #[test]
    fn validates_bounds() {
        let s = Settings {
            refresh_minutes: 0,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }
    #[test]
    fn saved_settings_take_precedence() {
        let t = tempfile::tempdir().unwrap();
        let store = Storage {
            dir: t.path().into(),
            user_home: t.path().into(),
        };
        let s = Settings {
            refresh_minutes: 30,
            ..Default::default()
        };
        store.save_settings(&s).unwrap();
        assert_eq!(store.load_settings().unwrap().refresh_minutes, 30);
    }
}
