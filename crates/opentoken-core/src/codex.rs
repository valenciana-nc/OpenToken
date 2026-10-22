use crate::{
    model::*,
    parser,
    storage::{normalize, path_key, read_json, Settings},
};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, process::Command};
use tokio_util::codec::{FramedRead, LinesCodec};

pub struct FileAuth {
    pub token: String,
    pub identity: Identity,
}
fn claim(v: &Value, key: &str) -> Option<String> {
    parser::text(&v["https://api.openai.com/auth"][key])
}
fn jwt(token: &str) -> Value {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 || parts[1].len() > 128 * 1024 {
        return Value::Null;
    }
    URL_SAFE_NO_PAD
        .decode(parts[1].trim_end_matches('='))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}
pub fn read_auth(home: &Path) -> Result<FileAuth, Fault> {
    let v: Value = read_json(&home.join("auth.json")).map_err(|_| {
        Fault::new(
            FaultKind::Unreadable,
            "The Codex login file could not be read. Try again after login finishes.",
        )
    })?;
    let tokens = &v["tokens"];
    let token = parser::text(&tokens["access_token"]).ok_or_else(|| {
        Fault::new(
            FaultKind::NotSignedIn,
            "No ChatGPT subscription login was found in this folder.",
        )
    })?;
    let id = jwt(tokens["id_token"].as_str().unwrap_or(""));
    let access = jwt(&token);
    if !id["https://api.openai.com/auth"].is_object()
        && !access["https://api.openai.com/auth"].is_object()
    {
        return Err(Fault::new(
            FaultKind::Unreadable,
            "The login file has no recognizable OpenAI identity claims.",
        ));
    }
    let ids: HashSet<_> = [
        parser::text(&tokens["account_id"]),
        claim(&id, "chatgpt_account_id"),
        claim(&access, "chatgpt_account_id"),
    ]
    .into_iter()
    .flatten()
    .map(|s| s.to_lowercase())
    .collect();
    if ids.len() != 1 {
        return Err(Fault::new(
            FaultKind::Unreadable,
            "The login file has missing or conflicting workspace identities. Sign in again.",
        ));
    }
    let email = parser::text(&id["email"])
        .or_else(|| parser::text(&access["email"]))
        .or_else(|| parser::text(&id["https://api.openai.com/profile"]["email"]))
        .or_else(|| parser::text(&access["https://api.openai.com/profile"]["email"]));
    let user = claim(&id, "chatgpt_user_id")
        .or_else(|| claim(&access, "chatgpt_user_id"))
        .or_else(|| parser::text(&id["sub"]));
    Ok(FileAuth {
        token,
        identity: Identity {
            workspace: ids.into_iter().next().unwrap(),
            email,
            user,
        },
    })
}
fn require_identity(home: &Path, expected: &Identity) -> Result<FileAuth, Fault> {
    let auth = read_auth(home).map_err(|_| Fault::changed())?;
    if !auth.identity.matches(expected) {
        return Err(Fault::changed());
    }
    Ok(auth)
}

