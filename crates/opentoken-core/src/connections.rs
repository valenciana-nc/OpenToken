//! Native connections: Cursor app sessions, Cursor Enterprise and personal Copilot billing.
//! Secrets are held by an injected OS vault, never Settings/Snapshot/cache or log messages.
use crate::{
    model::*,
    storage::{Settings, Storage},
};
use anyhow::{bail, Result};
use chrono::{Datelike, Utc};
use futures_util::StreamExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc, time::Duration};

#[path = "cursor_personal.rs"]
mod cursor_personal;
#[path = "github_personal.rs"]
mod github_personal;
#[path = "grok_personal.rs"]
mod grok_personal;
#[path = "grok_renewal.rs"]
mod grok_renewal;
#[path = "session_http.rs"]
mod session_http;
pub use grok_personal::{config_home as grok_config_home, executable as find_grok_cli};
#[path = "local_providers.rs"]
mod local_providers;
pub use local_providers::UsageProbe as AntigravityUsageProbe;
pub(crate) use local_providers::{antigravity_cli_detected, profile as local_provider_profile};
#[path = "reported.rs"]
mod reported;
pub use github_personal::{config_home as github_config_home, executable as find_github_cli};
#[cfg(test)]
#[path = "connections_tests.rs"]
mod tests;

/// Secret-free outcome for deliberate native read-only diagnosis. Never persists a connection.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProbe {
    pub provider: Provider,
    pub source: ReportSource,
    pub quota_count: usize,
    pub percentage_count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntigravityClient {
    Desktop,
    Ide,
    Cli,
}
pub trait CredentialStore: Send + Sync {
    fn read(&self, id: &str) -> Result<Option<String>>;
    fn write(&self, id: &str, secret: &str) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}
