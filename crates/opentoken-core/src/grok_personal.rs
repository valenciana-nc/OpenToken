//! Explicit read-only reuse of an official Grok CLI session; never refresh or copy it.
use super::*;
use serde_json::Value;
use std::io::Read;
use zeroize::{Zeroize, Zeroizing};
pub fn config_home(storage: &Storage) -> std::path::PathBuf {
    // Preserve an explicit invalid/relative context so the reader rejects it;
    // silently falling back could connect a different default account.
    std::env::var_os("GROK_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| storage.user_home.join(".grok"))
}
pub fn executable() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "grok.exe" } else { "grok" };
    let mut paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .collect();
    paths.push(Storage::system().user_home.join(".grok/bin").join(name));
    paths.into_iter().find(|p| {
        p.is_absolute()
            && std::fs::symlink_metadata(p)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    })
}
#[derive(Deserialize)]
struct Entry {
    key: String,
    expires_at: Option<Value>,
    user_id: Option<String>,
    principal_type: Option<String>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
struct Session {
    token: Zeroizing<String>,
    subject: Option<String>,
    expires: Option<i64>,
}
fn expiry_hint(value: Option<&Value>) -> Option<i64> {
    let v = value?;
    if let Some(s) = v.as_str() {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(t.timestamp());
        }
    }
    let n = v.as_i64().or_else(|| v.as_str()?.parse::<i64>().ok())?;
    if n <= 0 {
        return None;
    }
    let seconds = if n >= 100_000_000_000 { n / 1000 } else { n };
    chrono::DateTime::from_timestamp(seconds, 0).map(|t| t.timestamp())
}
fn read(home: &std::path::Path) -> Result<Session, Fault> {
    let path = home.join("auth.json");
    if !path.is_absolute() {
        return Err(invalid());
    }
    for ancestor in path.ancestors() {
        if let Ok(meta) = std::fs::symlink_metadata(ancestor) {
            let linked = meta.file_type().is_symlink();
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                linked || meta.file_attributes() & 0x400 != 0
            };
            if linked {
                return Err(Fault::new(
                    FaultKind::Unreadable,
                    "Grok's selected sign-in could not be read safely.",
                ));
            }
        }
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Fault::new(FaultKind::NotSignedIn, "Grok's saved login file is missing. Recover access through the official Grok CLI.")
        } else {
            Fault::new(FaultKind::Unreadable, "Grok's selected login is temporarily unreadable. Monitoring is still saved; no sign-out was inferred.")
        }
    })?;
    if !meta.is_file() || meta.len() > 256 * 1024 {
        return Err(invalid());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path)
        .and_then(|f| f.take(256 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|_| Fault::new(FaultKind::Unreadable, "Grok's sign-in could not be read."))?;
    if bytes.len() > 256 * 1024 {
        return Err(invalid());
    }
    let entries: std::collections::BTreeMap<String, Entry> =
        serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if entries.len() > 16 {
        return Err(invalid());
    }
    let mut matches = entries
        .iter()
        .filter(|(scope, entry)| scope.starts_with("https://auth.x.ai::") && !entry.key.is_empty());
    let (_, entry) = matches.next().ok_or_else(|| {
        Fault::new(
            FaultKind::NeedsRelogin,
            "Run the official Grok login to create a current SuperGrok OAuth session.",
        )
    })?;
    if matches.next().is_some() {
        return Err(Fault::new(
            FaultKind::Unreadable,
            "More than one Grok OAuth session was found. Select the intended login in Grok first.",
        ));
    }
    if entry
        .principal_type
        .as_deref()
        .is_some_and(|p| p.eq_ignore_ascii_case("team"))
    {
        return Err(Fault::new(
            FaultKind::PermissionDenied,
            "This is a Grok team principal. No personal quota was inferred from it.",
        ));
    }
    // Cache timestamps are hints, not authentication. Unknown is not expired;
    // explicit usage reads let the fixed provider accept/reject the saved bearer.
    let expires = expiry_hint(entry.expires_at.as_ref());
    if !(16..=16384).contains(&entry.key.len()) || !entry.key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(invalid());
    }
    Ok(Session {
        token: Zeroizing::new(entry.key.clone()),
        subject: entry.user_id.clone(),
        expires,
    })
}
fn source_marker(session: &Session) -> String {
    let material = Zeroizing::new(format!("{}\n{:?}", session.token.as_str(), session.subject));
    hash(&material)
}
impl Service {
    /// Only explicitly monitored sources are inspected. No HTTP, launch or renewal.
    /// Changes invalidate cached identity-bound quota, never reset the network budget.
    pub(crate) async fn grok_scope_changes(&self, settings: &Settings) -> HashSet<String> {
        let mut changed = HashSet::new();
        for c in settings
            .connections
            .iter()
            .filter(|c| c.mode == ConnectionMode::GrokCli)
        {
            let Some(home) = c.profile.as_deref().map(std::path::Path::new) else {
                continue;
            };
            let marker = match self.grok_session(home).await {
                Ok(session) => source_marker(&session),
                // Partial writes/locks are not evidence of a different user.
                Err(f) if f.kind == FaultKind::Unreadable => continue,
                Err(f) => format!("fault:{:?}", f.kind),
            };
            let prior = self
                .grok_sources
                .lock()
                .unwrap()
                .insert(crate::storage::path_key(home), marker.clone());
            if prior.is_some_and(|p| p != marker) {
                changed.insert(c.id.clone());
            }
        }
        changed
    }
    fn grok_home(&self) -> std::path::PathBuf {
        #[cfg(test)]
        if let Some(home) = &self.grok_test_home {
            return home.clone();
        }
        config_home(&self.storage)
    }
    async fn grok_session(&self, home: &std::path::Path) -> Result<Session, Fault> {
        let home = home.to_owned();
        tokio::task::spawn_blocking(move || read(&home))
            .await
            .map_err(|_| Fault::offline())?
    }
    pub async fn grok_sign_in_marker(&self, consent: bool) -> Result<String, Fault> {
        if !consent {
            return Err(Fault::new(
                FaultKind::PermissionDenied,
                "Start Grok sign-in explicitly.",
            ));
        }
        let session = self.grok_session(&self.grok_home()).await?;
        // Presence/change marker only; authenticated quota verification decides readiness.
        let material = Zeroizing::new(format!("{}:{:?}", session.token.as_str(), session.expires));
        Ok(hash(&material))
    }
    async fn grok_identity(&self, token: &str) -> Result<(String, String), Fault> {
        let value: Value = self
            .session_json(
                self.http
                    .get(format!("{}/oauth2/userinfo", self.grok_identity_base))
                    .bearer_auth(token),
                Provider::Grok,
                "Grok identity check",
            )
            .await.map_err(|mut f|{if f.kind==FaultKind::NeedsRelogin {f.message="Grok rejected the saved OAuth session. Monitoring is still saved. Choose Recover Grok session for the official CLI's model-list/recovery check, or choose Sign in with Grok explicitly for new OAuth setup. OpenToken never renews the grant itself.".into();}f})?;
        let subject = value["sub"]
            .as_str()
            .filter(|s| text(s, 256))
            .ok_or_else(invalid)?
            .to_string();
        let login = value["email"]
            .as_str()
            .filter(|s| email(s))
            .unwrap_or(&subject)
            .to_string();
        Ok((subject, login))
    }
    pub async fn connect_grok_personal(
        &self,
        settings: &Settings,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            bail!("Start Grok sign-in explicitly.");
        }
        self.begin_verification(Provider::Grok)?;
        let result: Result<(Settings, AccountState)> =
            tokio::time::timeout(Duration::from_secs(45), async {
                let (c, report) = self.grok_existing_reading().await?;
                let mut next = settings.clone();
                next.connections
                    .retain(|old| old.mode != ConnectionMode::GrokCli || old.scope != c.scope);
                next.connections.push(c.clone());
                next.validate()?;
                self.storage.save_settings(&next)?;
                Ok((
                    next.clone(),
                    AccountState::apply(c.account(&next), Ok(report), None),
                ))
            })
            .await
            .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(error) = &result {
            if let Some(fault) = error.downcast_ref::<Fault>() {
                self.pause(Provider::Grok, fault);
            }
        }
        result
    }
    pub(super) async fn grok_existing_reading(&self) -> Result<(Connection, Report), Fault> {
        let home = self.grok_home();
        let session = self.grok_session(&home).await?;
        let (subject, login) = self.grok_identity(&session.token).await?;
        if session.subject.as_deref().is_some_and(|s| s != subject) {
            return Err(Fault::changed());
        }
        let mut c = Connection::new(
            Provider::Grok,
            subject,
            login,
            format!("grok:{}", hash(&crate::storage::path_key(&home))),
        );
        c.mode = ConnectionMode::GrokCli;
        c.profile = Some(home.to_string_lossy().into_owned());
        let report = self.fetch_grok_personal(&c).await?;
        Ok((c, report))
    }
    pub(super) async fn fetch_grok_personal(&self, c: &Connection) -> Result<Report, Fault> {
        let home = c
            .profile
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(invalid)?;
        let session = self.grok_session(home).await?;
        let before = self.grok_identity(&session.token).await?;
        if before.0 != c.subject
            || before.1 != c.login
            || session.subject.as_deref().is_some_and(|s| s != c.subject)
        {
            return Err(Fault::changed());
        }
        let value: Value = self
            .session_json(
                self.http
                    .get(format!(
                        "{}/v1/billing?format=credits",
                        self.grok_proxy_base
                    ))
                    .bearer_auth(session.token.as_str())
                    .header("x-xai-token-auth", "xai-grok-cli")
                    .header("Accept", "application/json"),
                Provider::Grok,
                "Grok credit quota read",
            )
            .await?;
        let report = parse(&value)?;
        let after = self.grok_identity(&session.token).await?;
        let local = self.grok_session(home).await?;
        if before != after || *local.token != *session.token || local.subject != session.subject {
            return Err(Fault::changed());
        }
        self.grok_sources
            .lock()
            .unwrap()
            .insert(crate::storage::path_key(home), source_marker(&session));
        Ok(report)
    }
}
fn parse(value: &Value) -> Result<Report, Fault> {
    let config = value["config"].as_object().ok_or_else(invalid)?;
    let now = Utc::now();
    let at = config
        .get("currentPeriod")
        .and_then(|p| p["end"].as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .or_else(|| {
            config
                .get("billingPeriodEnd")
                .and_then(Value::as_str)
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        })
        .map(|d| d.with_timezone(&Utc));
    let mut quotas = vec![];
    if let Some(percent) = config
        .get("creditUsagePercent")
        .and_then(crate::parser::number)
    {
        if !percent.is_finite() || percent < 0. {
            return Err(invalid());
        }
        quotas.push(Quota {
            id: "grok_credit_usage".into(),
            label: "Reported credit usage".into(),
            percent: Some(percent),
            value: None,
            unit: None,
            resets_at: at,
        });
    }
    if let (Some(cap), Some(used)) = (
        crate::parser::number(&value["config"]["onDemandCap"]["val"]),
        crate::parser::number(&value["config"]["onDemandUsed"]["val"]),
    ) {
        if cap.is_finite()
            && cap > 0.
            && used.is_finite()
            && used >= 0.
            && (used / cap * 100.).is_finite()
        {
            quotas.push(Quota {
                id: "grok_on_demand_budget".into(),
                label: "On-demand budget".into(),
                percent: Some(used / cap * 100.),
                value: None,
                unit: None,
                resets_at: at,
            });
        }
    }
    let raw = &value["config"]["prepaidBalance"]["val"];
    if let Some(cents) = raw
        .as_i64()
        .or_else(|| raw.as_str().and_then(|s| s.parse::<i64>().ok()))
        .filter(|n| (0..=9_007_199_254_740_992).contains(n))
    {
        quotas.push(Quota {
            id: "grok_prepaid_balance".into(),
            label: "Prepaid balance · USD".into(),
            percent: None,
            value: Some(cents as f64 / 100.),
            unit: Some("USD".into()),
            resets_at: None,
        });
    }
    let plan = value["config"]["subscriptionTier"]
        .as_str()
        .or_else(|| value["subscriptionTier"].as_str())
        .filter(|s| text(s, 100))
        .map(str::to_string);
    if quotas.is_empty() && at.is_none() && plan.is_none() {
        return Err(invalid());
    }
    Ok(Report {
        plan,
        quotas,
        observed_at: now,
        verified_at: Some(now),
        source: ReportSource::GrokCli,
    })
}
#[cfg(test)]
#[path = "grok_personal_tests.rs"]
mod tests;
