//! Muse Session Protocol observation reader. Never sends turns, auth or mint requests.
use super::*;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{FramedRead, LinesCodec};

fn executable(storage: &Storage) -> Option<PathBuf> {
    let name = if cfg!(windows) { "muse.exe" } else { "muse" };
    if storage.user_home != Storage::system().user_home {
        return Some(storage.user_home.join(".local/bin").join(name))
            .filter(|p| crate::monitoring::safe_file(p));
    }
    let mut paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .collect();
    paths.extend([
        storage.user_home.join(".local/bin").join(name),
        storage.user_home.join(".muse/bin").join(name),
    ]);
    paths.into_iter().find(|p| crate::monitoring::safe_file(p))
}
pub(super) fn credential(bytes: &[u8]) -> Result<String, Fault> {
    Ok(hash(&device_token(bytes)?))
}
pub(super) fn device_token(bytes: &[u8]) -> Result<Zeroizing<String>, Fault> {
    #[derive(Deserialize)]
    struct Auth {
        providers: Providers,
    }
    #[derive(Deserialize)]
    struct Providers {
        meta: Meta,
    }
    #[derive(Deserialize)]
    struct Meta {
        access_token: Option<String>,
        mechanism: Option<String>,
    }
    impl Drop for Meta {
        fn drop(&mut self) {
            if let Some(t) = &mut self.access_token {
                zeroize::Zeroize::zeroize(t);
            }
        }
    }
    let auth: Auth = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let meta = &auth.providers.meta;
    let token=meta.access_token.as_deref().filter(|t|t.starts_with("dca:")&&(16..=16384).contains(&t.len())&&t.bytes().all(|b|b.is_ascii_graphic())).ok_or_else(||denied("Muse's selected profile has no safely scoped inline subscription login. Local token tracking remains independent. No credential store was imported."))?;
    if meta.mechanism.as_deref().is_some_and(|s| s != "oauth") {
        return Err(denied("This Muse profile is not an OAuth subscription source. An inference key is not subscription quota access."));
    }
    Ok(Zeroizing::new(token.to_owned()))
}
impl Service {
    pub(super) async fn read_muse(&self, path: &Path) -> Result<Reading, Fault> {
        if !path.exists() {
            return Err(if executable(&self.storage).is_none() {
                Fault::new(FaultKind::ToolMissing,"No official Muse CLI installation or supported local subscription profile was found for this Windows user. A website sign-in or generic Meta API key is not a Muse CLI quota source. Nothing was installed and no login or key issuance was started.")
            } else {
                Fault::new(FaultKind::NotSignedIn,"Muse CLI is installed, but its supported subscription profile was not found. Open Muse and use its own authentication settings; no other credential store or browser session was imported.")
            });
        }
        let bytes = file(path).await?;
        let scope = credential(&bytes)?;
        #[cfg(test)]
        let exe = self
            .local
            .muse_executable
            .clone()
            .or_else(|| executable(&self.storage));
        #[cfg(not(test))]
        let exe = executable(&self.storage);
        let exe=exe.ok_or_else(||Fault::new(FaultKind::ToolMissing,"The installed official Muse executable was not found. Nothing was installed. Local token tracking is still available."))?;
        let parent = path.parent().ok_or_else(invalid)?;
        let mut command = crate::codex::quiet_command(&exe);
        command
            .arg("serve")
            .current_dir(&self.storage.dir)
            .env("MUSE_AUTH_PATH", path)
            .env("XDG_CONFIG_HOME", parent.parent().ok_or_else(invalid)?)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // No environment key may silently select a pay-as-you-go/alternate-provider route.
        for name in [
            "META_API_KEY",
            "MUSE_API_KEY",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_BASE_URL",
            "ANTHROPIC_BASE_URL",
            "META_API_BASE_URL",
            "MUSE_API_BASE_URL",
        ] {
            command.env_remove(name);
        }
        let mut child = command.spawn().map_err(|_| {
            Fault::new(
                FaultKind::ToolMissing,
                "Muse could not start its read-only protocol probe. No login was started.",
            )
        })?;
        let result=tokio::time::timeout(Duration::from_secs(12),async {
            let mut input=child.stdin.take().ok_or_else(invalid)?;
            let output=child.stdout.take().ok_or_else(invalid)?;
            let mut lines=FramedRead::new(output,LinesCodec::new_with_max_length(256*1024));
            let init=serde_json::json!({"jsonrpc":"2.0","id":"opentoken-init","method":"initialize","params":{"clientInfo":{"name":"opentoken","title":"OpenToken","version":env!("CARGO_PKG_VERSION")},"capabilities":{}}}).to_string()+"\n";
            input.write_all(init.as_bytes()).await.map_err(|_|Fault::offline())?;
            let mut initialized=false;let mut budget=0usize;
            while let Some(line)=lines.next().await {
                let line=Zeroizing::new(line.map_err(|_|invalid())?);budget=budget.saturating_add(line.len());if budget>1024*1024 {return Err(invalid());}
                let v:Value=serde_json::from_str(&line).map_err(|_|invalid())?;
                if v["id"]=="opentoken-init" {
                    if !v["error"].is_null()||!v["result"].is_object(){return Err(denied("This Muse version did not accept the usage observation protocol. No session or prompt was created."));}
                    initialized=true;
                    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":\"opentoken-usage\",\"method\":\"usage/read\",\"params\":{}}\n").await.map_err(|_|Fault::offline())?;
                } else if initialized&&v["id"]=="opentoken-usage" {
                    if !v["error"].is_null(){return Err(denied("Muse did not expose a subscription observation. Version 1.4 or newer is required; no minting fallback was attempted."));}
                    return parse(&v["result"]["usage"]);
                } else if initialized&&v["method"]=="usage/changed" {return parse(&v["params"]);}
                // Never answer approval/auth requests or invoke a session, turn, plugin or MCP method.
                if v["method"].is_string()&&!v["id"].is_null(){return Err(denied("Muse requested an interactive operation during the observation probe. No approval, login or prompt was sent."));}
            }
            Err(Fault::new(FaultKind::Offline,"Muse's observation host closed before a reading arrived."))
        }).await.unwrap_or_else(|_|Err(Fault::new(FaultKind::Offline,"Muse's observation probe timed out. No prompt or login was sent.")));
        let _ = child.kill().await;
        let _ = child.wait().await;
        if credential(&file(path).await?)? != scope || marker(&file(path).await?) != marker(&bytes)
        {
            return Err(Fault::changed());
        }
        Ok(Reading {
            subject: format!("credential:{scope}"),
            login: "Muse CLI · observed subscription".into(),
            report: result?,
        })
    }
}
pub(super) fn parse(value: &Value) -> Result<Report, Fault> {
    if value.is_null() {
        return Err(denied("Muse has no subscription observation for this host yet. A fresh host cannot read another terminal's private stream. Local tracking remains available; no prompt, login or key-mint fallback was requested."));
    }
    let observed = crate::parser::number(&value["observedAtMs"])
        .filter(|n| n.is_finite() && n.fract() == 0. && *n > 0. && *n < 9_007_199_254_740_991.)
        .and_then(|n| chrono::DateTime::from_timestamp_millis(n as i64))
        .ok_or_else(invalid)?;
    let now = Utc::now();
    if observed > now + chrono::Duration::seconds(5) || now - observed > chrono::Duration::hours(24)
    {
        return Err(denied("Muse's last subscription observation is too old or has an invalid timestamp. No current allowance was inferred."));
    }
    let mut quotas = vec![];
    for (key, label) in [
        ("window", "Subscription window"),
        ("weekly", "Weekly subscription"),
    ] {
        let row = &value[key];
        let used = crate::parser::number(&row["usedPercent"])
            .filter(|n| n.is_finite() && *n >= 0. && *n <= 1e9)
            .ok_or_else(invalid)?;
        let reset = crate::parser::number(&row["resetsAtMs"])
            .filter(|n| n.is_finite() && n.fract() == 0. && *n > 0. && *n < 64092211200000.)
            .and_then(|n| chrono::DateTime::from_timestamp_millis(n as i64))
            .ok_or_else(invalid)?;
        if reset <= now {
            continue;
        }
        let label = if key == "window" {
            let minutes = crate::parser::number(&row["windowDurationMins"])
                .filter(|n| n.is_finite() && n.fract() == 0. && (1.0..=10080.).contains(n))
                .ok_or_else(invalid)?;
            format!("{}-minute subscription window", minutes as u32)
        } else {
            label.into()
        };
        quotas.push(Quota {
            id: format!("muse_{key}"),
            label,
            percent: Some(used),
            value: None,
            unit: None,
            resets_at: Some(reset),
        });
    }
    if quotas.is_empty() {
        return Err(denied("Muse's observed windows have expired. A new reading is required; no reset was rolled forward and no full allowance was invented."));
    }
    Ok(Report {
        plan: value["tier"]
            .as_str()
            .filter(|s| text(s, 100))
            .map(str::to_owned),
        quotas,
        observed_at: observed,
        verified_at: Some(now),
        source: ReportSource::MuseCli,
    })
}
