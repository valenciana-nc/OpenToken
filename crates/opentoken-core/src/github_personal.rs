//! Opt-in GitHub CLI-owned login and private Copilot entitlement snapshots.
use super::*;
use tokio::io::AsyncReadExt;
use zeroize::Zeroizing;
#[cfg(test)]
#[path = "github_personal_tests.rs"]
mod tests;
pub fn config_home(storage: &Storage) -> std::path::PathBuf {
    std::env::var_os("GH_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join("GitHub CLI"))
        })
        .unwrap_or_else(|| storage.user_home.join(".config/gh"))
}
pub fn executable() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "gh.exe" } else { "gh" };
    let mut paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .collect();
    if let Some(p) = std::env::var_os("ProgramFiles") {
        paths.push(std::path::PathBuf::from(p).join("GitHub CLI/gh.exe"));
    }
    paths.into_iter().find(|p| {
        p.is_absolute()
            && std::fs::symlink_metadata(p)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    })
}
fn token_output(success: bool, bytes: &[u8], stderr: &[u8]) -> Result<Zeroizing<String>, Fault> {
    if !success {
        // gh also fails for OS credential-store/process errors. Those are NOT proof
        // that the user signed out. Never copy its raw error/token into a Fault.
        let error = Zeroizing::new(String::from_utf8_lossy(stderr).to_ascii_lowercase());
        return Err(
            if error.contains("no oauth token found for github.com")
                || error.contains("not logged into any hosts")
            {
                Fault::new(FaultKind::NotSignedIn, "GitHub CLI has no saved github.com login. Sign in through the official client.")
            } else {
                Fault::new(FaultKind::Unreadable, "GitHub CLI could not read its saved login. Monitoring is still saved; OpenToken will retry without starting sign-in.")
            },
        );
    }
    if !(16..=8192).contains(&bytes.len()) {
        return Err(invalid());
    }
    let raw = Zeroizing::new(String::from_utf8(bytes.to_vec()).map_err(|_| invalid())?);
    let value = Zeroizing::new(raw.trim().to_string());
    if !(16..=8192).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid());
    }
    Ok(value)
}
async fn token(home: &std::path::Path) -> Result<Zeroizing<String>, Fault> {
    let exe=executable().ok_or_else(||Fault::new(FaultKind::ToolMissing,"Install the official GitHub CLI, then sign in to connect Copilot. Nothing was installed automatically."))?;
    let mut command = crate::codex::quiet_command(exe);
    command
        .args(["auth", "token", "--hostname", "github.com"])
        .env("GH_CONFIG_DIR", home)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    for key in [
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "GH_ENTERPRISE_TOKEN",
        "GITHUB_ENTERPRISE_TOKEN",
        "GH_HOST",
    ] {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .map_err(|_| Fault::new(FaultKind::ToolMissing, "GitHub CLI could not start."))?;
    let result = tokio::time::timeout(Duration::from_secs(8), async {
        let mut bytes = Zeroizing::new(Vec::new());
        let mut errors = Zeroizing::new(Vec::new());
        let mut stdout = child.stdout.take().ok_or_else(invalid)?.take(8193);
        let mut stderr = child.stderr.take().ok_or_else(invalid)?.take(16385);
        let (out, err) = tokio::join!(
            stdout.read_to_end(&mut bytes),
            stderr.read_to_end(&mut errors),
        );
        out.map_err(|_| Fault::offline())?;
        err.map_err(|_| Fault::offline())?;
        if bytes.len() > 8192 || errors.len() > 16384 {
            return Err(invalid());
        }
        let status = child.wait().await.map_err(|_| Fault::offline())?;
        token_output(status.success(), &bytes, &errors)
    })
    .await
    .unwrap_or_else(|_| Err(Fault::offline()));
    let _ = child.kill().await;
    let _ = child.wait().await;
    result.map_err(|fault| {
        tracing::warn!(provider=?Provider::Copilot, stage="GitHub CLI credential read", kind=?fault.kind, "Session usage stage failed");
        Fault { message: format!("GitHub CLI credential read: {}", fault.message), ..fault }
    })
}
impl Service {
    async fn gh_token(&self, home: &std::path::Path) -> Result<Zeroizing<String>, Fault> {
        #[cfg(test)]
        if let Some(token) = &self.github_test_token {
            return Ok(Zeroizing::new(token.lock().unwrap().clone()));
        }
        token(home).await
    }
    pub async fn github_sign_in_marker(&self, consent: bool) -> Result<String, Fault> {
        if !consent {
            return Err(Fault::new(
                FaultKind::PermissionDenied,
                "Start GitHub sign-in explicitly.",
            ));
        }
        Ok(hash(&self.gh_token(&config_home(&self.storage)).await?))
    }
    pub async fn connect_github_personal(
        &self,
        settings: &Settings,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            bail!("Start GitHub sign-in explicitly.");
        }
        self.begin_verification(Provider::Copilot)?;
        let result =
            tokio::time::timeout(Duration::from_secs(45), self.connect_github_inner(settings))
                .await
                .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(error) = &result {
            if let Some(fault) = error.downcast_ref::<Fault>() {
                self.pause(Provider::Copilot, fault);
            }
        }
        result
    }
    async fn connect_github_inner(&self, settings: &Settings) -> Result<(Settings, AccountState)> {
        let profile = config_home(&self.storage);
        let token = self.gh_token(&profile).await?;
        let user = self.github_session_user(&token).await?;
        let scope = format!("gh:{}", hash(&crate::storage::path_key(&profile)));
        let mut connection =
            Connection::new(Provider::Copilot, user.id.to_string(), user.login, scope);
        connection.mode = ConnectionMode::GithubCli;
        connection.profile = Some(profile.to_string_lossy().into_owned());
        let report = self.copilot_internal(&connection, &token).await?;
        if *self.gh_token(&profile).await? != *token {
            return Err(Fault::changed().into());
        }
        let mut next = settings.clone();
        next.connections
            .retain(|c| c.mode != ConnectionMode::GithubCli || c.scope != connection.scope);
        next.connections.push(connection.clone());
        next.validate()?;
        self.storage.save_settings(&next)?;
        Ok((
            next.clone(),
            AccountState::apply(connection.account(&next), Ok(report), None),
        ))
    }
    pub(super) async fn fetch_github_personal(&self, c: &Connection) -> Result<Report, Fault> {
        let profile = c
            .profile
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(invalid)?;
        if c.scope != format!("gh:{}", hash(&crate::storage::path_key(profile))) {
            return Err(Fault::changed());
        }
        let token = self.gh_token(profile).await?;
        let report = self.copilot_internal(c, &token).await?;
        if *self.gh_token(profile).await? != *token {
            return Err(Fault::changed());
        }
        Ok(report)
    }
    async fn github_session_user(&self, token: &str) -> Result<GithubUser, Fault> {
        let user: GithubUser = self
            .session_json(
                self.github_request("/user", token),
                Provider::Copilot,
                "GitHub identity check",
            )
            .await?;
        if user.id == 0 || !github_login(&user.login) {
            return Err(invalid());
        }
        Ok(user)
    }
    async fn copilot_internal(&self, c: &Connection, token: &str) -> Result<Report, Fault> {
        let before = self.github_session_user(token).await?;
        if before.id.to_string() != c.subject || before.login != c.login {
            return Err(Fault::changed());
        }
        let mut authorization = reqwest::header::HeaderValue::from_str(&format!("token {}", token))
            .map_err(|_| invalid())?;
        authorization.set_sensitive(true);
        let value: serde_json::Value = self
            .session_json(
                self.http
                    .get(format!("{}/copilot_internal/user", self.github_base))
                    .header(reqwest::header::AUTHORIZATION, authorization)
                    .header("Accept", "application/json")
                    .header("X-GitHub-Api-Version", "2026-03-10"),
                Provider::Copilot,
                "Copilot quota read",
            )
            .await?;
        let after = self.github_session_user(token).await?;
        if before.id != after.id || before.login != after.login {
            return Err(Fault::changed());
        }
        let now = Utc::now();
        let mut quotas = vec![];
        let reset = value["quota_reset_date"].as_str().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
                .or_else(|| {
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .map(|d| d.and_utc())
                })
        });
        for (key, label) in [
            ("premium_interactions", "Premium requests"),
            ("chat", "Chat"),
            ("completions", "Completions"),
        ] {
            let bucket = &value["quota_snapshots"][key];
            if bucket.is_null()
                || bucket["unlimited"] == true
                || crate::parser::number(&bucket["entitlement"]).is_some_and(|n| n <= 0.)
            {
                continue;
            }
            if let Some(left) = crate::parser::number(&bucket["percent_remaining"]) {
                if left.is_finite() && (0. ..=100.).contains(&left) {
                    quotas.push(Quota {
                        id: format!("copilot_{key}"),
                        label: label.into(),
                        percent: Some(100. - left),
                        value: None,
                        unit: None,
                        resets_at: reset,
                    });
                }
            }
        }
        if let Some(used) =
            crate::parser::number(&value["quota_snapshots"]["premium_interactions"]["credits_used"])
                .filter(|n| n.is_finite() && *n >= 0.)
        {
            if used > 0. || value["token_based_billing"] == true {
                quotas.push(Quota {
                    id: "copilot_ai_credits".into(),
                    label: "AI credits used · reported".into(),
                    percent: None,
                    value: Some(used),
                    unit: Some("credits".into()),
                    resets_at: reset,
                });
            }
        }
        if quotas.is_empty() && value["copilot_plan"].as_str().is_none() {
            return Err(invalid());
        }
        Ok(Report {
            plan: value["copilot_plan"]
                .as_str()
                .filter(|s| s.len() <= 100)
                .map(str::to_string),
            quotas,
            observed_at: now,
            verified_at: Some(now),
            source: ReportSource::CopilotInternal,
        })
    }
}