pub struct UnavailableStore;
impl CredentialStore for UnavailableStore {
    fn read(&self, _: &str) -> Result<Option<String>> {
        bail!("Windows secure credential storage is unavailable.")
    }
    fn write(&self, _: &str, _: &str) -> Result<()> {
        bail!("Windows secure credential storage is unavailable.")
    }
    fn delete(&self, _: &str) -> Result<()> {
        bail!("Windows secure credential storage is unavailable.")
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    #[default]
    Api,
    CursorApp,
    GithubCli,
    GrokCli,
    GrokCliRenewal,
    DevinCli,
    DevinApp,
    AntigravityApp,
    AntigravityCli,
    AntigravityStored,
    AntigravityCliUsage,
    MuseCli,
    MuseDevice,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub provider: Provider,
    pub subject: String,
    pub login: String,
    pub scope: String,
    #[serde(default)]
    pub mode: ConnectionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}
impl Connection {
    fn new(provider: Provider, subject: String, login: String, scope: String) -> Self {
        let id = format!(
            "{}-{}",
            provider.key(),
            hash(&format!("{scope}\n{subject}"))
        );
        Self {
            id,
            provider,
            subject,
            login,
            scope,
            mode: ConnectionMode::Api,
            profile: None,
        }
    }
    pub fn account(&self, settings: &Settings) -> Account {
        Account {
            id: self.id.clone(),
            provider: self.provider,
            label: settings
                .aliases
                .get(&self.id)
                .cloned()
                .unwrap_or_else(|| self.login.clone()),
            email: (matches!(
                self.provider,
                Provider::Cursor | Provider::Grok | Provider::Antigravity | Provider::Muse
            ) && email(&self.login))
            .then(|| self.login.clone()),
            identity: Some(Identity {
                workspace: format!("{}:{}", self.provider.key(), self.scope),
                email: None,
                user: Some(self.subject.clone()),
            }),
            sources: vec![],
        }
    }
}
pub fn validate_connections(connections: &[Connection]) -> Result<()> {
    if connections.len() > 20 {
        bail!("At most 20 secure API connections can be configured.");
    }
    let mut ids = HashSet::new();
    let mut profiles = HashSet::new();
    for c in connections {
        let valid_scope = match c.mode {
            ConnectionMode::DevinCli
            | ConnectionMode::DevinApp
            | ConnectionMode::AntigravityApp
            | ConnectionMode::AntigravityCli
            | ConnectionMode::AntigravityStored
            | ConnectionMode::AntigravityCliUsage
            | ConnectionMode::MuseCli
            | ConnectionMode::MuseDevice => local_providers::valid_scope(c),
            ConnectionMode::CursorApp => {
                c.provider == Provider::Cursor
                    && c.profile.as_deref().is_some_and(|p| {
                        text(p, 2048)
                            && std::path::Path::new(p).is_absolute()
                            && c.scope
                                == format!(
                                    "app:{}",
                                    hash(&crate::storage::path_key(std::path::Path::new(p)))
                                )
                    })
            }
            ConnectionMode::GrokCli | ConnectionMode::GrokCliRenewal => {
                c.provider == Provider::Grok
                    && c.profile.as_deref().is_some_and(|p| {
                        text(p, 2048)
                            && std::path::Path::new(p).is_absolute()
                            && c.scope
                                == format!(
                                    "grok:{}",
                                    hash(&crate::storage::path_key(std::path::Path::new(p)))
                                )
                    })
            }
            ConnectionMode::GithubCli => {
                c.provider == Provider::Copilot
                    && c.subject.parse::<u64>().is_ok_and(|id| id > 0)
                    && c.profile.as_deref().is_some_and(|p| {
                        text(p, 2048)
                            && std::path::Path::new(p).is_absolute()
                            && c.scope
                                == format!(
                                    "gh:{}",
                                    hash(&crate::storage::path_key(std::path::Path::new(p)))
                                )
                    })
            }
            ConnectionMode::Api => {
                c.profile.is_none()
                    && match c.provider {
                        Provider::Cursor => {
                            c.scope.len() == 24 && c.scope.bytes().all(|b| b.is_ascii_hexdigit())
                        }
                        Provider::Copilot => c.scope == "personal",
                        provider if reported::supported(provider) => {
                            reported::valid_scope(provider, &c.scope, &c.subject)
                        }
                        _ => false,
                    }
            }
        };
        if !valid_scope
            || !text(&c.subject, 256)
            || !text(&c.login, 254)
            || c.id
                != Connection::new(
                    c.provider,
                    c.subject.clone(),
                    c.login.clone(),
                    c.scope.clone(),
                )
                .id
            || !ids.insert(&c.id)
            || (c.mode == ConnectionMode::CursorApp
                && c.profile.as_deref().is_some_and(|p| {
                    !profiles.insert(crate::storage::path_key(std::path::Path::new(p)))
                }))
            || (c.provider == Provider::Cursor
                && !email(&c.login)
                && !(c.mode == ConnectionMode::CursorApp && c.login == c.subject))
            || (c.provider == Provider::Copilot && !github_login(&c.login))
        {
            bail!("Invalid secure connection metadata.");
        }
    }
    Ok(())
}
// Intentionally no Debug or Serialize: a submitted credential must never be logged/cached.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectInput {
    pub provider: Provider,
    #[serde(default)]
    pub region: String,
    pub token: String,
    #[serde(default)]
    pub member_email: String,
}
impl Drop for ConnectInput {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.token);
    }
}
fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.trim() == s && !s.chars().any(char::is_control)
}
fn email(s: &str) -> bool {
    text(s, 254)
        && !s.chars().any(char::is_whitespace)
        && s.split('@').count() == 2
        && s.split('@').all(|p| !p.is_empty())
}
fn github_login(s: &str) -> bool {
    text(s, 39) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn invalid() -> Fault {
    Fault::new(
        FaultKind::Unreadable,
        "The provider returned incomplete or invalid usage data. No totals were inferred.",
    )
}
fn nonnegative(v: f64) -> Result<f64, Fault> {
    if v.is_finite() && (0.0..=1e15).contains(&v) {
        Ok(v)
    } else {
        Err(invalid())
    }
}
fn total(mut values: impl Iterator<Item = f64>) -> Result<f64, Fault> {
    values.try_fold(0.0, |sum, v| nonnegative(sum + nonnegative(v)?))
}

#[derive(Default)]
struct Gate {
    connect_after: Option<tokio::time::Instant>,
    read_after: Option<tokio::time::Instant>,
    attempts: std::collections::VecDeque<tokio::time::Instant>,
}
#[derive(Clone)]
pub struct Service {
    storage: Storage,
    vault: Arc<dyn CredentialStore>,
    http: reqwest::Client,
    cursor_base: String,
    github_base: String,
    personal: Arc<cursor_personal::Personal>,
    reported: reported::Reported,
    local: Arc<local_providers::Readers>,
    #[cfg(test)]
    github_test_token: Option<Arc<std::sync::Mutex<String>>>,
    grok_identity_base: String,
    grok_proxy_base: String,
    grok_sources: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    grok_renewal: Arc<grok_renewal::Renewal>,
    pub(crate) grok_owner: Arc<tokio::sync::Mutex<()>>,
    #[cfg(test)]
    grok_test_home: Option<std::path::PathBuf>,
    gates: Arc<std::sync::Mutex<std::collections::HashMap<Provider, Gate>>>,
}
impl Service {
    pub fn new(storage: Storage, vault: Arc<dyn CredentialStore>) -> Self {
        Self {
            local: Arc::new(local_providers::Readers::new(&storage)),
            personal: Arc::new(cursor_personal::Personal::new(&storage)),
            reported: Default::default(),
            #[cfg(test)]
            github_test_token: None,
            grok_identity_base: "https://auth.x.ai".into(),
            grok_proxy_base: "https://cli-chat-proxy.grok.com".into(),
            grok_sources: Default::default(),
            grok_renewal: Default::default(),
            grok_owner: Default::default(),
            #[cfg(test)]
            grok_test_home: None,
            storage,
            vault,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("OpenToken/1.0.0")
                .build()
                .expect("TLS client initialization"),
            cursor_base: "https://api.cursor.com".into(),
            github_base: "https://api.github.com".into(),
            gates: Default::default(),
        }
    }
    pub async fn connect(
        &self,
        settings: &Settings,
        input: ConnectInput,
    ) -> Result<(Settings, AccountState)> {
        if !matches!(input.provider, Provider::Cursor | Provider::Copilot)
            && !reported::supported(input.provider)
        {
            bail!("This provider does not support API-key connections.");
        }
        if !(16..=2048).contains(&input.token.len())
            || !input.token.bytes().all(|b| b.is_ascii_graphic())
            || input.token.contains(':')
        {
            bail!("Enter a provider-issued API key or token (16–2048 characters, without spaces).");
        }
        if input.provider == Provider::Cursor && !email(&input.member_email) {
            bail!("Enter the exact Cursor team member email to monitor.");
        }
        self.begin_verification(input.provider)?;
        let provider = input.provider;
        let result =
            tokio::time::timeout(Duration::from_secs(45), self.connect_inner(settings, input))
                .await
                .unwrap_or_else(|_| {
                    Err(anyhow::anyhow!(
                        "Provider verification timed out. No connection was saved."
                    ))
                });
        if let Err(error) = &result {
            if let Some(fault) = error.downcast_ref::<Fault>() {
                self.pause(provider, fault);
            }
        }
        result
    }
    pub(crate) fn verification_wait_secs(&self, provider: Provider) -> u64 {
        let gates = self.gates.lock().unwrap();
        let Some(gate) = gates.get(&provider) else {
            return 0;
        };
        let now = tokio::time::Instant::now();
        let mut after = gate.connect_after.into_iter().chain(gate.read_after).max();
        let active: Vec<_> = gate
            .attempts
            .iter()
            .filter(|t| now.saturating_duration_since(**t) < Duration::from_secs(3600))
            .copied()
            .collect();
        if active.len() >= 20 {
            let at = active[0] + Duration::from_secs(3600);
            after = Some(after.unwrap_or(at).max(at));
        }
        let delay = after
            .map(|at| at.saturating_duration_since(now))
            .unwrap_or_default();
        delay.as_secs() + u64::from(delay.subsec_nanos() != 0)
    }
    pub(crate) fn begin_verification(&self, provider: Provider) -> Result<()> {
        {
            let mut gates = self.gates.lock().unwrap();
            let now = tokio::time::Instant::now();
            let gate = gates.entry(provider).or_default();
            if gate
                .connect_after
                .into_iter()
                .chain(gate.read_after)
                .any(|at| at > now)
            {
                bail!("Please wait before reconnecting this provider. Provider backoff also applies to connection attempts.");
            }
            gate.attempts
                .retain(|at| now.saturating_duration_since(*at) < Duration::from_secs(3600));
            if gate.attempts.len() >= 20 {
                bail!("Connection verification is limited to 20 attempts per provider per hour. Please wait before trying again.");
            }
            gate.attempts.push_back(now);
            gate.connect_after = Some(now + Duration::from_secs(15));
        }
        Ok(())
    }
    pub(crate) fn pause(&self, provider: Provider, fault: &Fault) {
        if fault.kind == FaultKind::RateLimited {
            let until = tokio::time::Instant::now()
                + Duration::from_secs(fault.retry_after_secs.unwrap_or(60));
            let mut gates = self.gates.lock().unwrap();
            let gate = gates.entry(provider).or_default();
            gate.read_after = Some(gate.read_after.unwrap_or(until).max(until));
            gate.connect_after = Some(gate.connect_after.unwrap_or(until).max(until));
        }
    }
    async fn connect_inner(
        &self,
        settings: &Settings,
        input: ConnectInput,
    ) -> Result<(Settings, AccountState)> {
        let (connection, report) = match input.provider {
            Provider::Cursor => {
                if !email(&input.member_email) {
                    bail!("Enter the exact Cursor team member email to monitor.");
                }
                let member = self
                    .cursor_member(&input.token, &input.member_email)
                    .await?;
                // Keys identify a team, not necessarily their creator. A different key is a new scope,
                // so the same member in another team can never inherit this connection's cached usage.
                let c = Connection::new(
                    Provider::Cursor,
                    member.id,
                    member.email,
                    hash(&input.token),
                );
                let report = self.cursor(&c, &input.token).await?;
                (c, report)
            }
            Provider::Copilot => {
                let user = self.github_user(&input.token).await?;
                let c = Connection::new(
                    Provider::Copilot,
                    user.id.to_string(),
                    user.login,
                    "personal".into(),
                );
                let report = self.copilot(&c, &input.token).await?;
                (c, report)
            }
            provider if reported::supported(provider) => {
                let region = reported::region(provider, &input.region)?;
                let report = self.reported_usage(provider, &input.token, region).await?;
                let (subject, login, scope) = reported::key_scope(provider, &input.token, region)?;
                (Connection::new(provider, subject, login, scope), report)
            }
            _ => unreachable!(),
        };
        let mut next = settings.clone();
        next.connections.retain(|c| c.id != connection.id);
        next.connections.push(connection.clone());
        next.validate()?;
        let previous = self.vault.read(&connection.id)?;
        self.vault.write(&connection.id, &input.token)?;
        if self.storage.save_settings(&next).is_err() {
            self.restore_secret(&connection.id, previous.as_deref())?;
            bail!("The connection could not be saved. The previous credential was restored.");
        }
        Ok((
            next.clone(),
            AccountState::apply(connection.account(&next), Ok(report), None),
        ))
    }
    fn restore_secret(&self, id: &str, previous: Option<&str>) -> Result<()> {
        let result = if let Some(secret) = previous {
            self.vault.write(id, secret)
        } else {
            self.vault.delete(id)
        };
        result.map_err(|_| {
            anyhow::anyhow!(
                "Secure credential rollback failed. Review or remove this OpenToken connection."
            )
        })
    }
    pub fn disconnect(&self, settings: &Settings, id: &str) -> Result<Settings> {
        if !settings.connections.iter().any(|c| c.id == id) {
            bail!("This OpenToken connection no longer exists.");
        }
        let app_session = settings
            .connections
            .iter()
            .any(|c| c.id == id && c.mode != ConnectionMode::Api);
        let previous = if app_session {
            None
        } else {
            self.vault.read(id)?
        };
        if !app_session {
            self.vault.delete(id)?;
        }
        let mut next = settings.clone();
        next.connections.retain(|c| c.id != id);
        if self.storage.save_settings(&next).is_err() {
            if !app_session {
                self.restore_secret(id, previous.as_deref())?;
            }
            bail!("The connection could not be removed. Its previous credential was restored.");
        }
        Ok(next)
    }
    pub async fn fetch(&self, c: &Connection) -> Result<Report, Fault> {
        if let Some(at) = self
            .gates
            .lock()
            .unwrap()
            .get(&c.provider)
            .and_then(|g| g.read_after)
            .filter(|at| *at > tokio::time::Instant::now())
        {
            return Err(Fault {
                kind: FaultKind::RateLimited,
                message:
                    "The provider requested a pause. Its backoff also applies to other connections."
                        .into(),
                retry_after_secs: Some(
                    at.saturating_duration_since(tokio::time::Instant::now())
                        .as_secs()
                        + 1,
                ),
            });
        }
        if matches!(
            c.mode,
            ConnectionMode::DevinCli
                | ConnectionMode::DevinApp
                | ConnectionMode::AntigravityApp
                | ConnectionMode::AntigravityCli
                | ConnectionMode::AntigravityStored
                | ConnectionMode::AntigravityCliUsage
                | ConnectionMode::MuseCli
                | ConnectionMode::MuseDevice
        ) {
            let result =
                tokio::time::timeout(Duration::from_secs(45), self.fetch_local_provider(c))
                    .await
                    .unwrap_or_else(|_| Err(Fault::offline()));
            if let Err(fault) = &result {
                self.pause(c.provider, fault);
            }
            return result;
        }
        if matches!(
            c.mode,
            ConnectionMode::GrokCli | ConnectionMode::GrokCliRenewal
        ) {
            let result =
                tokio::time::timeout(Duration::from_secs(60), self.fetch_grok_monitored(c))
                    .await
                    .unwrap_or_else(|_| Err(Fault::offline()));
            if let Err(fault) = &result {
                self.pause(c.provider, fault);
            }
            return result;
        }
        if c.mode == ConnectionMode::GithubCli {
            let result =
                tokio::time::timeout(Duration::from_secs(45), self.fetch_github_personal(c))
                    .await
                    .unwrap_or_else(|_| Err(Fault::offline()));
            if let Err(fault) = &result {
                self.pause(c.provider, fault);
            }
            return result;
        }
        if c.mode == ConnectionMode::CursorApp {
            let result = tokio::time::timeout(Duration::from_secs(45), self.fetch_cursor_app(c))
                .await
                .unwrap_or_else(|_| Err(Fault::offline()));
            if let Err(fault) = &result {
                self.pause(c.provider, fault);
            }
            return result;
        }
        let token = zeroize::Zeroizing::new(
            self.vault
                .read(&c.id)
                .map_err(|_| {
                    Fault::new(
                        FaultKind::Unreadable,
                        "Windows could not read this OpenToken credential.",
                    )
                })?
                .ok_or_else(|| {
                    Fault::new(
                        FaultKind::NotSignedIn,
                        "The saved API credential is missing. Reconnect this provider.",
                    )
                })?,
        );
        let result = tokio::time::timeout(Duration::from_secs(45), async {
            match c.provider {
                Provider::Cursor => self.cursor(c, &token).await,
                Provider::Copilot => self.copilot(c, &token).await,
                provider if reported::supported(provider) => {
                    let (region, _) = c.scope.split_once(':').ok_or_else(invalid)?;
                    self.reported_usage(provider, &token, region).await
                }
                _ => Err(invalid()),
            }
        })
        .await
        .unwrap_or_else(|_| Err(Fault::offline()));
        if let Err(fault) = &result {
            self.pause(c.provider, fault);
        }
        result
    }
    pub async fn inspect_existing_source(
        &self,
        provider: Provider,
        consent: bool,
    ) -> std::result::Result<SourceProbe, Fault> {
        if !consent
            || !matches!(
                provider,
                Provider::Cursor
                    | Provider::Grok
                    | Provider::Antigravity
                    | Provider::Devin
                    | Provider::Muse
                    | Provider::Claude
            )
        {
            return Err(Fault::new(FaultKind::PermissionDenied,"Explicitly allow a supported existing-source diagnosis. No login, key issuance or credential access was started."));
        }
        self.begin_verification(provider)
            .map_err(|e| e.downcast_ref::<Fault>().cloned().unwrap_or_else(invalid))?;
        let result = tokio::time::timeout(Duration::from_secs(45), async {
            match provider {
                Provider::Cursor => self.cursor_app_read(None).await.map(|(_, r)| r),
                Provider::Grok => self.grok_existing_reading().await.map(|(_, r)| r),
                Provider::Claude => {
                    let mut settings = Settings { claude_enabled: true, ..Settings::default() };
                    settings.claude_homes = crate::monitoring::cli_homes(&self.storage,&settings,Provider::Claude).into_iter().map(|p|p.to_string_lossy().into_owned()).collect();
                    let adapter = crate::claude::ClaudeAdapter { http: self.http.clone(), user_home: self.storage.user_home.clone() };
                    let mut result = Err(Fault::new(FaultKind::NotSignedIn,"No supported existing Claude CLI source was found. No login or token renewal was started."));
                    for account in adapter.monitored_accounts(&settings) { result=adapter.fetch_scoped(&account,false).await.map(|(r,_)|r);if result.is_ok(){break;} }
                    result
                }
                _ => self.probe_local(provider).await,
            }
        })
        .await
        .unwrap_or_else(|_| Err(Fault::offline()));
        if let Err(f) = &result {
            self.pause(provider, f);
        }
        result.map(|r| SourceProbe {
            provider,
            source: r.source,
            quota_count: r.quotas.len(),
            percentage_count: r.quotas.iter().filter(|q| q.percent.is_some()).count(),
        })
    }
    async fn json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        optional: bool,
    ) -> Result<Option<T>, Fault> {
        let response = request.send().await.map_err(|_| Fault::offline())?;
        self.decode_json_response(response, optional).await
    }
    async fn decode_json_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
        optional: bool,
    ) -> Result<Option<T>, Fault> {
        let status = response.status();
        if optional && status == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| {
                    s.parse::<u64>().ok().or_else(|| {
                        chrono::DateTime::parse_from_rfc2822(s)
                            .ok()
                            .map(|d| (d.timestamp() - Utc::now().timestamp()).max(1) as u64)
                    })
                });
            if status == 429
                || (status.is_server_error() && retry.is_some())
                || (status == 403
                    && (retry.is_some()
                        || response
                            .headers()
                            .get("x-ratelimit-remaining")
                            .is_some_and(|v| v == "0")))
            {
                let reset = response
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|d| (d - Utc::now().timestamp()).max(1) as u64);
                return Err(Fault { kind: FaultKind::RateLimited, message: "The provider requested a pause. Automatic and manual reads will respect its backoff.".into(), retry_after_secs: Some(retry.or(reset).unwrap_or(60).min(32*86400)) });
            }
            return Err(match status.as_u16() {
                401 => Fault::new(FaultKind::NeedsRelogin,"The API key or token was rejected. Reconnect with a valid credential."),
                403 | 404 => Fault::new(FaultKind::PermissionDenied,"The provider refused this read-only usage request or does not expose it for this session. A working login does not establish quota access."),
                _ => Fault::offline(),
            });
        }
        const MAX: usize = 2 * 1024 * 1024;
        if response.content_length().is_some_and(|n| n > MAX as u64) {
            return Err(invalid());
        }
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| Fault::offline())?;
            if bytes.len().saturating_add(chunk.len()) > MAX {
                return Err(invalid());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| invalid())
    }
    fn github_request(&self, path: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .get(format!("{}{path}", self.github_base))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
    }
    async fn github_user(&self, token: &str) -> Result<GithubUser, Fault> {
        let u: GithubUser = self
            .json(self.github_request("/user", token), false)
            .await?
            .ok_or_else(invalid)?;
        if u.id == 0 || !github_login(&u.login) {
            return Err(invalid());
        }
        Ok(u)
    }
    async fn copilot(&self, c: &Connection, token: &str) -> Result<Report, Fault> {
        let user = self.github_user(token).await?;
        if user.id.to_string() != c.subject {
            return Err(Fault::changed());
        }
        let now = Utc::now();
        let mut quotas = vec![];
        let mut supported = false;
        for (kind, unit, label) in [
            ("premium_request", "requests", "Premium requests"),
            ("ai_credit", "ai-credits", "AI credits"),
        ] {
            let report: Option<Billing> = self
                .json(
                    self.github_request(
                        &format!("/users/{}/settings/billing/{kind}/usage", user.login),
                        token,
                    )
                    .query(&[
                        ("year", now.year().to_string()),
                        ("month", now.month().to_string()),
                    ]),
                    true,
                )
                .await?;
            let Some(report) = report else {
                continue;
            };
            supported = true;
            if !report.user.eq_ignore_ascii_case(&user.login)
                || report.time_period.year != now.year()
                || report.time_period.month.is_some_and(|m| m != now.month())
                || report.usage_items.len() > 10000
            {
                return Err(Fault::changed());
            }
            let rows: Vec<_> = report
                .usage_items
                .iter()
                .filter(|r| {
                    r.product.eq_ignore_ascii_case("Copilot")
                        || r.product.eq_ignore_ascii_case("Copilot AI Credits")
                })
                .collect();
            if rows.is_empty() {
                continue;
            } // Empty personal reports do NOT prove an individual quota or zero usage.
            if rows
                .iter()
                .any(|r| r.unit_type != unit && !(kind == "ai_credit" && r.unit_type == "credits"))
            {
                return Err(invalid());
            }
            quotas.push(Quota::value(
                kind,
                &format!("This month · {label} used (reported)"),
                total(rows.iter().map(|r| r.gross_quantity))?,
                if kind == "ai_credit" {
                    "AI credits"
                } else {
                    "requests"
                },
            ));
            quotas.push(Quota::value(
                &format!("{kind}_net"),
                &format!("This month · {label} net usage amount"),
                total(rows.iter().map(|r| r.net_amount))?,
                "USD",
            ));
        }
        if !supported {
            return Err(Fault::new(FaultKind::PermissionDenied,"Personal Copilot billing reports are unavailable. Use a fine-grained token with Plan: read; organization-paid usage is not included."));
        }
        let after = self.github_user(token).await?;
        if after.id != user.id || !after.login.eq_ignore_ascii_case(&user.login) {
            return Err(Fault::changed());
        }
        Ok(Report {
            plan: Some(format!(
                "Personal billing · {}-{:02}",
                now.year(),
                now.month()
            )),
            quotas,
            observed_at: Utc::now(),
            source: ReportSource::Provider,
            verified_at: None,
        })
    }
    async fn cursor_member(&self, token: &str, member_email: &str) -> Result<Member, Fault> {
        let members: Members = self
            .json(
                self.http
                    .get(format!("{}/teams/members", self.cursor_base))
                    .basic_auth(token, Some("")),
                false,
            )
            .await?
            .ok_or_else(invalid)?;
        if members.team_members.len() > 5000 {
            return Err(invalid());
        }
        let found: Vec<_> = members
            .team_members
            .into_iter()
            .filter(|m| !m.is_removed && m.email.eq_ignore_ascii_case(member_email))
            .collect();
        if found.len() != 1 {
            return Err(Fault::new(FaultKind::IdentityChanged,"The exact selected email is not a unique active member of this Cursor team. Check the member email and admin key."));
        }
        let m = found.into_iter().next().unwrap();
        if !text(&m.id, 256) || !email(&m.email) {
            return Err(invalid());
        }
        Ok(m)
    }
    async fn cursor(&self, c: &Connection, token: &str) -> Result<Report, Fault> {
        if hash(token) != c.scope {
            return Err(Fault::changed());
        }
        let member = self.cursor_member(token, &c.login).await?;
        if member.id != c.subject {
            return Err(Fault::changed());
        }
        let mut found = None;
        let mut page = 1;
        loop {
            let spend: Spend = self
                .json(
                    self.http
                        .post(format!("{}/teams/spend", self.cursor_base))
                        .basic_auth(token, Some(""))
                        .json(
                            &serde_json::json!({"searchTerm":c.login,"page":page,"pageSize":100}),
                        ),
                    false,
                )
                .await?
                .ok_or_else(invalid)?;
            if spend.total_pages > 10 || spend.team_member_spend.len() > 100 {
                return Err(invalid());
            }
            for row in spend.team_member_spend {
                if row.email.eq_ignore_ascii_case(&c.login) {
                    if row.user_id != c.subject || found.is_some() {
                        return Err(Fault::changed());
                    }
                    found = Some(row);
                }
            }
            if page >= spend.total_pages {
                break;
            }
            page += 1;
        }
        let row = found.ok_or_else(|| Fault::new(FaultKind::Unreadable,"Cursor returned no spending row for this member. A missing record is not zero usage."))?;
        let after = self.cursor_member(token, &c.login).await?;
        if after.id != member.id {
            return Err(Fault::changed());
        }
        let demand = nonnegative(row.spend_cents)? / 100.0;
        let overall = nonnegative(row.overall_spend_cents)? / 100.0;
        let mut quotas = vec![
            Quota::value(
                "cursor_on_demand",
                "Billing cycle · On-demand usage (reported)",
                demand,
                "USD",
            ),
            Quota::value(
                "cursor_overall",
                "Billing cycle · Usage incl. included value",
                overall,
                "USD",
            ),
        ];
        if let Some(limit) = row.effective_per_user_limit_dollars {
            let limit = nonnegative(limit)?;
            if limit > 0.0 {
                quotas.insert(
                    0,
                    Quota::window(
                        "on_demand_budget",
                        "On-demand budget",
                        (demand / limit * 100.0).min(100.0),
                        None,
                    ),
                );
                quotas.push(Quota::value(
                    "cursor_budget_limit",
                    "Enforced on-demand budget",
                    limit,
                    "USD",
                ));
            }
        }
        Ok(Report {
            plan: Some("Enterprise · selected team member".into()),
            quotas,
            observed_at: Utc::now(),
            source: ReportSource::Provider,
            verified_at: None,
        })
    }
}
#[derive(Deserialize)]
struct GithubUser {
    id: u64,
    login: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Billing {
    time_period: BillingPeriod,
    user: String,
    usage_items: Vec<BillingItem>,
}
#[derive(Deserialize)]
struct BillingPeriod {
    year: i32,
    month: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingItem {
    product: String,
    unit_type: String,
    gross_quantity: f64,
    net_amount: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Members {
    team_members: Vec<Member>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Member {
    id: String,
    email: String,
    is_removed: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spend {
    team_member_spend: Vec<MemberSpend>,
    total_pages: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberSpend {
    user_id: String,
    email: String,
    spend_cents: f64,
    overall_spend_cents: f64,
    effective_per_user_limit_dollars: Option<f64>,
}
