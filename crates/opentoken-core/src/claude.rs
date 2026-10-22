use crate::{
    codex::{bounded_json, check_response},
    model::*,
    parser,
    storage::{atomic_json, normalize, path_key, read_json, Settings},
};
use chrono::Utc;
use fs2::FileExt;
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    time::Duration,
};

pub struct ClaudeAdapter {
    pub http: reqwest::Client,
    pub user_home: PathBuf,
}
impl ClaudeAdapter {
    pub fn accounts(&self, s: &Settings) -> Vec<Account> {
        let mut homes: Vec<_> = s
            .claude_homes
            .iter()
            .map(|p| normalize(p, &self.user_home))
            .collect();
        if homes.is_empty() {
            if let Some(p) = std::env::var_os("CLAUDE_CONFIG_DIR") {
                homes.push(PathBuf::from(p));
            }
            homes.push(self.user_home.join(".claude"));
        }
        let ignored: std::collections::HashSet<_> = s
            .ignored_homes
            .iter()
            .map(|p| path_key(&normalize(p, &self.user_home)))
            .collect();
        let mut seen = std::collections::HashSet::new();
        homes
            .into_iter()
            .filter(|p| !ignored.contains(&path_key(p)) && seen.insert(path_key(p)))
            .map(|home| {
                let id = format!("claude-{}", hash(&path_key(&home)));
                Account {
                    label: s.aliases.get(&id).cloned().unwrap_or_else(|| {
                        if home == self.user_home.join(".claude") {
                            "Claude".into()
                        } else if home
                            .parent()
                            .and_then(|p| p.file_name())
                            .is_some_and(|n| n == "claude")
                            && home.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                                n.len() == 24 && n.bytes().all(|b| b.is_ascii_hexdigit())
                            })
                        {
                            format!("Claude · account {}", &hash(&path_key(&home))[..6])
                        } else {
                            format!(
                                "Claude · {}",
                                home.file_name().unwrap_or_default().to_string_lossy()
                            )
                        }
                    }),
                    id,
                    provider: Provider::Claude,
                    email: None,
                    identity: None,
                    sources: vec![Source {
                        path: home,
                        store: Store::File,
                    }],
                }
            })
            .collect()
    }
    pub fn monitored_accounts(&self, s: &Settings) -> Vec<Account> {
        let mut accounts = self.accounts(s);
        for account in &mut accounts {
            if let Some(path) = account
                .sources
                .first()
                .map(|s| s.path.join(".credentials.json"))
                .filter(|p| crate::monitoring::safe_file(p))
            {
                if let Ok(value) = load(&path) {
                    if let Some(token) = value["claudeAiOauth"]["accessToken"]
                        .as_str()
                        .filter(|t| !t.trim().is_empty())
                    {
                        Self::bind_scope(account, &hash(token.trim()));
                    }
                }
            }
        }
        accounts
    }
    pub(crate) fn bind_scope(account: &mut Account, marker: &str) {
        if let Some(source) = account.sources.first() {
            account.identity = Some(Identity {
                workspace: format!("claude-profile:{}", hash(&path_key(&source.path))),
                email: None,
                user: Some(format!("credential:{marker}")),
            });
        }
    }
    pub async fn fetch(&self, a: &Account, allow_refresh: bool) -> Result<Report, Fault> {
        self.fetch_scoped(a, allow_refresh).await.map(|(r, _)| r)
    }
    pub(crate) async fn fetch_scoped(
        &self,
        a: &Account,
        allow_refresh: bool,
    ) -> Result<(Report, String), Fault> {
        let path = a
            .sources
            .first()
            .ok_or_else(|| {
                Fault::new(
                    FaultKind::NotSignedIn,
                    "Sign in with Claude to connect a subscription.",
                )
            })?
            .path
            .join(".credentials.json");
        let mut auth = load(&path)?;
        let oauth = &auth["claudeAiOauth"];
        if let Some(scopes) = oauth["scopes"].as_array() {
            if !scopes.is_empty() && !scopes.iter().any(|s| s == "user:profile") {
                return Err(Fault::new(
                    FaultKind::NeedsRelogin,
                    "Re-login with Claude Code to grant the usage-profile scope.",
                ));
            }
        }
        let exp = parser::number(&oauth["expiresAt"]);
        let mut token = parser::text(&oauth["accessToken"]).unwrap_or_default();
        // Proactive five-minute renewal is only for separately approved monitoring.
        // Existing-source reads let the provider decide bearer acceptance; cached expiry is only a renewal hint.
        let needs_refresh =
            renewal_needed(&token, exp, allow_refresh, Utc::now().timestamp_millis());
        if needs_refresh {
            token = self.refresh(&path, &auth, allow_refresh).await?;
            auth = load(&path)?;
        }
        let response = self.usage(&token).await?;
        let response = if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if !allow_refresh {
                return Err(Fault::new(FaultKind::NeedsRelogin,"Claude rejected the saved OAuth session. Recover it in the official Claude Code CLI, then use existing again. OpenToken did not renew or change its credentials."));
            }
            // Exactly one refresh+retry, and re-read after locking to respect concurrent logins.
            token = self.refresh(&path, &auth, allow_refresh).await?;
            auth = load(&path)?;
            self.usage(&token).await?
        } else {
            response
        };
        check_response(&response)?;
        let v = bounded_json(response).await?;
        let current = load(&path)?;
        if current["claudeAiOauth"]["accessToken"] != auth["claudeAiOauth"]["accessToken"] {
            return Err(Fault::changed());
        }
        parser::claude(
            &v,
            parser::text(&auth["claudeAiOauth"]["subscriptionType"]),
            parser::text(&auth["claudeAiOauth"]["rateLimitTier"]),
            Utc::now(),
        )
        .map(|report| (report, hash(token.trim())))
    }
    async fn usage(&self, token: &str) -> Result<reqwest::Response, Fault> {
        // Only production HTTPS hosts. Never forward shared CLI credentials to an environment-controlled URL.
        self.http
            .get("https://api.anthropic.com/api/oauth/usage")
            .bearer_auth(token.trim())
            .header("anthropic-beta", "oauth-2025-04-20")
            .header(
                "User-Agent",
                concat!("OpenToken/", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .await
            .map_err(|_| Fault::offline())
    }
    async fn refresh(&self, path: &Path, original: &Value, allowed: bool) -> Result<String, Fault> {
        if !allowed {
            return Err(Fault::new(
                FaultKind::NeedsRelogin,
                "Claude's saved credential has no usable access token. Recover it in the official Claude Code CLI, then use existing again. Existing-source connection never renews credentials.",
            ));
        }
        let lock_path = path.with_file_name(".opentoken-refresh.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|_| {
                Fault::new(
                    FaultKind::Unreadable,
                    "Cannot lock the Claude credential folder.",
                )
            })?;
        let start = tokio::time::Instant::now();
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => break,
                Err(_) if start.elapsed() < Duration::from_secs(5) => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                Err(_) => return Err(Fault::new(
                    FaultKind::Unreadable,
                    "Another OpenToken process is updating this Claude login. Try again shortly.",
                )),
            }
        }
        // The file lock is released by Drop on every path, including cancellation.
        let mut current = load(path)?;
        if current["claudeAiOauth"]["accessToken"] != original["claudeAiOauth"]["accessToken"] {
            return parser::text(&current["claudeAiOauth"]["accessToken"]).ok_or_else(|| {
                Fault::new(
                    FaultKind::NeedsRelogin,
                    "Claude login changed. Refresh again.",
                )
            });
        }
        let refresh = parser::text(&current["claudeAiOauth"]["refreshToken"]).ok_or_else(|| {
            Fault::new(
                FaultKind::NeedsRelogin,
                "Claude session expired. Run claude to sign in again.",
            )
        })?;
        let response=self.http.post("https://platform.claude.com/v1/oauth/token").json(&json!({"grant_type":"refresh_token","refresh_token":refresh,"client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e","scope":"user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload"})).send().await.map_err(|_|Fault::offline())?;
        check_response(&response)?;
        let updated = bounded_json(response).await?;
        let access = parser::text(&updated["access_token"]).ok_or_else(|| {
            Fault::new(
                FaultKind::Unknown,
                "Claude returned an invalid token-refresh response.",
            )
        })?;
        let latest = load(path)?;
        if latest["claudeAiOauth"] != current["claudeAiOauth"] {
            return Err(Fault::new(FaultKind::NeedsRelogin,"Claude credentials changed during refresh. Open Claude Code to verify your sign-in; OpenToken did not overwrite it."));
        }
        // Preserve even unrelated fields modified by Claude Code while HTTP was in flight.
        current = latest;
        current["claudeAiOauth"]["accessToken"] = json!(access);
        if let Some(token) = parser::text(&updated["refresh_token"]) {
            current["claudeAiOauth"]["refreshToken"] = json!(token);
        }
        if let Some(seconds) =
            parser::number(&updated["expires_in"]).filter(|n| (0.0..=31536000.0).contains(n))
        {
            current["claudeAiOauth"]["expiresAt"] =
                json!(Utc::now().timestamp_millis() + (seconds * 1000.0) as i64);
        }
        atomic_json(path,&current).map_err(|_|Fault::new(FaultKind::Unreadable,"Claude renewed its token but the credential file could not be saved. Re-login with Claude Code."))?;
        Ok(access)
    }
}
fn renewal_needed(token: &str, expires: Option<f64>, allowed: bool, now: i64) -> bool {
    token.trim().is_empty()
        || (allowed && expires.is_some_and(|e| e <= now.saturating_add(300000) as f64))
}
fn load(path: &Path) -> Result<Value, Fault> {
    if !path.exists() {
        return Err(Fault::new(
            FaultKind::NotSignedIn,
            "Not signed in. Run claude to connect your subscription.",
        ));
    }
    if !crate::monitoring::safe_file(path) {
        return Err(Fault::new(
            FaultKind::Unreadable,
            "The Claude credential source is not a regular unlinked file.",
        ));
    }
    let value: Value = read_json(path).map_err(|_| {
        Fault::new(
            FaultKind::Unreadable,
            "The Claude credential file could not be read.",
        )
    })?;
    if !value["claudeAiOauth"].is_object() {
        return Err(Fault::new(
            FaultKind::NotSignedIn,
            "This folder has no Claude OAuth sign-in.",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_connection_does_not_demand_proactive_renewal_of_a_valid_token() {
        let now = 1000000;
        assert!(!renewal_needed(
            "fixture-only",
            Some((now + 180000) as f64),
            false,
            now
        ));
        assert!(renewal_needed(
            "fixture-only",
            Some((now + 180000) as f64),
            true,
            now
        ));
        assert!(!renewal_needed(
            "fixture-only",
            Some((now + 30000) as f64),
            false,
            now
        ));
        assert!(renewal_needed("", Some((now + 3600000) as f64), false, now));
        assert!(!renewal_needed(
            "fixture-only",
            Some((now - 1) as f64),
            false,
            now
        ));
        assert!(!renewal_needed("fixture-only", None, false, now));
    }
    #[test]
    fn missing_signin_is_typed() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&t.path().join(".credentials.json")).unwrap_err().kind,
            FaultKind::NotSignedIn
        );
    }
    #[test]
    fn supports_multiple_isolated_homes() {
        let a = ClaudeAdapter {
            http: reqwest::Client::new(),
            user_home: PathBuf::from("/test"),
        };
        let s = Settings {
            claude_homes: vec!["/a".into(), "/b".into()],
            ..Default::default()
        };
        let accounts = a.accounts(&s);
        assert_eq!(accounts.len(), 2);
        assert_ne!(accounts[0].id, accounts[1].id);
    }
}