#[derive(Debug)]
pub struct Reading {
    pub identity: Identity,
    pub plan: Option<String>,
    pub usage: Value,
}
#[async_trait]
pub trait CodexCli: Send + Sync {
    fn available(&self) -> bool;
    async fn read(&self, home: &Path) -> Result<Reading, Fault>;
}
pub struct CliClient {
    pub executable: Option<PathBuf>,
}
impl Default for CliClient {
    fn default() -> Self {
        Self {
            executable: find_cli(),
        }
    }
}
pub fn find_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    let mut paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .collect();
    if let Some(roaming) = std::env::var_os("APPDATA") {
        for base in [
            "npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor",
            "npm/node_modules/@openai/codex/vendor",
        ] {
            for suffix in [
                "x86_64-pc-windows-msvc/codex/codex.exe",
                "x86_64-pc-windows-msvc/bin/codex.exe",
            ] {
                paths.push(PathBuf::from(&roaming).join(base).join(suffix));
            }
        }
    }
    paths.into_iter().find(|p| {
        p.is_absolute()
            && std::fs::symlink_metadata(p)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    })
}
pub fn quiet_command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(exe);
    #[cfg(windows)]
    c.creation_flags(0x08000000); // CREATE_NO_WINDOW, never flash a console during refresh.
    c
}
#[async_trait]
impl CodexCli for CliClient {
    fn available(&self) -> bool {
        self.executable.is_some()
    }
    async fn read(&self, home: &Path) -> Result<Reading, Fault> {
        let executable = self.executable.as_ref().ok_or_else(|| {
            Fault::new(
                FaultKind::ToolMissing,
                "Install the Codex CLI to read Windows credential-store sign-ins.",
            )
        })?;
        let mut child = quiet_command(executable)
            .args(["app-server", "--listen", "stdio://"])
            .current_dir(home)
            .env("CODEX_HOME", home)
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_SQLITE_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| {
                Fault::new(
                    FaultKind::ToolMissing,
                    "Codex could not start. Check your CLI installation.",
                )
            })?;
        let result = tokio::time::timeout(Duration::from_secs(25), async {
            let mut input = child.stdin.take().unwrap();
            let mut output = FramedRead::new(child.stdout.take().unwrap(), LinesCodec::new_with_max_length(2*1024*1024));
            async fn request(input: &mut tokio::process::ChildStdin, output: &mut FramedRead<tokio::process::ChildStdout, LinesCodec>, id: u64, method: &str, params: Value) -> Result<Value,Fault> {
                input.write_all(format!("{}\n",json!({"id":id,"method":method,"params":params})).as_bytes()).await.map_err(|_|Fault::offline())?;
                input.flush().await.map_err(|_|Fault::offline())?;
                while let Some(line) = output.next().await { let line=line.map_err(|_|Fault::new(FaultKind::Unknown,"Codex returned an oversized or invalid message."))?;
                    let v:Value=serde_json::from_str(&line).map_err(|_|Fault::new(FaultKind::Unknown,"Codex returned invalid JSON."))?;
                    if v["id"].as_u64()!=Some(id) { continue; }
                    if !v["error"].is_null() { return Err(Fault::new(FaultKind::NeedsRelogin,"Codex could not read this sign-in. Open Codex for this account, then refresh.")); }
                    return Ok(v["result"].clone());
                }
                Err(Fault::new(FaultKind::Unknown,"Codex closed before returning usage."))
            }
            request(&mut input,&mut output,1,"initialize",json!({"clientInfo":{"name":"opentoken","version":"1.0.0"},"capabilities":{"experimentalApi":true}})).await?;
            input.write_all(b"{\"method\":\"initialized\",\"params\":{}}\n").await.map_err(|_|Fault::offline())?;
            let before=request(&mut input,&mut output,2,"account/read",json!({"refreshToken":false})).await?;
            if before["account"]["type"]!="chatgpt" { return Err(Fault::new(FaultKind::NotSignedIn,"This folder is not signed in with a ChatGPT subscription.")); }
            let usage=request(&mut input,&mut output,3,"account/rateLimits/read",json!({})).await?;
            let after=request(&mut input,&mut output,4,"account/read",json!({"refreshToken":false})).await?;
            for key in ["email","accountId","id"] { if parser::text(&before["account"][key]).map(|s|s.to_lowercase()) != parser::text(&after["account"][key]).map(|s|s.to_lowercase()) { return Err(Fault::changed()); } }
            if after["account"]["type"]!="chatgpt" { return Err(Fault::changed()); }
            let a=&before["account"];
            let workspace=parser::text(&a["accountId"]).or_else(||parser::text(&a["id"])).or_else(||parser::text(&usage["accountId"])).unwrap_or_else(||format!("home:{}",hash(&path_key(home))));
            Ok(Reading { identity: Identity { workspace, email:parser::text(&a["email"]),user:None },plan:parser::text(&a["planType"]),usage })
        }).await.unwrap_or_else(|_|Err(Fault::new(FaultKind::Offline,"Codex account read timed out. Try Refresh again.")));
        let _ = child.kill().await;
        let _ = child.wait().await;
        result
    }
}

