//! Original existing-session readers. No login, borrowed OAuth registration or key minting.
use super::*;
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;
#[path = "local_providers/antigravity.rs"]
mod antigravity;
#[path = "local_providers/antigravity_cli.rs"]
mod antigravity_cli;
#[path = "local_providers/antigravity_stored.rs"]
mod antigravity_stored;
#[path = "local_providers/antigravity_usage_probe.rs"]
mod antigravity_usage_probe;
pub use antigravity_usage_probe::UsageProbe;
#[cfg(test)]
#[path = "local_providers/compatibility_tests.rs"]
mod compatibility_tests;
#[path = "local_providers/devin_app.rs"]
mod devin_app;
#[path = "local_providers/muse.rs"]
mod muse;
#[path = "local_providers/muse_device.rs"]
mod muse_device;
#[cfg(test)]
#[path = "local_providers/tests.rs"]
mod tests;
#[cfg(windows)]
#[path = "local_providers/windows.rs"]
mod windows;

pub(super) struct Readers {
    pub(super) devin_base: String,
    muse_base: String,
    google_identity_url: String,
    google_quota_url: String,
    google_context_url: String,
    desktop: Arc<dyn antigravity::Desktop>,
    credential: Arc<dyn antigravity_cli::SessionStore>,
    scope_watch: std::sync::Mutex<Option<tokio::time::Instant>>,
    stored_marks: std::sync::Mutex<std::collections::HashMap<String, String>>,
    #[cfg(test)]
    pub(super) muse_executable: Option<PathBuf>,
}
impl Readers {
    pub(super) fn new(storage: &Storage) -> Self {
        let _ = storage; // The non-Windows fallback has no process locator.
        Self {
            devin_base: "https://server.codeium.com".into(),
            muse_base: "https://api.meta.ai".into(),
            google_identity_url: "https://openidconnect.googleapis.com/v1/userinfo".into(),
            google_context_url: "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist"
                .into(),
            google_quota_url:
                "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary"
                    .into(),
            #[cfg(windows)]
            desktop: Arc::new(windows::Desktop::new(storage)),
            #[cfg(not(windows))]
            desktop: Arc::new(antigravity::Unavailable),
            credential: Arc::new(antigravity_cli::NativeStore {
                allowed: storage.user_home == Storage::system().user_home,
            }),
            scope_watch: std::sync::Mutex::new(None),
            stored_marks: Default::default(),
            #[cfg(test)]
            muse_executable: None,
        }
    }
}
fn roaming(storage: &Storage) -> PathBuf {
    if storage.user_home != Storage::system().user_home {
        return storage.user_home.join("AppData/Roaming");
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| storage.user_home.join("AppData/Roaming"))
}
pub(crate) fn antigravity_cli_detected(storage: &Storage) -> bool {
    antigravity_cli::profile(storage).is_some()
}
pub(crate) fn profile(storage: &Storage, provider: Provider) -> PathBuf {
    let roaming = roaming(storage);
    match provider {
        Provider::Devin => {
            let cli = roaming.join("devin/credentials.toml");
            let desktop = roaming.join("Devin/User/globalStorage/state.vscdb");
            if !cli.exists() && crate::monitoring::safe_file(&desktop) {
                desktop
            } else {
                cli
            }
        }
        Provider::Antigravity => {
            let first = roaming.join("Antigravity");
            if !first.exists() && roaming.join("Antigravity IDE").is_dir() {
                roaming.join("Antigravity IDE")
            } else {
                first
            }
        }
        Provider::Muse if storage.user_home != Storage::system().user_home => {
            storage.user_home.join(".config/muse/auth.json")
        }
        // An explicit relative auth override must fail safe, never select another default login.
        Provider::Muse => std::env::var_os("MUSE_AUTH_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .unwrap_or_else(|| storage.user_home.join(".config"))
                    .join("muse/auth.json")
            }),
        _ => storage.user_home.clone(),
    }
}
fn scope(provider: Provider, path: &Path) -> String {
    format!(
        "local:{}:{}",
        provider.key(),
        hash(&crate::storage::path_key(path))
    )
}
pub(super) fn valid_scope(c: &Connection) -> bool {
    let expected = match c.mode {
        ConnectionMode::DevinCli | ConnectionMode::DevinApp => Provider::Devin,
        ConnectionMode::AntigravityApp
        | ConnectionMode::AntigravityCli
        | ConnectionMode::AntigravityStored
        | ConnectionMode::AntigravityCliUsage => Provider::Antigravity,
        ConnectionMode::MuseCli | ConnectionMode::MuseDevice => Provider::Muse,
        _ => return false,
    };
    c.provider == expected
        && c.profile.as_deref().is_some_and(|s| {
            text(s, 2048) && Path::new(s).is_absolute() && c.scope == scope(expected, Path::new(s))
        })
}
fn denied(message: &str) -> Fault {
    Fault::new(FaultKind::PermissionDenied, message)
}
fn missing(provider: Provider) -> Fault {
    Fault::new(FaultKind::NotSignedIn,format!("No usable existing {} source was found. Open the official app/CLI and sign in there if needed. No login was started.",provider.name()))
}
fn raw_file(path: &Path) -> Result<Zeroizing<Vec<u8>>, Fault> {
    if !crate::monitoring::safe_file(path) {
        return Err(Fault::new(FaultKind::Unreadable,"The selected provider source is absent, linked or unsafe. No file was copied or modified."));
    }
    let meta = std::fs::metadata(path).map_err(|_| invalid())?;
    if meta.len() > 256 * 1024 {
        return Err(invalid());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path)
        .and_then(|f| f.take(256 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|_| invalid())?;
    if bytes.len() > 256 * 1024 {
        return Err(invalid());
    }
    Ok(bytes)
}
async fn file(path: &Path) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let p = path.to_owned();
    tokio::task::spawn_blocking(move || raw_file(&p))
        .await
        .map_err(|_| Fault::offline())?
}
fn marker(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
fn timestamp(v: &Value) -> Option<chrono::DateTime<Utc>> {
    crate::parser::number(v)
        .filter(|n| n.is_finite() && *n > 0. && *n < 64092211200. && n.fract() == 0.)
        .and_then(|n| chrono::DateTime::from_timestamp(n as i64, 0))
}
fn percent(v: &Value) -> Option<f64> {
    crate::parser::number(v).filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
}
struct Reading {
    subject: String,
    login: String,
    report: Report,
}
impl Service {
    pub async fn connect_local_provider(
        &self,
        settings: &Settings,
        provider: Provider,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            bail!("Allow existing provider-source access explicitly. No login was started.");
        }
        if !matches!(
            provider,
            Provider::Devin | Provider::Antigravity | Provider::Muse
        ) {
            bail!("This is not an existing local provider reader.");
        }
        self.begin_verification(provider)?;
        let path = profile(&self.storage, provider);
        let result = tokio::time::timeout(
            Duration::from_secs(45),
            self.connect_local_at(settings, provider, &path),
        )
        .await
        .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(e) = &result {
            if let Some(f) = e.downcast_ref::<Fault>() {
                self.pause(provider, f);
            }
        }
        result
    }
    async fn connect_local_at(
        &self,
        settings: &Settings,
        provider: Provider,
        path: &Path,
    ) -> Result<(Settings, AccountState)> {
        if provider == Provider::Antigravity {
            return self.connect_antigravity(settings).await;
        }
        let read = self.read_local(provider, path).await?;
        let mode = match provider {
            Provider::Devin if path.file_name().is_some_and(|n| n == "state.vscdb") => {
                ConnectionMode::DevinApp
            }
            Provider::Devin => ConnectionMode::DevinCli,
            Provider::Antigravity => ConnectionMode::AntigravityApp,
            Provider::Muse => ConnectionMode::MuseCli,
            _ => unreachable!(),
        };
        self.persist_local(settings, provider, path, read, mode)
    }
    fn persist_local(
        &self,
        settings: &Settings,
        provider: Provider,
        path: &Path,
        read: Reading,
        mode: ConnectionMode,
    ) -> Result<(Settings, AccountState)> {
        let mut c = Connection::new(provider, read.subject, read.login, scope(provider, path));
        c.mode = mode;
        c.profile = Some(path.to_string_lossy().into_owned());
        let mut next = settings.clone();
        next.connections.retain(|old| {
            if old.provider != c.provider {
                return true;
            }
            if old.scope == c.scope {
                return false;
            }
            if c.provider == Provider::Antigravity {
                fn family(c: &Connection) -> Option<String> {
                    let p = Path::new(c.profile.as_deref()?);
                    let root = match c.mode {
                        ConnectionMode::AntigravityStored => p.ancestors().nth(3)?,
                        ConnectionMode::AntigravityApp => p,
                        _ => return None,
                    };
                    Some(crate::storage::path_key(root))
                }
                if let (Some(a), Some(b)) = (family(old), family(&c)) {
                    return a != b;
                }
            }
            true
        });
        next.connections.push(c.clone());
        next.validate()?;
        self.storage.save_settings(&next)?;
        Ok((
            next.clone(),
            AccountState::apply(c.account(&next), Ok(read.report), None),
        ))
    }
    pub(super) async fn fetch_local_provider(&self, c: &Connection) -> Result<Report, Fault> {
        if !valid_scope(c) {
            return Err(invalid());
        }
        let path = Path::new(c.profile.as_deref().ok_or_else(invalid)?);
        let read = if c.mode == ConnectionMode::MuseDevice {
            self.read_muse_device(path).await?
        } else if c.mode == ConnectionMode::AntigravityCli {
            self.read_antigravity_cli(path).await?
        } else if c.mode == ConnectionMode::AntigravityStored {
            self.read_antigravity_stored(path).await?
        } else if c.mode == ConnectionMode::AntigravityCliUsage {
            self.read_antigravity_cli_usage(path).await?
        } else {
            self.read_local(c.provider, path).await?
        };
        if read.subject != c.subject || read.login != c.login {
            return Err(Fault::changed());
        }
        Ok(read.report)
    }
    pub(super) async fn probe_local(&self, provider: Provider) -> Result<Report, Fault> {
        if provider == Provider::Antigravity {
            return self.antigravity_existing().await.map(|(_, r, _)| r.report);
        }
        self.read_local(provider, &profile(&self.storage, provider))
            .await
            .map(|r| r.report)
    }
    async fn read_local(&self, provider: Provider, path: &Path) -> Result<Reading, Fault> {
        match provider {
            Provider::Devin => self.read_devin(path).await,
            Provider::Antigravity => self.read_antigravity(path).await,
            Provider::Muse => self.read_muse(path).await,
            _ => Err(invalid()),
        }
    }
    async fn local_json(
        &self,
        request: reqwest::RequestBuilder,
        optional: bool,
    ) -> Result<Option<Value>, Fault> {
        self.json(request,optional).await.map_err(|f|match f.kind {
            FaultKind::PermissionDenied=>denied("The existing provider session does not permit this usage read. No alternative login, host or credential was tried."),
            FaultKind::NeedsRelogin=>Fault::new(FaultKind::NeedsRelogin,"The provider rejected its existing session. Recover access in the official app/CLI, then reconnect."),
            _=>f,
        })
    }
    async fn read_devin(&self, path: &Path) -> Result<Reading, Fault> {
        if !path.exists() {
            return Err(missing(Provider::Devin));
        }
        let desktop = path.file_name().is_some_and(|n| n == "state.vscdb");
        if desktop
            && crate::storage::path_key(path)
                != crate::storage::path_key(
                    &roaming(&self.storage).join("Devin/User/globalStorage/state.vscdb"),
                )
        {
            return Err(Fault::changed());
        }
        let token = devin_source_token(path).await?;
        let token_mark = hash(&token);
        let body=Zeroizing::new(serde_json::json!({"metadata":{"apiKey":token.as_str(),"ideName":"devin","ideVersion":"unknown","extensionName":"opentoken","extensionVersion":env!("CARGO_PKG_VERSION"),"locale":"en"}}).to_string());
        let value = self
            .local_json(
                self.http
                    .post(format!(
                        "{}/exa.seat_management_pb.SeatManagementService/GetUserStatus",
                        self.local.devin_base
                    ))
                    .header("Content-Type", "application/json")
                    .header("Connect-Protocol-Version", "1")
                    .body(body.as_bytes().to_vec()),
                false,
            )
            .await?
            .ok_or_else(invalid)?;
        if devin_source_token(path).await?.as_str() != token.as_str() {
            return Err(Fault::changed());
        }
        let mut report = parse_devin(&value)?;
        if desktop {
            report.source = ReportSource::DevinApp;
        }
        let status = &value["userStatus"];
        let context = ["userId", "email", "organizationId", "teamId"]
            .into_iter()
            .map(|k| status[k].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Reading {
            subject: format!("credential:{token_mark}:{}", hash(&context)),
            login: if desktop {
                "Devin app · credential scope"
            } else {
                "Devin CLI · credential scope"
            }
            .into(),
            report,
        })
    }
}
async fn devin_source_token(path: &Path) -> Result<Zeroizing<String>, Fault> {
    if path.file_name().is_some_and(|n| n == "state.vscdb") {
        let p = path.to_owned();
        let row = tokio::task::spawn_blocking(move || {
            devin_app::verify(&p)?;
            crate::source_db::read(&p, "windsurfAuthStatus", 256 * 1024)
        })
        .await
        .map_err(|_| Fault::offline())??;
        #[derive(Deserialize)]
        struct Auth {
            #[serde(rename = "apiKey")]
            key: String,
        }
        impl Drop for Auth {
            fn drop(&mut self) {
                zeroize::Zeroize::zeroize(&mut self.key);
            }
        }
        let auth: Auth = serde_json::from_str(&row).map_err(|_| denied("Devin's desktop sign-in has an unsupported format. No CLI login was started and its database was not copied."))?;
        if !(16..=16384).contains(&auth.key.len())
            || !auth.key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(invalid());
        }
        Ok(Zeroizing::new(auth.key.clone()))
    } else {
        devin_token(&file(path).await?)
    }
}
fn devin_token(bytes: &[u8]) -> Result<Zeroizing<String>, Fault> {
    #[derive(Deserialize)]
    struct Auth {
        windsurf_api_key: String,
        api_server_url: Option<String>,
    }
    impl Drop for Auth {
        fn drop(&mut self) {
            zeroize::Zeroize::zeroize(&mut self.windsurf_api_key);
        }
    }
    let auth: Auth = toml::from_str(std::str::from_utf8(bytes).map_err(|_| invalid())?)
        .map_err(|_| invalid())?;
    if !(16..=16384).contains(&auth.windsurf_api_key.len())
        || !auth.windsurf_api_key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(invalid());
    }
    if auth
        .api_server_url
        .as_deref()
        .is_some_and(|s| s.trim_end_matches('/') != "https://server.codeium.com")
    {
        return Err(denied("This Devin deployment uses an unsupported server. Its credential was not sent to any host."));
    }
    Ok(Zeroizing::new(auth.windsurf_api_key.clone()))
}
impl Service {
    pub(crate) fn local_watch_due(&self, settings: &Settings) -> bool {
        if !settings.connections.iter().any(|c| {
            matches!(
                c.mode,
                ConnectionMode::DevinApp | ConnectionMode::AntigravityStored
            )
        }) {
            return false;
        }
        let mut checked = self.local.scope_watch.lock().unwrap();
        let now = tokio::time::Instant::now();
        if checked.is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(1)) {
            return false;
        }
        *checked = Some(now);
        true
    }
    /// Permissioned local scope confirmation only; never bypasses network read budgets.
    pub(crate) async fn local_scope_changes(&self, settings: &Settings) -> HashSet<String> {
        let mut changed = HashSet::new();
        for c in settings.connections.iter().filter(|c| {
            matches!(
                c.mode,
                ConnectionMode::DevinCli
                    | ConnectionMode::DevinApp
                    | ConnectionMode::AntigravityStored
                    | ConnectionMode::MuseCli
                    | ConnectionMode::MuseDevice
            )
        }) {
            if c.mode == ConnectionMode::AntigravityStored {
                let Some(path) = c.profile.as_deref().map(Path::new) else {
                    changed.insert(c.id.clone());
                    continue;
                };
                match antigravity_stored::read(&self.storage, path).await {
                    Ok(s) => {
                        if self
                            .local
                            .stored_marks
                            .lock()
                            .unwrap()
                            .get(&crate::storage::path_key(path))
                            != Some(&s.stamp)
                        {
                            changed.insert(c.id.clone());
                        }
                    }
                    Err(f) if f.kind == FaultKind::Unreadable => {}
                    Err(_) => {
                        changed.insert(c.id.clone());
                    }
                }
                continue;
            }
            let result = async {
                let path = Path::new(c.profile.as_deref().ok_or_else(invalid)?);
                match c.mode {
                    ConnectionMode::DevinCli | ConnectionMode::DevinApp => {
                        Ok(hash(&devin_source_token(path).await?))
                    }
                    ConnectionMode::MuseCli | ConnectionMode::MuseDevice => {
                        muse::credential(&file(path).await?)
                    }
                    _ => Err(invalid()),
                }
            }
            .await;
            if c.mode == ConnectionMode::DevinApp
                && result
                    .as_ref()
                    .is_err_and(|f| f.kind == FaultKind::Unreadable)
            {
                continue;
            }
            if result.as_deref().ok() != c.subject.split(':').nth(1) {
                changed.insert(c.id.clone());
            }
        }
        changed
    }
}
fn parse_devin(value: &Value) -> Result<Report, Fault> {
    let status = &value["userStatus"];
    let plan = &status["planStatus"];
    let info = &plan["planInfo"];
    if info["isDevin"].as_bool() != Some(true)
        && !status["teamsTier"]
            .as_str()
            .is_some_and(|s| s.starts_with("TEAMS_TIER_DEVIN_"))
    {
        return Err(denied("The response did not establish a Devin entitlement. No Windsurf or organization quota was substituted."));
    }
    let mut quotas = vec![];
    for (field, id, label, reset, visible) in [
        (
            "dailyQuotaRemainingPercent",
            "devin_daily",
            "Daily quota",
            "dailyQuotaResetAtUnix",
            info["hideDailyQuota"].is_null() || info["hideDailyQuota"].as_bool() == Some(false),
        ),
        (
            "weeklyQuotaRemainingPercent",
            "devin_weekly",
            "Weekly quota",
            "weeklyQuotaResetAtUnix",
            info["hideWeeklyQuota"].is_null() || info["hideWeeklyQuota"].as_bool() == Some(false),
        ),
    ] {
        if visible {
            if let Some(left) = percent(&plan[field]) {
                quotas.push(Quota::window(
                    id,
                    label,
                    100. - left,
                    timestamp(&plan[reset]),
                ));
            }
        }
    }
    if info["hideDailyQuota"].as_bool() == Some(true)
        && plan["weeklyQuotaRemainingPercent"].is_null()
    {
        if let Some(raw) = percent(&plan["dailyQuotaRemainingPercent"]) {
            quotas.push(Quota::value(
                "devin_unmapped",
                "Reported quota field · window unverified",
                raw,
                "raw %",
            ));
        }
    }
    if let Some(micros) = crate::parser::number(&plan["overageBalanceMicros"])
        .filter(|n| n.is_finite() && (0.0..=9_007_199_254_740_991.0).contains(n) && n.fract() == 0.)
    {
        quotas.push(Quota::value(
            "devin_extra_balance",
            "Extra usage balance · USD",
            micros / 1_000_000.,
            "USD",
        ));
    }
    if quotas.is_empty() {
        return Err(denied("Devin did not supply an unambiguous quota or balance. Missing values were not treated as zero; daily fields were not relabelled weekly."));
    }
    let now = Utc::now();
    Ok(Report {
        plan: info["planName"]
            .as_str()
            .filter(|s| text(s, 100))
            .map(str::to_owned),
        quotas,
        observed_at: now,
        verified_at: Some(now),
        source: ReportSource::DevinCli,
    })
}
