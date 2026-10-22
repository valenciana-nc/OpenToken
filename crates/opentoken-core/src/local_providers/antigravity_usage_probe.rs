//! Explicit official CLI-profile quota reader and separate one-shot diagnosis.
//! Not an OAuth-account fallback. Raw stdout/stderr stay native; credentials stay provider-owned.
use super::*;
use tokio::io::AsyncReadExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageProbe {
    pub version: String,
    pub quota_count: usize,
    pub buckets: Vec<Quota>,
    pub zero_model_work: bool,
    pub identity_verified: bool,
}
const CLEARED: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GOOGLE_CLOUD_PROJECT",
    "GCLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "CLOUDSDK_AUTH_ACCESS_TOKEN",
    "ANTIGRAVITY_API_KEY",
    "ANTIGRAVITY_OAUTH_TOKEN",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "NODE_OPTIONS",
];
fn version(bytes: &[u8]) -> Result<String, Fault> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?.trim();
    if text.len() > 32 {
        return Err(invalid());
    }
    let parts = text
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    if parts.len() != 3
        || parts[0] != 1
        || (parts[1], parts[2]) < (2, 2)
        || (parts[1], parts[2]) > (3, 5)
    {
        return Err(denied("This installed agy version is not approved for the CLI-handled quota report. No print command was run."));
    }
    Ok(text.to_owned())
}
async fn collect(
    mut input: impl tokio::io::AsyncRead + Unpin,
    limit: usize,
) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let mut bytes = Zeroizing::new(Vec::new());
    (&mut input)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| Fault::offline())?;
    if bytes.len() > limit {
        return Err(Fault::new(
            FaultKind::Unreadable,
            "The official CLI diagnostic exceeded its output limit. Raw output was not exported.",
        ));
    }
    Ok(bytes)
}
async fn run(
    exe: &Path,
    home: &Path,
    isolated: bool,
    args: &[&str],
    deadline: Duration,
) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let mut command = crate::codex::quiet_command(exe);
    command
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if isolated {
        command
            .env("APPDATA", home.join("AppData/Roaming"))
            .env("LOCALAPPDATA", home.join("AppData/Local"))
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"));
    }
    for name in CLEARED {
        command.env_remove(name);
    }
    let mut child = command.spawn().map_err(|_| {
        Fault::new(
            FaultKind::ToolMissing,
            "The installed official agy diagnostic could not start.",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(invalid)?;
    let stderr = child.stderr.take().ok_or_else(invalid)?;
    let result=tokio::time::timeout(deadline,async{
        let(out,err,status)=tokio::try_join!(collect(stdout,2*1024*1024),collect(stderr,64*1024),async{child.wait().await.map_err(|_|Fault::offline())})?;
        if !status.success(){
            let message=Zeroizing::new(String::from_utf8_lossy(&err).to_ascii_lowercase());
            let auth=["authentication required","not logged in","invalid_grant","please log in","please sign in"].iter().any(|s|message.contains(s));let limited=["rate limit","too many requests","429"].iter().any(|s|message.contains(s));
            return Err(Fault::new(if limited{FaultKind::RateLimited}else if auth{FaultKind::NeedsRelogin}else{FaultKind::PermissionDenied},"The official agy quota diagnostic did not complete successfully. No alternate login or model prompt was started, and raw output was not exported."));
        }
        Ok(out)
    }).await.unwrap_or_else(|_|Err(Fault::new(FaultKind::Offline,"The official agy quota diagnostic timed out. Only its owned child was stopped; no connection was saved.")));
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
    result
}
#[derive(Deserialize)]
struct Envelope {
    status: String,
    num_turns: u64,
    usage: Counters,
    command: Command,
}
#[derive(Deserialize)]
struct Counters {
    input_tokens: u64,
    output_tokens: u64,
    thinking_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
}
#[derive(Deserialize)]
struct Command {
    name: String,
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    groups: Vec<Group>,
}
#[derive(Deserialize)]
struct Group {
    name: Option<String>,
    buckets: Vec<Bucket>,
}
#[derive(Deserialize)]
struct Bucket {
    id: String,
    remaining_fraction: Option<f64>,
    reset_time: Option<String>,
    #[serde(default)]
    disabled: bool,
}
fn parse(bytes: &[u8], version: String) -> Result<UsageProbe, Fault> {
    let report:Envelope=serde_json::from_slice(bytes).map_err(|_|Fault::new(FaultKind::Unreadable,"agy did not return the supported structured quota report. No raw response or account identity was exported."))?;
    let u = report.usage;
    if report.status != "SUCCESS"
        || report.command.name != "usage"
        || report.num_turns != 0
        || [
            u.input_tokens,
            u.output_tokens,
            u.thinking_tokens,
            u.cache_read_tokens,
            u.total_tokens,
        ]
        .iter()
        .any(|n| *n != 0)
    {
        return Err(denied("The CLI report did not prove a successful CLI-handled usage command with zero model work. Nothing was persisted."));
    }
    if report.command.data.groups.len() > 32 {
        return Err(invalid());
    }
    let mut groups = vec![];
    for group in report.command.data.groups {
        if group.buckets.len() > 128 {
            return Err(invalid());
        }
        groups.push(serde_json::json!({"displayName":group.name,"buckets":group.buckets.into_iter().map(|b|serde_json::json!({"bucketId":b.id,"remainingFraction":b.remaining_fraction,"resetTime":b.reset_time,"disabled":b.disabled})).collect::<Vec<_>>()}));
    }
    let quotas = super::antigravity::parse_summary(&serde_json::json!({"groups":groups}))?.quotas;
    Ok(UsageProbe {
        version,
        quota_count: quotas.len(),
        buckets: quotas,
        zero_model_work: true,
        identity_verified: false,
    })
}
impl Service {
    pub async fn connect_antigravity_cli_usage(
        &self,
        settings: &Settings,
        consent: bool,
    ) -> anyhow::Result<(Settings, AccountState)> {
        if !consent {
            return Err(denied(
                "Approve CLI-profile monitoring explicitly. No client was launched.",
            )
            .into());
        }
        let result = tokio::time::timeout(Duration::from_secs(60), async {
            let wait = self.verification_wait_secs(Provider::Antigravity);
            if wait > 0 && wait <= 15 {
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
            self.begin_verification(Provider::Antigravity)?;
            let path = super::antigravity_cli::profile(&self.storage).ok_or_else(|| {
                Fault::new(
                    FaultKind::ToolMissing,
                    "The installed agy CLI was not found. Nothing was installed.",
                )
            })?;
            let reading = self.read_antigravity_cli_usage(&path).await?;
            self.persist_local(
                settings,
                Provider::Antigravity,
                &path,
                reading,
                ConnectionMode::AntigravityCliUsage,
            )
        })
        .await
        .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(e) = &result {
            if let Some(f) = e.downcast_ref::<Fault>() {
                self.pause(Provider::Antigravity, f);
            }
        }
        result
    }
    pub(super) async fn read_antigravity_cli_usage(&self, path: &Path) -> Result<Reading, Fault> {
        if super::antigravity_cli::profile(&self.storage)
            .is_none_or(|p| crate::storage::path_key(&p) != crate::storage::path_key(path))
        {
            return Err(Fault::changed());
        }
        let probe = self.inspect_antigravity_cli_usage(true, true).await?;
        let now = Utc::now();
        let mut quotas = probe.buckets;
        quotas.retain(|q| q.resets_at.is_none_or(|reset| reset > now));
        if quotas.is_empty() {
            return Err(denied(
                "The official CLI reported no unexpired quota. No window was rolled forward.",
            ));
        }
        Ok(Reading {
            subject: format!(
                "cli-context:{}",
                hash(&format!(
                    "{}\n{}",
                    crate::storage::path_key(&self.storage.user_home),
                    crate::storage::path_key(path)
                ))
            ),
            login: "Antigravity CLI · current profile".into(),
            report: Report {
                plan: None,
                quotas,
                observed_at: now,
                verified_at: None,
                source: ReportSource::AntigravityCliUsage,
            },
        })
    }
    pub async fn inspect_antigravity_cli_usage(
        &self,
        consent: bool,
        allow_owner_recovery: bool,
    ) -> Result<UsageProbe, Fault> {
        if !consent || !allow_owner_recovery {
            return Err(denied("Explicit existing-source and official-client recovery approvals are required. No CLI was launched."));
        }
        let exe = super::antigravity_cli::profile(&self.storage).ok_or_else(|| {
            Fault::new(
                FaultKind::ToolMissing,
                "The installed official agy executable was not found. Nothing was installed.",
            )
        })?;
        let metadata = std::fs::metadata(&exe).map_err(|_| invalid())?;
        let stamp = (metadata.len(), metadata.modified().ok());
        let isolated = self.storage.user_home != Storage::system().user_home;
        let v = version(
            &run(
                &exe,
                &self.storage.user_home,
                isolated,
                &["--version"],
                Duration::from_secs(5),
            )
            .await?,
        )?;
        let current = std::fs::metadata(&exe).map_err(|_| invalid())?;
        if !crate::monitoring::safe_file(&exe) || (current.len(), current.modified().ok()) != stamp
        {
            return Err(Fault::changed());
        }
        let bytes = run(
            &exe,
            &self.storage.user_home,
            isolated,
            &["--print", "/usage", "--output-format", "json"],
            Duration::from_secs(35),
        )
        .await?;
        let current = std::fs::metadata(&exe).map_err(|_| invalid())?;
        if (current.len(), current.modified().ok()) != stamp {
            return Err(Fault::changed());
        }
        parse(&bytes, v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> Value {
        serde_json::json!({"status":"SUCCESS","num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0},"command":{"name":"usage","data":{"groups":[{"buckets":[{"id":"gemini-5h","remaining_fraction":0.75}]}]}}})
    }
    #[test]
    fn exact_zero_work_command_and_version_are_required() {
        assert_eq!(version(b"1.3.5\n").unwrap(), "1.3.5");
        for v in ["1.2.1", "2.0.0", "unknown", "1.3.5-beta"] {
            assert!(version(v.as_bytes()).is_err());
        }
        let mut v = report();
        let probe = parse(&serde_json::to_vec(&v).unwrap(), "1.3.5".into()).unwrap();
        assert_eq!(probe.buckets[0].percent, Some(25.));
        assert!(!probe.identity_verified);
        v["num_turns"] = serde_json::json!(1);
        assert!(parse(&serde_json::to_vec(&v).unwrap(), "1.3.5".into()).is_err());
        v = report();
        v["command"]["name"] = serde_json::json!("models");
        assert!(parse(&serde_json::to_vec(&v).unwrap(), "1.3.5".into()).is_err());
        v = report();
        v["usage"]["total_tokens"] = serde_json::json!(1);
        assert!(parse(&serde_json::to_vec(&v).unwrap(), "1.3.5".into()).is_err());
        assert!(parse(b"{}", "1.3.5".into()).is_err());
    }
    #[test]
    fn distinct_argon_pool_is_not_folded_into_the_shared_gemini_bucket() {
        let mut v = report();
        v["command"]["data"]["groups"].as_array_mut().unwrap().push(serde_json::json!({"name":"Gemini 4 Argon","buckets":[{"id":"gemini-5h","remaining_fraction":0.1}]}));
        let probe = parse(&serde_json::to_vec(&v).unwrap(), "1.3.5".into()).unwrap();
        assert_eq!(probe.quota_count, 1);
        assert_eq!(probe.buckets[0].percent, Some(25.));
    }
    #[tokio::test]
    async fn native_pipe_reads_are_bounded_without_exporting_contents() {
        assert!(collect(b"fixture-output".as_slice(), 3).await.is_err());
        assert_eq!(
            collect(b"safe".as_slice(), 4).await.unwrap().as_slice(),
            b"safe"
        );
    }
    #[tokio::test]
    async fn both_permissions_are_checked_before_process_or_filesystem_effects() {
        let root = tempfile::tempdir().unwrap();
        let s = Service::new(
            Storage {
                dir: root.path().join("data"),
                user_home: root.path().join("home"),
            },
            Arc::new(UnavailableStore),
        );
        for (read, recovery) in [(false, false), (true, false), (false, true)] {
            assert!(s
                .inspect_antigravity_cli_usage(read, recovery)
                .await
                .is_err());
        }
        assert!(!root.path().join("data").exists());
    }
}
