//! App-managed CLI profile locations. Providers create/own their credentials.
//! No path, credential or OAuth URL is accepted from the frontend sign-in action.
use crate::{
    model::*,
    storage::{self, Settings, Storage},
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CliProvider {
    Codex,
    Claude,
}
impl CliProvider {
    pub fn provider(self) -> Provider {
        match self {
            Self::Codex => Provider::Codex,
            Self::Claude => Provider::Claude,
        }
    }
    pub fn name(self) -> &'static str {
        self.provider().name()
    }
    pub fn key(self) -> &'static str {
        self.provider().key()
    }
    pub fn environment(self) -> &'static str {
        match self {
            Self::Codex => "CODEX_HOME",
            Self::Claude => "CLAUDE_CONFIG_DIR",
        }
    }
    pub fn auth_file(self) -> &'static str {
        match self {
            Self::Codex => "auth.json",
            Self::Claude => ".credentials.json",
        }
    }
    pub fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Codex => &["login"],
            Self::Claude => &["auth", "login", "--claudeai"],
        }
    }
}
#[derive(Clone)]
pub struct Profile {
    pub provider: CliProvider,
    pub home: PathBuf,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    id: String,
}
fn safe_id(id: &str) -> bool {
    id.len() == 24 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
fn linked(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
fn safe_directory(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("The sign-in location must be absolute.");
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if linked(&m) || !m.is_dir() => {
                bail!("The sign-in location could not be accessed safely.")
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("The sign-in location is unavailable."),
        }
    }
    Ok(())
}
fn root(storage: &Storage, provider: CliProvider) -> PathBuf {
    storage.dir.join("sign-ins").join(provider.key())
}
pub fn is_managed(storage: &Storage, provider: CliProvider, home: &Path) -> bool {
    home.parent().is_some_and(|parent| {
        storage::path_key(parent) == storage::path_key(&root(storage, provider))
    }) && home
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(safe_id)
}
impl Profile {
    pub fn validate(&self, storage: &Storage) -> Result<()> {
        if !is_managed(storage, self.provider, &self.home) {
            bail!("No app-managed sign-in was selected.");
        }
        safe_directory(&self.home)?;
        if !self.home.is_dir() {
            bail!("The sign-in location is unavailable.");
        }
        Ok(())
    }
    pub fn prepare(storage: &Storage, provider: CliProvider, settings: &Settings) -> Result<Self> {
        let base = root(storage, provider);
        safe_directory(&base)?;
        fs::create_dir_all(&base).context("The sign-in location could not be created.")?;
        let pending = base.join("pending.json");
        if let Ok(meta) = fs::symlink_metadata(&pending) {
            if linked(&meta) || !meta.is_file() || meta.len() > 1024 {
                bail!("Pending sign-in metadata could not be read safely.");
            }
            let previous: Pending =
                storage::read_json(&pending).context("Pending sign-in metadata is invalid.")?;
            if !safe_id(&previous.id) {
                bail!("Pending sign-in metadata is invalid.");
            }
            let profile = Self {
                provider,
                home: base.join(previous.id),
            };
            profile.validate(storage)?;
            let homes = match provider {
                CliProvider::Codex => &settings.codex_homes,
                CliProvider::Claude => &settings.claude_homes,
            };
            if !homes.iter().any(|h| {
                storage::path_key(&storage::normalize(h, &storage.user_home))
                    == storage::path_key(&profile.home)
            }) {
                return Ok(profile);
            }
        }
        let count = fs::read_dir(&base)?
            .take(102)
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_str().is_some_and(safe_id))
            .count();
        if count >= 100 {
            bail!(
                "There are too many saved CLI sign-ins. Existing credentials were left untouched."
            );
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..16 {
            let id = hash(&format!(
                "{:?}-{}-{}",
                SystemTime::now(),
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let home = base.join(&id);
            match fs::create_dir(&home) {
                Ok(()) => {
                    storage::atomic_json(&pending, &Pending { id })?;
                    return Ok(Self { provider, home });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => bail!("The sign-in location could not be created."),
            }
        }
        bail!("A separate sign-in location could not be created.")
    }
    /// Metadata only: a successful CLI exit also supports Windows keyring-only sign-ins.
    pub fn local_marker(&self, storage: &Storage) -> Result<Option<String>> {
        self.validate(storage)?;
        let path = self.home.join(self.provider.auth_file());
        match fs::symlink_metadata(path) {
            Ok(m) if !linked(&m) && m.is_file() && m.len() <= 2 * 1024 * 1024 => {
                Ok(Some(hash(&format!("{}:{:?}", m.len(), m.modified().ok()))))
            }
            Ok(_) => bail!("The CLI sign-in could not be checked safely."),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => bail!("The CLI sign-in could not be checked."),
        }
    }
    pub fn commit(
        &self,
        storage: &Storage,
        current: &Settings,
        account: &AccountState,
    ) -> Result<Settings> {
        self.validate(storage)?;
        if account.account.provider != self.provider.provider()
            || account.status != Status::Ready
            || account.report.is_none()
            || account.account.sources.is_empty()
            || account
                .account
                .sources
                .iter()
                .any(|s| storage::path_key(&s.path) != storage::path_key(&self.home))
        {
            bail!("The CLI sign-in has not been verified.");
        }
        let mut next = current.clone();
        let homes = match self.provider {
            CliProvider::Codex => {
                next.codex_enabled = true;
                &mut next.codex_homes
            }
            CliProvider::Claude => {
                next.claude_enabled = true;
                &mut next.claude_homes
            }
        };
        // An empty legacy list means the default home was implicitly monitored.
        // Adding a managed home must not silently remove that existing sign-in.
        let previously_enabled = match self.provider {
            CliProvider::Codex => current.codex_enabled,
            CliProvider::Claude => current.claude_enabled,
        };
        if homes.is_empty() && previously_enabled {
            let default = match self.provider {
                CliProvider::Codex => std::env::var_os("CODEX_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| storage.user_home.join(".codex")),
                CliProvider::Claude => std::env::var_os("CLAUDE_CONFIG_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| storage.user_home.join(".claude")),
            };
            let standard = storage.user_home.join(match self.provider {
                CliProvider::Codex => ".codex",
                CliProvider::Claude => ".claude",
            });
            for default in [default, standard] {
                if default.is_absolute()
                    && default.is_dir()
                    && !homes.iter().any(|h| {
                        storage::path_key(&storage::normalize(h, &storage.user_home))
                            == storage::path_key(&default)
                    })
                {
                    homes.push(default.to_string_lossy().into_owned());
                }
            }
        }
        let key = storage::path_key(&self.home);
        if !homes
            .iter()
            .any(|h| storage::path_key(&storage::normalize(h, &storage.user_home)) == key)
        {
            homes.push(self.home.to_string_lossy().into_owned());
        }
        next.ignored_homes
            .retain(|h| storage::path_key(&storage::normalize(h, &storage.user_home)) != key);
        storage.save_settings(&next)?;
        // Never delete provider credentials. A stale pending marker is harmless:
        // prepare() recognizes a committed home and allocates the next account.
        let _ = fs::remove_file(root(storage, self.provider).join("pending.json"));
        Ok(next)
    }
}
/// Stale cosmetic preference saves must not erase newly connected managed homes.
pub fn preserve_managed(storage: &Storage, current: &Settings, next: &mut Settings) {
    for (provider, old, homes, enabled, was_enabled) in [
        (
            CliProvider::Codex,
            &current.codex_homes,
            &mut next.codex_homes,
            &mut next.codex_enabled,
            current.codex_enabled,
        ),
        (
            CliProvider::Claude,
            &current.claude_homes,
            &mut next.claude_homes,
            &mut next.claude_enabled,
            current.claude_enabled,
        ),
    ] {
        let stale = old.iter().any(|value| {
            let path = storage::normalize(value, &storage.user_home);
            is_managed(storage, provider, &path)
                && !homes.iter().any(|h| {
                    storage::path_key(&storage::normalize(h, &storage.user_home))
                        == storage::path_key(&path)
                })
        });
        if stale {
            // A draft from before Connect cannot erase either the new home or
            // the default/legacy sources made explicit in the same transaction.
            for value in old {
                if !homes.iter().any(|h| {
                    storage::path_key(&storage::normalize(h, &storage.user_home))
                        == storage::path_key(&storage::normalize(value, &storage.user_home))
                }) {
                    homes.push(value.clone());
                }
            }
            *enabled = was_enabled;
        }
    }
    // Disconnect is targeted; old cosmetic drafts must not undo its exclusions.
    for home in &current.ignored_homes {
        if !next.ignored_homes.iter().any(|h| {
            storage::path_key(&storage::normalize(h, &storage.user_home))
                == storage::path_key(&storage::normalize(home, &storage.user_home))
        }) {
            next.ignored_homes.push(home.clone());
        }
    }
}
pub fn disconnect(storage: &Storage, current: &Settings, account: &Account) -> Result<Settings> {
    if !matches!(account.provider, Provider::Codex | Provider::Claude) || account.sources.is_empty()
    {
        bail!("No CLI account was selected.");
    }
    let mut next = current.clone();
    let removed: std::collections::HashSet<_> = account
        .sources
        .iter()
        .map(|s| storage::path_key(&s.path))
        .collect();
    let homes = match account.provider {
        Provider::Codex => &mut next.codex_homes,
        _ => &mut next.claude_homes,
    };
    homes.retain(|h| {
        !removed.contains(&storage::path_key(&storage::normalize(
            h,
            &storage.user_home,
        )))
    });
    for source in &account.sources {
        if !next.ignored_homes.iter().any(|h| {
            storage::path_key(&storage::normalize(h, &storage.user_home))
                == storage::path_key(&source.path)
        }) {
            next.ignored_homes
                .push(source.path.to_string_lossy().into_owned());
        }
    }
    storage.save_settings(&next)?;
    Ok(next)
}
#[cfg(test)]
mod tests;
