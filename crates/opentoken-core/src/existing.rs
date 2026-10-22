//! Connect only existing data. Deliberately has no browser/CLI-login capability.
use crate::{
    claude::ClaudeAdapter,
    codex::CodexAdapter,
    connections::Service,
    model::*,
    storage::{path_key, Settings, Storage},
};
use anyhow::Result;
use std::time::Duration;

pub(crate) async fn connect(
    storage: &Storage,
    current: &Settings,
    service: &Service,
    codex: &mut CodexAdapter,
    claude: &ClaudeAdapter,
    provider: Provider,
    consent: bool,
) -> Result<(Settings, Vec<AccountState>)> {
    if !consent {
        anyhow::bail!("Allow read-only use of existing usage sources explicitly.");
    }
    match provider {
        Provider::Cursor => {
            let (settings, account) = service.connect_cursor_personal(current, true).await?;
            Ok((settings, vec![account]))
        }
        Provider::Copilot => {
            let (settings, account) = service.connect_github_personal(current, true).await?;
            Ok((settings, vec![account]))
        }
        Provider::Grok => {
            let (settings, account) = service.connect_grok_personal(current, true).await?;
            Ok((settings, vec![account]))
        }
        Provider::Codex | Provider::Claude => {
            connect_cli(storage, current, service, codex, claude, provider).await
        }
        Provider::Muse | Provider::Devin | Provider::Antigravity => {
            let (settings, account) = service
                .connect_local_provider(current, provider, true)
                .await?;
            Ok((settings, vec![account]))
        }
        Provider::Zai | Provider::Deepseek | Provider::Kimi | Provider::Opencode => {
            if !current.connections.iter().any(|c| c.provider == provider) {
                return Err(Fault::new(
                    FaultKind::PermissionDenied,
                    "This provider needs its scoped API credential. A generic sign-in cannot supply quota access.",
                ).into());
            }
            service.begin_verification(provider)?;
            let result = tokio::time::timeout(Duration::from_secs(45), async {
                let mut states = vec![];
                for connection in current
                    .connections
                    .iter()
                    .filter(|c| c.provider == provider)
                {
                    let report = service.fetch(connection).await?;
                    states.push(AccountState::apply(
                        connection.account(current),
                        Ok(report),
                        None,
                    ));
                }
                Ok((current.clone(), states))
            })
            .await
            .unwrap_or_else(|_| Err(Fault::offline().into()));
            pause_failure(service, provider, &result);
            result
        }
    }
}

async fn connect_cli(
    storage: &Storage,
    current: &Settings,
    service: &Service,
    codex: &mut CodexAdapter,
    claude: &ClaudeAdapter,
    provider: Provider,
) -> Result<(Settings, Vec<AccountState>)> {
    service.begin_verification(provider)?;
    let homes = crate::monitoring::cli_homes(storage, current, provider);
    let mut probe = current.clone();
    let selected = homes
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    match provider {
        Provider::Codex => {
            probe.codex_enabled = true;
            probe.codex_homes = selected;
        }
        _ => {
            probe.claude_enabled = true;
            probe.claude_homes = selected;
        }
    }
    let result = tokio::time::timeout(Duration::from_secs(55), async {
        let mut verified = vec![];
        let mut failure = None;
        if provider == Provider::Codex {
            let mut found = codex.discover(&probe, true).await;
            for account in found.accounts {
                let report = match found.prefetched.remove(&account.id) {
                    Some(report) => Ok(report),
                    None => codex.fetch(&account).await,
                };
                match report {
                    Ok(report) => verified.push(AccountState::apply(account, Ok(report), None)),
                    Err(fault) => failure = Some(fault),
                }
            }
        } else {
            for mut account in claude.monitored_accounts(&probe) {
                if !account.sources.iter().any(|s| crate::monitoring::safe_file(&s.path.join(".credentials.json"))) {
                    continue;
                }
                // Existing-session connection cannot renew or replace the CLI's credentials.
                match claude.fetch_scoped(&account, false).await {
                    Ok((report, marker)) => {
                        ClaudeAdapter::bind_scope(&mut account, &marker);
                        verified.push(AccountState::apply(account, Ok(report), None));
                    }
                    Err(fault) => failure = Some(fault),
                }
            }
        }
        if verified.is_empty() {
            return Err(failure.unwrap_or_else(|| Fault::new(
                FaultKind::NotSignedIn,
                "No usable existing subscription session was found. Sign in through the official app/CLI only if needed.",
            )).into());
        }
        let mut next = current.clone();
        let previously_enabled = if provider == Provider::Codex { current.codex_enabled } else { current.claude_enabled };
        let target = if provider == Provider::Codex {
            next.codex_enabled = true;
            &mut next.codex_homes
        } else {
            next.claude_enabled = true;
            &mut next.claude_homes
        };
        // Materializing an implicit list must preserve previously monitored defaults,
        // but must not introduce unrelated defaults for a newly enabled provider.
        if target.is_empty() && previously_enabled {
            for home in &homes { append_home(target, home, storage); }
        }
        for state in &verified {
            for source in &state.account.sources { append_home(target, &source.path, storage); }
        }
        storage.save_settings(&next)?;
        Ok((next, verified))
    }).await.unwrap_or_else(|_| Err(Fault::offline().into()));
    pause_failure(service, provider, &result);
    result
}

fn append_home(homes: &mut Vec<String>, home: &std::path::Path, storage: &Storage) {
    if !homes
        .iter()
        .any(|p| path_key(&crate::storage::normalize(p, &storage.user_home)) == path_key(home))
    {
        homes.push(home.to_string_lossy().into_owned());
    }
}
fn pause_failure(
    service: &Service,
    provider: Provider,
    result: &Result<(Settings, Vec<AccountState>)>,
) {
    if let Err(error) = result {
        if let Some(fault) = error.downcast_ref::<Fault>() {
            service.pause(provider, fault);
        }
    }
}
#[cfg(test)]
#[path = "existing_tests.rs"]
mod tests;