pub struct Discovery {
    pub accounts: Vec<Account>,
    pub folders: Vec<FolderStatus>,
    pub prefetched: HashMap<String, Report>,
}
struct Probe {
    stamp: String,
    time: Instant,
    result: Result<(Identity, Option<Report>), Fault>,
}
pub struct CodexAdapter {
    pub cli: Box<dyn CodexCli>,
    pub http: reqwest::Client,
    probes: HashMap<String, Probe>,
    pub user_home: PathBuf,
    pub override_home: Option<PathBuf>,
}
impl CodexAdapter {
    pub fn new(user_home: PathBuf, http: reqwest::Client) -> Self {
        Self {
            cli: Box::<CliClient>::default(),
            http,
            probes: HashMap::new(),
            user_home,
            override_home: std::env::var_os("CODEX_HOME").map(PathBuf::from),
        }
    }
    pub fn candidate_homes(&self, s: &Settings) -> Vec<PathBuf> {
        let default = self
            .override_home
            .clone()
            .unwrap_or_else(|| self.user_home.join(".codex"));
        let mut homes: Vec<PathBuf> = s
            .codex_homes
            .iter()
            .map(|p| normalize(p, &self.user_home))
            .collect();
        if homes.is_empty() || s.auto_discover {
            homes.push(default);
        }
        if s.auto_discover {
            homes.extend([
                self.user_home.join(".codex"),
                self.user_home.join(".config/codex"),
            ]);
            for parent in [&self.user_home, &self.user_home.join(".config")] {
                for dir in directories(parent) {
                    let name = dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    if name.contains("codex") {
                        homes.push(dir.clone());
                        if ["accounts", "t3", "swap", "switch"]
                            .iter()
                            .any(|s| name.contains(s))
                        {
                            add_children(&dir, 3, &mut homes);
                        }
                    }
                }
            }
        }
        let ignored: HashSet<_> = s
            .ignored_homes
            .iter()
            .map(|p| path_key(&normalize(p, &self.user_home)))
            .collect();
        let mut seen = HashSet::new();
        homes
            .into_iter()
            .filter(|p| {
                let k = path_key(p);
                !ignored.contains(&k) && seen.insert(k)
            })
            .take(256)
            .collect()
    }
    pub async fn discover(&mut self, s: &Settings, force: bool) -> Discovery {
        self.discover_gated(s, force, &HashSet::new()).await
    }
    pub async fn discover_gated(
        &mut self,
        s: &Settings,
        force: bool,
        blocked: &HashSet<String>,
    ) -> Discovery {
        let mut result = Discovery {
            accounts: vec![],
            folders: vec![],
            prefetched: HashMap::new(),
        };
        let mut grouped: BTreeMap<String, Account> = BTreeMap::new();
        for home in self.candidate_homes(s) {
            let configured = s
                .codex_homes
                .iter()
                .any(|p| path_key(&normalize(p, &self.user_home)) == path_key(&home));
            if !home.is_dir() {
                if configured {
                    result.folders.push(folder(
                        &home,
                        "awaiting_login",
                        "This folder does not exist yet. Log in to connect it.",
                        None,
                    ));
                }
                continue;
            }
            let mode = match storage_mode(&home) {
                Ok(mode) => mode,
                Err(fault) => {
                    result
                        .folders
                        .push(folder(&home, "unreadable", &fault.message, None));
                    continue;
                }
            };
            if mode == "ephemeral" {
                if configured {
                    result.folders.push(folder(&home,"ephemeral","This login exists only in a running Codex session and cannot be monitored.",None));
                }
                continue;
            }
            let keyring = mode == "keyring"
                || mode == "auto"
                || (!home.join("auth.json").is_file()
                    && (configured
                        || self.override_home.as_ref() == Some(&home)
                        || home == self.user_home.join(".codex")));
            if !keyring && !home.join("auth.json").exists() {
                continue;
            }
            let key = path_key(&home);
            let stamp = stamp(&home);
            let ttl = if keyring { 60 } else { 3600 };
            let cached = self.probes.get(&key).filter(|p| {
                p.stamp == stamp
                    && (blocked.contains(&key)
                        || (!force
                            && p.time.elapsed()
                                < Duration::from_secs(match &p.result {
                                    Err(f) => {
                                        ttl.max(f.retry_after_secs.unwrap_or(60).min(32 * 86400))
                                    }
                                    Ok(_) => ttl,
                                })))
            });
            let probe = if let Some(p) = cached {
                p.result.clone().map(|(id, _)| (id, None))
            } else {
                let value = if keyring {
                    match self.cli.read(&home).await {
                        Ok(r) => {
                            let report = parser::codex_cli(&r.usage, r.plan, Utc::now()).ok();
                            Ok((r.identity, report))
                        }
                        Err(f) => Err(f),
                    }
                } else {
                    read_auth(&home).map(|a| (a.identity, None))
                };
                self.probes.insert(
                    key,
                    Probe {
                        stamp,
                        time: Instant::now(),
                        result: value.clone(),
                    },
                );
                value
            };
            match probe {
                Ok((identity, report)) => {
                    let id = identity.id();
                    let email = identity.email.clone();
                    let label = s.aliases.get(&id).cloned().unwrap_or_else(|| {
                        format!("Codex · {}", email.as_deref().unwrap_or("Account"))
                    });
                    grouped
                        .entry(id.clone())
                        .or_insert_with(|| Account {
                            id: id.clone(),
                            provider: Provider::Codex,
                            label,
                            email,
                            identity: Some(identity),
                            sources: vec![],
                        })
                        .sources
                        .push(Source {
                            path: home.clone(),
                            store: if keyring { Store::Keyring } else { Store::File },
                        });
                    if let Some(report) = report {
                        result.prefetched.insert(id.clone(), report);
                    }
                    result
                        .folders
                        .push(folder(&home, "connected", "Signed in", Some(id)));
                }
                Err(f) => result.folders.push(folder(
                    &home,
                    if f.kind == FaultKind::NotSignedIn {
                        "awaiting_login"
                    } else {
                        "unavailable"
                    },
                    &f.message,
                    None,
                )),
            }
        }
        result.accounts = grouped.into_values().collect();
        result
    }
    pub async fn fetch(&self, account: &Account) -> Result<Report, Fault> {
        let expected = account.identity.as_ref().ok_or_else(Fault::changed)?;
        let mut last = Fault::new(FaultKind::NotSignedIn, "No verified sign-in was found.");
        let mut changed = false;
        for source in &account.sources {
            match self.fetch_source(source, expected).await {
                Ok(r) => return Ok(r),
                Err(f) => {
                    changed |= f.kind == FaultKind::IdentityChanged;
                    last = f;
                }
            }
        }
        if changed {
            Err(Fault::changed())
        } else {
            Err(last)
        }
    }
    async fn fetch_source(&self, source: &Source, expected: &Identity) -> Result<Report, Fault> {
        if source.store == Store::File {
            require_identity(&source.path, expected)?;
        }
        if self.cli.available() {
            match self.cli.read(&source.path).await {
                Ok(r) => {
                    if (!r.identity.workspace.starts_with("home:")
                        && r.identity.workspace.to_lowercase() != expected.workspace.to_lowercase())
                        || (r.identity.email.is_some()
                            && expected.email.is_some()
                            && r.identity.email.as_ref().unwrap().to_lowercase()
                                != expected.email.as_ref().unwrap().to_lowercase())
                    {
                        return Err(Fault::changed());
                    }
                    if source.store == Store::File {
                        require_identity(&source.path, expected)?;
                    }
                    return parser::codex_cli(&r.usage, r.plan, Utc::now());
                }
                Err(f) if f.kind == FaultKind::IdentityChanged => return Err(f),
                Err(f) if source.store == Store::Keyring => return Err(f),
                Err(_) => {}
            }
        }
        if source.store == Store::Keyring {
            return Err(Fault::new(
                FaultKind::ToolMissing,
                "Install Codex CLI to read this credential-store sign-in.",
            ));
        }
        let auth = require_identity(&source.path, expected)?;
        let request = |url: &str| {
            self.http
                .get(url)
                .bearer_auth(&auth.token)
                .header("ChatGPT-Account-Id", &expected.workspace)
                .header("User-Agent", "OpenToken/1.0.0")
        };
        let response = request("https://chatgpt.com/backend-api/wham/usage")
            .send()
            .await
            .map_err(|_| Fault::offline())?;
        check_response(&response)?;
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (k.as_str().to_lowercase(), v.to_string()))
            })
            .collect();
        let v = bounded_json(response).await?;
        for (key, expected) in [
            ("account_id", Some(expected.workspace.as_str())),
            ("email", expected.email.as_deref()),
        ] {
            if let (Some(actual), Some(want)) = (v[key].as_str(), expected) {
                if !actual.eq_ignore_ascii_case(want) {
                    return Err(Fault::changed());
                }
            }
        }
        let resets = match request("https://chatgpt.com/backend-api/wham/rate-limit-reset-credits")
            .header("OpenAI-Beta", "codex-1")
            .header("originator", "Codex Desktop")
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => bounded_json(r).await.ok(),
            _ => None,
        };
        require_identity(&source.path, expected)?;
        parser::codex_http(&v, &headers, resets.as_ref(), Utc::now())
    }
}
pub fn check_response(r: &reqwest::Response) -> Result<(), Fault> {
    match r.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(Fault::new(
            FaultKind::NeedsRelogin,
            "This sign-in needs renewal. Log in again with the provider CLI.",
        )),
        429 => Err(Fault {
            kind: FaultKind::RateLimited,
            message: "The provider rate-limited updates. OpenToken will wait before retrying."
                .into(),
            retry_after_secs: r
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
        }),
        _ => Err(Fault::offline()),
    }
}
pub async fn bounded_json(response: reqwest::Response) -> Result<Value, Fault> {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| Fault::offline())?;
        if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err(Fault::new(
                FaultKind::Unknown,
                "The provider response exceeded the safety limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| Fault::new(FaultKind::Unknown, "The provider returned invalid JSON."))
}
fn folder(home: &Path, status: &str, message: &str, id: Option<String>) -> FolderStatus {
    FolderStatus {
        path: home.into(),
        status: status.into(),
        message: message.into(),
        account_id: id,
    }
}
pub fn stamp(home: &Path) -> String {
    ["auth.json", "config.toml", ".credentials.json"]
        .iter()
        .map(|p| {
            std::fs::metadata(home.join(p))
                .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("|")
}
fn storage_mode(home: &Path) -> Result<String, Fault> {
    let path = home.join("config.toml");
    if !path.exists() {
        return Ok("file".into());
    }
    let fail = || {
        Fault::new(
            FaultKind::Unreadable,
            "Codex config.toml could not be safely parsed. No stale file login was used.",
        )
    };
    if std::fs::metadata(&path).map_err(|_| fail())?.len() > 1024 * 1024 {
        return Err(fail());
    }
    let config: toml::Value = std::fs::read_to_string(path)
        .map_err(|_| fail())?
        .parse()
        .map_err(|_| fail())?;
    let mode = config
        .get("cli_auth_credentials_store")
        .and_then(|v| v.as_str())
        .unwrap_or("file");
    if !["file", "keyring", "auto", "ephemeral"].contains(&mode) {
        return Err(fail());
    }
    Ok(mode.to_string())
}
fn directories(home: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(home)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()))
        .map(|e| e.path())
        .collect()
}
fn add_children(home: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if depth == 0 || out.len() > 256 {
        return;
    }
    for dir in directories(home) {
        if [
            "sessions",
            "archived_sessions",
            "skills",
            "node_modules",
            ".git",
        ]
        .contains(
            &dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ) {
            continue;
        }
        out.push(dir.clone());
        add_children(&dir, depth - 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn auth(home: &Path, workspace: &str, email: &str) {
        std::fs::create_dir_all(home).unwrap();
        let claims =
            json!({"email":email,"https://api.openai.com/auth":{"chatgpt_account_id":workspace}});
        let token = format!(
            "e30.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        std::fs::write(
            home.join("auth.json"),
            json!({"tokens":{"access_token":token,"account_id":workspace}}).to_string(),
        )
        .unwrap();
    }
    #[test]
    fn conflicts_and_unrelated_tokens_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        auth(t.path(), "a", "a@test");
        let mut v: Value = read_json(&t.path().join("auth.json")).unwrap();
        v["tokens"]["account_id"] = json!("b");
        std::fs::write(t.path().join("auth.json"), v.to_string()).unwrap();
        assert!(read_auth(t.path()).is_err());
    }
    #[test]
    fn identity_is_workspace_and_user_not_folder() {
        let a = Identity {
            workspace: "A".into(),
            email: Some("A@test".into()),
            user: None,
        };
        let b = Identity {
            workspace: "a".into(),
            email: Some("a@test".into()),
            user: None,
        };
        assert!(a.matches(&b));
        assert_ne!(
            a.id(),
            Identity {
                workspace: "b".into(),
                ..b
            }
            .id()
        );
    }
    #[tokio::test]
    async fn scans_siblings_nested_deduplicates_and_detects_switch() {
        let t = tempfile::tempdir().unwrap();
        auth(&t.path().join(".codex"), "a", "a@test");
        auth(&t.path().join(".codex-copy"), "a", "A@test");
        auth(&t.path().join(".codex-t3/work/home"), "b", "b@test");
        let mut adapter = CodexAdapter::new(t.path().into(), reqwest::Client::new());
        adapter.override_home = None;
        let s = Settings::default();
        let d = adapter.discover(&s, false).await;
        assert_eq!(d.accounts.len(), 2);
        assert_eq!(
            d.accounts
                .iter()
                .find(|a| a.email.as_deref() == Some("a@test"))
                .unwrap()
                .sources
                .len(),
            2
        );
        auth(&t.path().join(".codex"), "c", "c@test");
        assert_eq!(adapter.discover(&s, true).await.accounts.len(), 3);
    }
    #[tokio::test]
    async fn ignores_and_ephemeral_stores() {
        let t = tempfile::tempdir().unwrap();
        auth(&t.path().join(".codex"), "a", "a@test");
        auth(&t.path().join(".codex-other"), "b", "b@test");
        let mut a = CodexAdapter::new(t.path().into(), reqwest::Client::new());
        a.override_home = None;
        let s = Settings {
            auto_discover: false,
            ..Default::default()
        };
        assert_eq!(a.discover(&s, false).await.accounts.len(), 1);
        std::fs::write(
            t.path().join(".codex/config.toml"),
            "cli_auth_credentials_store='ephemeral'",
        )
        .unwrap();
        assert_eq!(a.discover(&s, true).await.accounts.len(), 0);
    }
    struct FakeCli;
    #[async_trait]
    impl CodexCli for FakeCli {
        fn available(&self) -> bool {
            true
        }
        async fn read(&self, _: &Path) -> Result<Reading, Fault> {
            Ok(Reading {
                identity: Identity {
                    workspace: "actual".into(),
                    email: Some("actual@test".into()),
                    user: None,
                },
                plan: None,
                usage: json!({"rateLimits":{"primary":{"usedPercent":25}}}),
            })
        }
    }
    #[tokio::test]
    async fn auto_store_ignores_stale_file_and_reuses_usage() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join(".codex");
        auth(&p, "stale", "stale@test");
        std::fs::write(p.join("config.toml"), "cli_auth_credentials_store='auto'").unwrap();
        let mut a = CodexAdapter::new(t.path().into(), reqwest::Client::new());
        a.override_home = None;
        a.cli = Box::new(FakeCli);
        let d = a.discover(&Settings::default(), false).await;
        assert_eq!(d.accounts[0].email.as_deref(), Some("actual@test"));
        assert_eq!(d.prefetched.len(), 1);
    }
    #[tokio::test]
    async fn wrong_cli_cannot_feed_file_account() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join(".codex");
        auth(&p, "a", "a@test");
        let mut a = CodexAdapter::new(t.path().into(), reqwest::Client::new());
        a.override_home = None;
        a.cli = Box::new(FakeCli);
        let d = a.discover(&Settings::default(), false).await;
        assert_eq!(
            a.fetch(&d.accounts[0]).await.unwrap_err().kind,
            FaultKind::IdentityChanged
        );
    }
}
