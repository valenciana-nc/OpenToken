//! Discovery-first monitoring. Discovery inspects known paths' metadata only;
//! connecting existing sources never invokes login, installs software or renews credentials.
use crate::{
    model::*,
    storage::{normalize, path_key, Settings, Storage},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuotaReader {
    Cli,
    Desktop,
    ApiKey,
    ExistingSession,
    NotImplemented,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub provider: Provider,
    pub quota: QuotaReader,
    pub local_history: bool,
    pub balance_only: bool,
    pub note: &'static str,
}
pub fn capabilities() -> Vec<Capability> {
    Provider::ALL.into_iter().map(|provider|{let(quota,local_history,balance_only,note)=match provider{
Provider::Codex=>(QuotaReader::Cli,true,false,"Existing Codex account/rate-limit reads; local tokens are separate."),
Provider::Claude=>(QuotaReader::Cli,true,false,"Existing Claude subscription session; no new login or token renewal during connection."),
Provider::Cursor=>(QuotaReader::Desktop,false,false,"Read-only existing Cursor desktop session; unofficial dashboard."),
Provider::Copilot=>(QuotaReader::ExistingSession,false,false,"Existing GitHub CLI credentials; quota acceptance is verified, not assumed from login."),
Provider::Grok=>(QuotaReader::ExistingSession,true,false,"Existing official Grok CLI session; private credit/budget API."),
Provider::Kimi=>(QuotaReader::ApiKey,true,false,"Kimi CLI tokens are device/client scoped; Code quota separately requires its API key."),
Provider::Zai|Provider::Opencode=>(QuotaReader::ApiKey,false,false,"Provider-reported API-key quota, not invented account identity or local token history."),
Provider::Deepseek=>(QuotaReader::ApiKey,false,true,"Official API balance only; no Chat subscription percentage."),
Provider::Muse=>(QuotaReader::Cli,true,false,"Official MSP last-observed subscription windows. A fresh host may have no observation; no prompt or minting fallback. Separate key-endpoint mode needs explicit native confirmation, may issue credentials, and repeats until disconnected. Local tokens are independent."),
Provider::Antigravity=>(QuotaReader::Desktop,false,false,"Owned app/IDE quota or separately approved official agy /usage CLI-profile monitoring. CLI-profile readings do not identify a human account. No borrowed OAuth, token copy or model prompt."),
Provider::Devin=>(QuotaReader::ExistingSession,false,false,"Existing Devin CLI credential or Devin desktop authentication record; bounded read-only source access and private status RPC. No database copy or browser extraction."),
};Capability{provider,quota,local_history,balance_only,note}}).collect()
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    #[serde(flatten)]
    pub capability: Capability,
    pub session_detected: bool,
    pub history_detected: bool,
    pub monitored_accounts: usize,
    pub history_enabled: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub revision: u64,
    pub sources: Vec<SourceStatus>,
}
/// Known native sources only. No file contents, secrets, processes or browser profiles are read.
pub fn inventory(
    storage: &Storage,
    settings: &Settings,
    snapshot: &Snapshot,
    revision: u64,
) -> Inventory {
    let sources = capabilities()
        .into_iter()
        .map(|capability| {
            let p = capability.provider;
            let homes = cli_homes(storage, settings, p);
            let session_detected = match p {
                Provider::Codex | Provider::Claude => homes.iter().any(|h| {
                    safe_file(&h.join(if p == Provider::Codex {
                        "auth.json"
                    } else {
                        ".credentials.json"
                    }))
                }),
                Provider::Cursor => std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .is_some_and(|p| safe_file(&p.join("Cursor/User/globalStorage/state.vscdb"))),
                Provider::Grok => {
                    safe_file(&crate::connections::grok_config_home(storage).join("auth.json"))
                }
                Provider::Copilot => {
                    safe_file(&crate::connections::github_config_home(storage).join("hosts.yml"))
                }
                Provider::Devin | Provider::Muse => {
                    safe_file(&crate::connections::local_provider_profile(storage, p))
                }
                Provider::Antigravity => {
                    safe_dir(&crate::connections::local_provider_profile(storage, p))
                        || crate::connections::antigravity_cli_detected(storage)
                }
                _ => false,
            };
            let history_detected = match p {
                Provider::Codex => homes.iter().any(|h| {
                    safe_dir(&h.join("sessions")) || safe_dir(&h.join("archived_sessions"))
                }),
                Provider::Claude => homes.iter().any(|h| safe_dir(&h.join("projects"))),
                Provider::Muse | Provider::Grok | Provider::Kimi => {
                    history_roots(storage, p).iter().any(|h| {
                        safe_dir(&if p == Provider::Muse {
                            h.clone()
                        } else {
                            h.join("sessions")
                        })
                    })
                }
                _ => false,
            };
            let monitored_accounts = snapshot
                .accounts
                .iter()
                .filter(|s| s.account.provider == p)
                .count();
            let history_enabled = history_enabled(settings, p);
            SourceStatus {
                capability,
                session_detected,
                history_detected,
                monitored_accounts,
                history_enabled,
            }
        })
        .collect();
    Inventory { revision, sources }
}
fn linked(meta: &std::fs::Metadata) -> bool {
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
fn safe(path: &Path, file: bool) -> bool {
    path.is_absolute()
        && path.ancestors().all(|p| {
            std::fs::symlink_metadata(p).is_ok_and(|m| {
                !linked(&m)
                    && (if p == path && file {
                        m.is_file()
                    } else {
                        m.is_dir()
                    })
            })
        })
}
pub(crate) fn safe_file(p: &Path) -> bool {
    safe(p, true)
}
fn safe_dir(p: &Path) -> bool {
    safe(p, false)
}
pub fn cli_homes(storage: &Storage, settings: &Settings, provider: Provider) -> Vec<PathBuf> {
    let (var, default, configured) = match provider {
        Provider::Codex => ("CODEX_HOME", ".codex", &settings.codex_homes),
        Provider::Claude => ("CLAUDE_CONFIG_DIR", ".claude", &settings.claude_homes),
        _ => return vec![],
    };
    let mut homes: Vec<_> = configured
        .iter()
        .map(|p| normalize(p, &storage.user_home))
        .collect();
    if let Some(home) = std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        homes.push(home);
    }
    homes.push(storage.user_home.join(default));
    let ignored: BTreeSet<_> = settings
        .ignored_homes
        .iter()
        .map(|h| path_key(&normalize(h, &storage.user_home)))
        .collect();
    let mut seen = BTreeSet::new();
    homes
        .into_iter()
        .filter(|p| safe_dir(p) && !ignored.contains(&path_key(p)) && seen.insert(path_key(p)))
        .take(100)
        .collect()
}
pub fn history_enabled(settings: &Settings, provider: Provider) -> bool {
    settings
        .history_sources
        .get(&provider)
        .copied()
        .unwrap_or(match provider {
            Provider::Codex => settings.codex_enabled,
            Provider::Claude => settings.claude_enabled,
            _ => false,
        })
}
pub fn history_roots(storage: &Storage, provider: Provider) -> Vec<PathBuf> {
    match provider {
        Provider::Muse => vec![muse_history(storage)],
        Provider::Grok => vec![crate::connections::grok_config_home(storage)],
        Provider::Kimi => {
            if let Some(paths) = std::env::var_os("KIMI_DATA_DIR") {
                paths
                    .to_string_lossy()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .take(20)
                    .collect()
            } else {
                vec![
                    storage.user_home.join(".kimi"),
                    storage.user_home.join(".kimi-code"),
                ]
            }
        }
        _ => vec![],
    }
}
pub fn muse_history(storage: &Storage) -> PathBuf {
    std::env::var_os("MUSE_SESSIONS_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| storage.user_home.join(".local/share"))
                .join("muse/sessions")
        })
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub provider: Provider,
    pub settings: Settings,
    pub connected: usize,
    pub history_enabled: bool,
}
/// Pure target planning; credential verification/persistence is owned by the serialized engine.
pub fn enable_history(settings: &Settings, provider: Provider) -> anyhow::Result<Settings> {
    if !capabilities()
        .iter()
        .any(|c| c.provider == provider && c.local_history)
    {
        anyhow::bail!("A measured local-history reader is not implemented for this provider.");
    }
    let mut next = settings.clone();
    next.history_sources.insert(provider, true);
    Ok(next)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_twelve_have_explicit_separate_capabilities() {
        let c = capabilities();
        assert_eq!(c.len(), 12);
        assert_eq!(
            c.iter()
                .map(|c| c.provider.key())
                .collect::<BTreeSet<_>>()
                .len(),
            12
        );
        let deep = c.iter().find(|c| c.provider == Provider::Deepseek).unwrap();
        assert!(deep.balance_only);
        assert!(!deep.local_history);
        let muse = c.iter().find(|c| c.provider == Provider::Muse).unwrap();
        assert!(muse.local_history);
        assert_eq!(muse.quota, QuotaReader::Cli);
        assert!(muse.note.contains("fresh host may have no observation"));
        assert_eq!(
            c.iter()
                .find(|c| c.provider == Provider::Antigravity)
                .unwrap()
                .quota,
            QuotaReader::Desktop
        );
        assert_eq!(
            c.iter()
                .find(|c| c.provider == Provider::Devin)
                .unwrap()
                .quota,
            QuotaReader::ExistingSession
        );
    }
    #[test]
    fn metadata_discovery_does_not_parse_credentials_or_modify_them() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".claude/projects")).unwrap();
        std::fs::write(
            home.join(".claude/.credentials.json"),
            "this is deliberately not parseable credentials",
        )
        .unwrap();
        let storage = Storage {
            dir: temp.path().join("app"),
            user_home: home.clone(),
        };
        let before = std::fs::read(home.join(".claude/.credentials.json")).unwrap();
        let i = inventory(&storage, &Settings::default(), &Snapshot::default(), 1);
        let c = i
            .sources
            .iter()
            .find(|s| s.capability.provider == Provider::Claude)
            .unwrap();
        assert!(c.session_detected && c.history_detected);
        assert_eq!(c.monitored_accounts, 0);
        assert!(!c.history_enabled);
        assert!(!storage.dir.exists());
        assert_eq!(
            before,
            std::fs::read(home.join(".claude/.credentials.json")).unwrap()
        );
    }
    #[test]
    fn disconnect_exclusions_are_respected_by_existing_source_discovery() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join(".claude")).unwrap();
        let storage = Storage {
            dir: temp.path().join("app"),
            user_home: temp.path().into(),
        };
        let s = Settings {
            ignored_homes: vec![temp.path().join(".claude").to_string_lossy().into_owned()],
            ..Default::default()
        };
        assert!(!cli_homes(&storage, &s, Provider::Claude).contains(&temp.path().join(".claude")));
    }
}
