use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Codex,
    Claude,
    Cursor,
    Copilot,
    Zai,
    Deepseek,
    Kimi,
    Opencode,
    Grok,
    Antigravity,
    Muse,
    Devin,
}
impl Provider {
    pub const ALL: [Self; 12] = [
        Self::Codex,
        Self::Claude,
        Self::Antigravity,
        Self::Grok,
        Self::Muse,
        Self::Cursor,
        Self::Copilot,
        Self::Devin,
        Self::Zai,
        Self::Deepseek,
        Self::Kimi,
        Self::Opencode,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
            Self::Copilot => "Copilot",
            Self::Zai => "Z.ai",
            Self::Deepseek => "DeepSeek",
            Self::Kimi => "Kimi",
            Self::Opencode => "OpenCode",
            Self::Grok => "Grok",
            Self::Antigravity => "Antigravity",
            Self::Muse => "Muse Code",
            Self::Devin => "Devin",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Copilot => "copilot",
            Self::Zai => "zai",
            Self::Deepseek => "deepseek",
            Self::Kimi => "kimi",
            Self::Opencode => "opencode",
            Self::Grok => "grok",
            Self::Antigravity => "antigravity",
            Self::Muse => "muse",
            Self::Devin => "devin",
        }
    }
}

pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..24].to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub workspace: String,
    pub email: Option<String>,
    pub user: Option<String>,
}
impl Identity {
    pub fn key(&self) -> String {
        format!(
            "{}\n{}",
            self.workspace.trim().to_lowercase(),
            self.email
                .as_deref()
                .or(self.user.as_deref())
                .unwrap_or("")
                .trim()
                .to_lowercase()
        )
    }
    pub fn id(&self) -> String {
        format!("codex-{}", hash(&self.key()))
    }
    pub fn matches(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    File,
    Keyring,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub path: PathBuf,
    pub store: Store,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub provider: Provider,
    pub label: String,
    pub email: Option<String>,
    pub identity: Option<Identity>,
    pub sources: Vec<Source>,
}

impl Account {
    pub fn same_scope(&self, other: &Self) -> bool {
        self.id == other.id
            && self.provider == other.provider
            && match (&self.identity, &other.identity) {
                (Some(a), Some(b)) => a.matches(b),
                (None, None) => true,
                _ => false,
            }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub id: String,
    pub label: String,
    pub percent: Option<f64>,
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub resets_at: Option<DateTime<Utc>>,
}
impl Quota {
    pub fn window(id: &str, label: &str, percent: f64, resets_at: Option<DateTime<Utc>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            percent: Some(percent.clamp(0.0, 100.0)),
            value: None,
            unit: None,
            resets_at,
        }
    }
    pub fn value(id: &str, label: &str, value: f64, unit: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            percent: None,
            value: Some(value),
            unit: Some(unit.into()),
            resets_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportSource {
    #[default]
    Provider,
    CliLog,
    CursorDashboard,
    CopilotInternal,
    GrokCli,
    GrokCliRenewal,
    DevinCli,
    DevinApp,
    AntigravityLocal,
    AntigravityCli,
    AntigravityStored,
    AntigravityCliUsage,
    MuseCli,
    MuseDevice,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub plan: Option<String>,
    pub quotas: Vec<Quota>,
    pub observed_at: DateTime<Utc>,
    #[serde(default)]
    pub source: ReportSource,
    #[serde(default)]
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    NotSignedIn,
    NeedsRelogin,
    IdentityChanged,
    PermissionDenied,
    ToolMissing,
    Offline,
    RateLimited,
    Unreadable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fault {
    pub kind: FaultKind,
    pub message: String,
    pub retry_after_secs: Option<u64>,
}
impl Fault {
    pub fn new(kind: FaultKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retry_after_secs: None,
        }
    }
    pub fn offline() -> Self {
        Self::new(
            FaultKind::Offline,
            "Usage is temporarily unavailable. Try refreshing again.",
        )
    }
    pub fn changed() -> Self {
        Self::new(
            FaultKind::IdentityChanged,
            "The login changed. Rescanning accounts…",
        )
    }
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Fault {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Ready,
    Stale,
    Faulted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub account: Account,
    pub status: Status,
    pub report: Option<Report>,
    pub fault: Option<Fault>,
}
impl AccountState {
    pub fn apply(account: Account, result: Result<Report, Fault>, previous: Option<&Self>) -> Self {
        match result {
            Ok(report) => Self {
                account,
                status: Status::Ready,
                report: Some(report),
                fault: None,
            },
            Err(fault) => {
                // Identity failures must NEVER inherit cached usage, even under the same folder alias.
                let report = if matches!(
                    fault.kind,
                    FaultKind::IdentityChanged
                        | FaultKind::NotSignedIn
                        | FaultKind::NeedsRelogin
                        | FaultKind::PermissionDenied
                ) {
                    None
                } else {
                    previous
                        .filter(|p| {
                            p.report.as_ref().is_none_or(|r| {
                                !matches!(
                                    r.source,
                                    ReportSource::AntigravityCliUsage
                                        | ReportSource::GrokCliRenewal
                                )
                            })
                        })
                        .filter(|p| p.account.same_scope(&account))
                        .and_then(|p| p.report.clone())
                };
                Self {
                    account,
                    status: if report.is_some() {
                        Status::Stale
                    } else {
                        Status::Faulted
                    },
                    report,
                    fault: Some(fault),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    pub path: PathBuf,
    pub status: String,
    pub message: String,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub refreshing: bool,
    pub scanned_at: Option<DateTime<Utc>>,
    pub accounts: Vec<AccountState>,
    pub folders: Vec<FolderStatus>,
    pub api_error: Option<String>,
}
