//! Separately approved owner operation. OpenToken never sends a refresh grant.
//! Only the installed CLI's fixed `models` command may maintain its own session.
use super::*;
use std::{collections::HashMap, path::Path};
use tokio::io::AsyncReadExt;
use zeroize::Zeroizing;

#[async_trait::async_trait]
pub(super) trait Owner: Send + Sync {
    async fn renew(&self, home: &Path) -> Result<(), Fault>;
}
struct Cli;
pub(super) struct Renewal {
    attempts: tokio::sync::Mutex<HashMap<String, tokio::time::Instant>>,
    owner: Arc<dyn Owner>,
}
impl Default for Renewal {
    fn default() -> Self {
        Self {
            attempts: Default::default(),
            owner: Arc::new(Cli),
        }
    }
}
impl Renewal {
    pub(super) async fn run(&self, home: &Path) -> Result<(), Fault> {
        // Held across the operation: no overlapping refresh-token owners from this app.
        let mut attempts = self.attempts.lock().await;
        let key = crate::storage::path_key(home);
        if attempts
            .get(&key)
            .is_some_and(|at| at.elapsed() < Duration::from_secs(900))
        {
            return Err(Fault::new(FaultKind::NeedsRelogin, "Grok still rejects this session. The CLI renewal attempt is limited to once per 15 minutes; finish an explicit Grok sign-in if recovery is needed."));
        }
        attempts.insert(key, tokio::time::Instant::now());
        self.owner.renew(home).await
    }
    #[cfg(test)]
    pub(super) fn with_owner(owner: Arc<dyn Owner>) -> Self {
        Self {
            attempts: Default::default(),
            owner,
        }
    }
}
fn denied(message: &str) -> Fault {
    Fault::new(FaultKind::PermissionDenied, message)
}
fn approved_version(bytes: &[u8]) -> bool {
    // Reviewed installed version. An update needs a new compatibility check, not guessed flags.
    std::str::from_utf8(bytes).is_ok_and(|s| s.trim() == "grok 1.0.67 (2fe064390d07)")
}
fn safe_file(path: &Path) -> Result<Option<Vec<u8>>, Fault> {
    use std::io::Read;
    if !path.is_absolute() {
        return Err(invalid());
    }
    for ancestor in path.ancestors() {
        if let Ok(m) = std::fs::symlink_metadata(ancestor) {
            let linked = m.file_type().is_symlink();
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                linked || m.file_attributes() & 0x400 != 0
            };
            if linked {
                return Err(denied(
                    "Linked Grok paths are not supported for CLI renewal.",
                ));
            }
        }
    }
    let m = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(invalid()),
    };
    if !m.is_file() || m.len() > 256 * 1024 {
        return Err(invalid());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .and_then(|f| f.take(256 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|_| invalid())?;
    if bytes.len() > 256 * 1024 {
        return Err(invalid());
    }
    Ok(Some(bytes))
}
fn context(home: &Path) -> Result<(), Fault> {
    // Refuse deployments that can replace authentication/endpoints or run an auth helper.
    // Ordinary UI/model-choice preferences are left unchanged. No config is copied or rewritten.
    for name in ["config.toml", "managed_config.toml", "requirements.toml"] {
        let Some(raw) = safe_file(&home.join(name))? else {
            continue;
        };
        let raw = Zeroizing::new(raw);
        let value: toml::Value = toml::from_str(std::str::from_utf8(&raw).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
        let table = value.as_table().ok_or_else(invalid)?;
        if table
            .keys()
            .any(|k| !["cli", "ui", "models", "privacy", "marketplace"].contains(&k.as_str()))
            || table.get("models").is_some_and(|v| {
                v.as_table().is_none_or(|t| {
                    t.keys()
                        .any(|k| !["default", "default_reasoning_effort"].contains(&k.as_str()))
                })
            })
        {
            return Err(denied("Automatic Grok CLI renewal supports the standard first-party configuration only. Custom authentication, endpoints or managed policy require recovery in Grok itself."));
        }
    }
    Ok(())
}
fn command(exe: &Path, home: &Path, args: &[&str]) -> tokio::process::Command {
    let mut cmd = crate::codex::quiet_command(exe);
    // Remove overrides case-insensitively, including inline config, auth helpers and debug sinks.
    for (key, _) in std::env::vars_os() {
        let upper = key.to_string_lossy().to_ascii_uppercase();
        if ["GROK_", "XAI_", "OPENAI_", "ANTHROPIC_"]
            .iter()
            .any(|prefix| upper.starts_with(prefix))
            || ["NODE_OPTIONS", "RUST_LOG", "BROWSER"].contains(&upper.as_str())
        {
            cmd.env_remove(key);
        }
    }
    cmd.args(args)
        .current_dir(home)
        .env("GROK_HOME", home)
        .env("GROK_DISABLE_AUTOUPDATER", "1")
        .env("DISABLE_TELEMETRY", "1")
        .env("DISABLE_ERROR_REPORTING", "1")
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    cmd
}
async fn collect(input: impl tokio::io::AsyncRead + Unpin) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let mut data = Zeroizing::new(vec![]);
    input
        .take(65537)
        .read_to_end(&mut data)
        .await
        .map_err(|_| Fault::offline())?;
    if data.len() > 65536 {
        return Err(invalid());
    }
    Ok(data)
}
async fn run(
    exe: &Path,
    home: &Path,
    args: &[&str],
    seconds: u64,
) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let mut child = command(exe, home, args).spawn().map_err(|_| {
        Fault::new(
            FaultKind::ToolMissing,
            "The installed Grok CLI could not start.",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(invalid)?;
    let stderr = child.stderr.take().ok_or_else(invalid)?;
    let result = tokio::time::timeout(Duration::from_secs(seconds), async {
        let (out, _err, status) = tokio::try_join!(collect(stdout), collect(stderr), async { child.wait().await.map_err(|_| Fault::offline()) })?;
        if !status.success() {
            return Err(Fault::new(FaultKind::NeedsRelogin, "The official Grok models check could not restore the session. Use Sign in with Grok (device code). No additional login command or model prompt was started; raw CLI output was not exported."));
        }
        Ok(out)
    }).await.unwrap_or_else(|_| Err(Fault::new(FaultKind::Offline, "Grok CLI renewal timed out. Only its owned child was stopped; no additional login command was started.")));
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
    result
}
#[async_trait::async_trait]
impl Owner for Cli {
    async fn renew(&self, home: &Path) -> Result<(), Fault> {
        context(home)?;
        let exe = grok_personal::executable().ok_or_else(|| {
            Fault::new(
                FaultKind::ToolMissing,
                "The official Grok CLI is required for approved session renewal.",
            )
        })?;
        for path in exe.ancestors() {
            let meta = std::fs::symlink_metadata(path).map_err(|_| invalid())?;
            let linked = meta.file_type().is_symlink();
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                linked || meta.file_attributes() & 0x400 != 0
            };
            if linked {
                return Err(denied(
                    "Linked Grok executables are not supported for CLI renewal.",
                ));
            }
        }
        let before = std::fs::metadata(&exe).map_err(|_| invalid())?;
        let stamp = (before.len(), before.modified().ok());
        if !approved_version(&run(&exe, home, &["--version"], 5).await?) {
            return Err(denied("This Grok CLI version has not been checked for background renewal. Use explicit sign-in; no models command was run."));
        }
        let current = std::fs::metadata(&exe).map_err(|_| invalid())?;
        if stamp != (current.len(), current.modified().ok()) {
            return Err(Fault::changed());
        }
        run(&exe, home, &["models"], 25).await?;
        let current = std::fs::metadata(&exe).map_err(|_| invalid())?;
        if stamp != (current.len(), current.modified().ok()) {
            return Err(Fault::changed());
        }
        context(home)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_and_custom_auth_are_fail_closed() {
        assert!(approved_version(b"grok 1.0.67 (2fe064390d07)\r\n"));
        for v in [
            "grok 1.0.68 (2fe064390d07)",
            "grok 1.0.67 (other)",
            "1.0.67",
        ] {
            assert!(!approved_version(v.as_bytes()));
        }
        let t = tempfile::tempdir().unwrap();
        assert!(context(t.path()).is_ok());
        std::fs::write(
            t.path().join("config.toml"),
            "[models]\ndefault='test'\n[ui]\ncompact_mode=true",
        )
        .unwrap();
        assert!(context(t.path()).is_ok());
        for data in [
            "[auth]\nauth_provider_command='never-run'",
            "[grok_com_config.oidc]\nissuer='https://other.invalid'",
            "[models.custom]\nurl='https://other.invalid'",
            "[hooks]\ncommand='never-run'",
        ] {
            std::fs::write(t.path().join("config.toml"), data).unwrap();
            assert!(context(t.path()).is_err());
        }
    }
}
