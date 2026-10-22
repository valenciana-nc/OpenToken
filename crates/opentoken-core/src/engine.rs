use crate::existing::connect as connect_existing;
use crate::{
    claude::ClaudeAdapter,
    codex::CodexAdapter,
    model::*,
    storage::{atomic_json, read_json, Settings, Storage},
};
use anyhow::Result;
use chrono::Utc;
use futures_util::StreamExt;
use notify::{RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::Instant;

pub enum Command {
    Refresh,
    Existing(
        Provider,
        bool,
        bool,
        tokio_util::sync::CancellationToken,
        oneshot::Sender<Result<crate::monitoring::Outcome>>,
    ),
    History(
        Provider,
        bool,
        bool,
        oneshot::Sender<Result<crate::monitoring::Outcome>>,
    ),
    Changed,
    CursorChanged,
    CursorPersonal(bool, oneshot::Sender<Result<Settings>>),
    CursorProbe(bool, oneshot::Sender<std::result::Result<String, Fault>>),
    GithubProbe(bool, oneshot::Sender<std::result::Result<String, Fault>>),
    GrokProbe(bool, oneshot::Sender<std::result::Result<String, Fault>>),
    VerificationWait(Provider, oneshot::Sender<u64>),
    GrokPersonal(bool, oneshot::Sender<Result<Settings>>),
    AntigravityProbe(
        crate::connections::AntigravityClient,
        bool,
        oneshot::Sender<std::result::Result<String, Fault>>,
    ),
    AntigravityPersonal(
        crate::connections::AntigravityClient,
        bool,
        oneshot::Sender<Result<Settings>>,
    ),
    GithubPersonal(bool, oneshot::Sender<Result<Settings>>),
    CliConnect(
        crate::cli_profiles::Profile,
        oneshot::Sender<Result<Settings>>,
    ),
    CliDisconnect(String, oneshot::Sender<Result<Settings>>),
    Activity(Box<crate::activity::Activity>),
    ApiStatus(Option<String>),
    Settings(Box<Settings>, oneshot::Sender<Result<()>>),
    AccountOrder(Vec<String>, oneshot::Sender<Result<()>>),
    DashboardPinned(bool, oneshot::Sender<Result<()>>),
    Connect(
        crate::connections::ConnectInput,
        oneshot::Sender<Result<Settings>>,
    ),
    Disconnect(String, oneshot::Sender<Result<Settings>>),
    Shutdown(oneshot::Sender<()>),
}
#[derive(Clone)]
pub struct Engine {
    pub commands: mpsc::Sender<Command>,
    pub state: watch::Receiver<Snapshot>,
    pub settings: watch::Receiver<Settings>,
    pub tracking: crate::tracking::Tracker,
    pub sources: watch::Receiver<crate::monitoring::Inventory>,
    existing_runs:
        std::sync::Arc<std::sync::Mutex<HashMap<Provider, tokio_util::sync::CancellationToken>>>,
}
impl Engine {
    /// Read-only API embedding/test handle; starts no engine, tracker or credential reader.
    pub fn from_receivers(
        commands: mpsc::Sender<Command>,
        state: watch::Receiver<Snapshot>,
        settings: watch::Receiver<Settings>,
    ) -> Self {
        Self {
            commands,
            state,
            settings,
            tracking: crate::tracking::Tracker::disabled(),
            sources: watch::channel(crate::monitoring::Inventory::default()).1,
            existing_runs: Default::default(),
        }
    }
    pub fn start(storage: Storage, settings: Settings) -> Self {
        Self::start_with_credentials(
            storage,
            settings,
            std::sync::Arc::new(crate::connections::UnavailableStore),
        )
    }
    pub fn start_with_credentials(
        storage: Storage,
        settings: Settings,
        vault: std::sync::Arc<dyn crate::connections::CredentialStore>,
    ) -> Self {
        let connections = crate::connections::Service::new(storage.clone(), vault);
        Self::start_with_service(storage, settings, connections)
    }
    pub(crate) fn start_with_service(
        storage: Storage,
        settings: Settings,
        connections: crate::connections::Service,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let (state_tx, state) = watch::channel(Snapshot::default());
        let (settings_tx, settings_rx) = watch::channel(settings.clone());
        let (sources_tx, sources) = watch::channel(crate::monitoring::Inventory::default());
        tokio::spawn(run(
            storage.clone(),
            settings,
            tx.clone(),
            rx,
            state_tx,
            settings_tx,
            connections,
            sources_tx,
        ));
        let tracking = crate::tracking::Tracker::start(
            storage,
            settings_rx.clone(),
            state.clone(),
            tx.clone(),
        );
        Self {
            commands: tx,
            state,
            settings: settings_rx,
            tracking,
            sources,
            existing_runs: Default::default(),
        }
    }
    pub async fn connect_existing(
        &self,
        provider: Provider,
        consent: bool,
    ) -> Result<crate::monitoring::Outcome> {
        self.connect_existing_mode(provider, consent, false).await
    }
    pub async fn connect_muse_subscription(
        &self,
        consent: bool,
    ) -> Result<crate::monitoring::Outcome> {
        self.connect_existing_mode(Provider::Muse, consent, true)
            .await
    }
    pub async fn connect_antigravity_cli_usage(
        &self,
        consent: bool,
    ) -> Result<crate::monitoring::Outcome> {
        self.connect_existing_mode(Provider::Antigravity, consent, true)
            .await
    }
    async fn connect_existing_mode(
        &self,
        provider: Provider,
        consent: bool,
        muse_key_endpoint: bool,
    ) -> Result<crate::monitoring::Outcome> {
        let cancel = tokio_util::sync::CancellationToken::new();
        {
            let mut runs = self.existing_runs.lock().unwrap();
            if runs.contains_key(&provider) {
                anyhow::bail!("An existing-source check for this provider is already running.");
            }
            runs.insert(provider, cancel.clone());
        }
        let (tx, rx) = oneshot::channel();
        let result = if self
            .commands
            .send(Command::Existing(
                provider,
                consent,
                muse_key_endpoint,
                cancel,
                tx,
            ))
            .await
            .is_ok()
        {
            rx.await.map_err(anyhow::Error::from).and_then(|r| r)
        } else {
            Err(anyhow::anyhow!("The monitoring engine is unavailable."))
        };
        self.existing_runs.lock().unwrap().remove(&provider);
        result
    }
    pub fn cancel_existing(&self, provider: Provider) {
        if let Some(token) = self.existing_runs.lock().unwrap().get(&provider) {
            token.cancel();
        }
    }
    pub async fn set_history_source(
        &self,
        provider: Provider,
        enabled: bool,
        consent: bool,
    ) -> Result<crate::monitoring::Outcome> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::History(provider, enabled, consent, tx))
            .await?;
        rx.await?
    }
    pub async fn refresh(&self) {
        let _ = self.commands.send(Command::Refresh).await;
    }
    pub async fn save(&self, s: Settings) -> Result<()> {
        s.validate()?;
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Settings(Box::new(s), tx))
            .await?;
        rx.await?
    }
    pub async fn save_account_order(&self, order: Vec<String>) -> Result<()> {
        crate::storage::validate_account_order(&order)?;
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::AccountOrder(order, tx)).await?;
        rx.await?
    }
    pub async fn save_dashboard_pin(&self, pinned: bool) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::DashboardPinned(pinned, tx))
            .await?;
        rx.await?
    }
    pub async fn connect(&self, input: crate::connections::ConnectInput) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Connect(input, tx)).await?;
        rx.await?
    }
    pub async fn cursor_sign_in_marker(&self, consent: bool) -> std::result::Result<String, Fault> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::CursorProbe(consent, tx))
            .await
            .map_err(|_| {
                Fault::new(FaultKind::Unreadable, "The sign-in service is unavailable.")
            })?;
        rx.await
            .map_err(|_| Fault::new(FaultKind::Unreadable, "The sign-in service stopped."))?
    }
    pub async fn github_sign_in_marker(&self, consent: bool) -> std::result::Result<String, Fault> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::GithubProbe(consent, tx))
            .await
            .map_err(|_| Fault::offline())?;
        rx.await.map_err(|_| Fault::offline())?
    }
    pub async fn verification_wait_secs(&self, provider: Provider) -> u64 {
        let (tx, rx) = oneshot::channel();
        if self
            .commands
            .send(Command::VerificationWait(provider, tx))
            .await
            .is_err()
        {
            return 60;
        }
        rx.await.unwrap_or(60)
    }
    pub async fn grok_sign_in_marker(&self, consent: bool) -> std::result::Result<String, Fault> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::GrokProbe(consent, tx))
            .await
            .map_err(|_| Fault::offline())?;
        rx.await.map_err(|_| Fault::offline())?
    }
    pub async fn antigravity_recovery_marker(
        &self,
        client: crate::connections::AntigravityClient,
        consent: bool,
    ) -> std::result::Result<String, Fault> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::AntigravityProbe(client, consent, tx))
            .await
            .map_err(|_| Fault::offline())?;
        rx.await.map_err(|_| Fault::offline())?
    }
    pub async fn connect_antigravity_client(
        &self,
        client: crate::connections::AntigravityClient,
        consent: bool,
    ) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::AntigravityPersonal(client, consent, tx))
            .await?;
        rx.await?
    }
    pub async fn connect_grok_personal(&self, consent: bool) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::GrokPersonal(consent, tx))
            .await?;
        rx.await?
    }
    pub async fn connect_github_personal(&self, consent: bool) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::GithubPersonal(consent, tx))
            .await?;
        rx.await?
    }
    pub async fn connect_cursor_personal(&self, consent: bool) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::CursorPersonal(consent, tx))
            .await?;
        rx.await?
    }
    pub async fn connect_cli(&self, profile: crate::cli_profiles::Profile) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::CliConnect(profile, tx)).await?;
        rx.await?
    }
    pub async fn disconnect_cli(&self, id: String) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::CliDisconnect(id, tx)).await?;
        rx.await?
    }
    pub async fn disconnect(&self, id: String) -> Result<Settings> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Disconnect(id, tx)).await?;
        rx.await?
    }
    pub async fn shutdown(&self) {
        for token in self.existing_runs.lock().unwrap().values() {
            token.cancel();
        }
        self.tracking.shutdown().await;
        let (tx, rx) = oneshot::channel();
        let _ = self.commands.send(Command::Shutdown(tx)).await;
        let _ = tokio::time::timeout(Duration::from_secs(3), rx).await;
    }
}
fn expire_cli_profile_observations(state: &watch::Sender<Snapshot>) {
    if !state.borrow().accounts.iter().any(|a| {
        a.report
            .as_ref()
            .is_some_and(|r| r.source == ReportSource::AntigravityCliUsage)
    }) {
        return;
    }
    let now = Utc::now();
    let mut snapshot = state.borrow().clone();
    let mut changed = false;
    for a in &mut snapshot.accounts {
        if let Some(r) = a
            .report
            .as_mut()
            .filter(|r| r.source == ReportSource::AntigravityCliUsage)
        {
            let count = r.quotas.len();
            r.quotas.retain(|q| {
                now - r.observed_at <= chrono::Duration::hours(1)
                    && q.resets_at.is_none_or(|reset| reset > now)
            });
            changed |= r.quotas.len() != count;
            if r.quotas.is_empty() {
                a.report = None;
                a.status = Status::Faulted;
                a.fault=Some(Fault::new(FaultKind::Unreadable,"The CLI-profile quota observation expired. No account, full allowance or reset rollover was inferred."));
                changed = true;
            }
        }
    }
    if changed {
        snapshot.revision += 1;
        state.send_replace(snapshot);
    }
}
fn expire_muse_observations(state: &watch::Sender<Snapshot>) {
    if !state.borrow().accounts.iter().any(|a| {
        a.report
            .as_ref()
            .is_some_and(|r| r.source == ReportSource::MuseCli)
    }) {
        return;
    }
    let now = Utc::now();
    let mut snapshot = state.borrow().clone();
    let mut changed = false;
    for account in &mut snapshot.accounts {
        if let Some(report) = account
            .report
            .as_mut()
            .filter(|r| r.source == ReportSource::MuseCli)
        {
            let count = report.quotas.len();
            report.quotas.retain(|q| {
                now - report.observed_at <= chrono::Duration::hours(24)
                    && q.resets_at.is_some_and(|at| at > now)
            });
            if report.quotas.len() != count {
                changed = true;
            }
            if report.quotas.is_empty() {
                account.report = None;
                account.status = Status::Faulted;
                account.fault=Some(Fault::new(FaultKind::PermissionDenied,"Muse's observed subscription windows expired. No full allowance or new reset was inferred."));
                changed = true;
            }
        }
    }
    if changed {
        snapshot.revision += 1;
        state.send_replace(snapshot);
    }
}
pub fn retry_delay(minutes: u32, failures: u32, fault: Option<&Fault>, id: &str) -> Duration {
    if failures == 0 {
        return Duration::from_secs(minutes as u64 * 60);
    }
    if fault.is_some_and(|f| {
        matches!(
            f.kind,
            FaultKind::NotSignedIn | FaultKind::NeedsRelogin | FaultKind::ToolMissing
        )
    }) {
        return Duration::from_secs(300);
    }
    let server = fault.and_then(|f| f.retry_after_secs).unwrap_or(0);
    let seconds = (30u64.saturating_mul(2u64.pow(failures.min(6))))
        .min(1800)
        .max(server.min(32 * 86400));
    let jitter = id.bytes().map(u64::from).sum::<u64>() % 11;
    Duration::from_secs(seconds + jitter)
}
#[allow(clippy::too_many_arguments)]
async fn run(
    storage: Storage,
    mut settings: Settings,
    tx: mpsc::Sender<Command>,
    mut commands: mpsc::Receiver<Command>,
    state_tx: watch::Sender<Snapshot>,
    settings_tx: watch::Sender<Settings>,
    connections: crate::connections::Service,
    sources_tx: watch::Sender<crate::monitoring::Inventory>,
) {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("TLS client initialization");
    let mut codex = CodexAdapter::new(storage.user_home.clone(), http.clone());
    let claude = ClaudeAdapter {
        http,
        user_home: storage.user_home.clone(),
    };
    let mut cached: Snapshot = read_json(&storage.dir.join("state.json")).unwrap_or_default();
    let mut persisted = String::new();
    for s in &mut cached.accounts {
        if settings.connections.iter().any(|c| {
            c.id == s.account.id
                && c.mode == crate::connections::ConnectionMode::AntigravityCliUsage
        }) {
            s.report = None;
            s.status = Status::Pending;
        } else {
            s.status = Status::Stale;
        }
    }
    let callback_tx = tx.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            if !matches!(event.kind, notify::EventKind::Access(_))
                && event.paths.iter().any(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("state.vscdb"))
                })
            {
                let _ = callback_tx.try_send(Command::CursorChanged);
            }
            if !matches!(event.kind, notify::EventKind::Access(_))
                && event.paths.iter().any(|p| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy();
                    name == "auth.json"
                        || name == "credentials.toml"
                        || name == "config.toml"
                        || name == "settings.json"
                        || name == ".credentials.json"
                        || name.to_lowercase().contains("codex")
                })
            {
                let _ = callback_tx.try_send(Command::Changed);
            }
        }
    })
    .ok();
    let mut watched = HashSet::<PathBuf>::new();
    let mut due = HashMap::<String, crate::schedule::Schedule>::new();
    let started = Utc::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut next_scan = Instant::now();
    let mut force = true;
    let mut requested = true;
    let mut last_change = Instant::now() - Duration::from_secs(5);
    loop {
        expire_muse_observations(&state_tx);
        expire_cli_profile_observations(&state_tx);
        if requested || Instant::now() >= next_scan {
            if connections.cursor_auth_changed(&settings).await {
                cursor_changed(&settings, &state_tx, &mut due);
            }
            if let Some(w) = watcher.as_mut() {
                let mut paths = codex.candidate_homes(&settings);
                paths.extend([storage.user_home.clone(), storage.user_home.join(".config")]);
                paths.extend(
                    claude
                        .accounts(&settings)
                        .iter()
                        .flat_map(|a| a.sources.iter().map(|s| s.path.clone())),
                );
                paths.extend(
                    settings
                        .connections
                        .iter()
                        .filter(|c| {
                            matches!(
                                c.mode,
                                crate::connections::ConnectionMode::DevinCli
                                    | crate::connections::ConnectionMode::DevinApp
                                    | crate::connections::ConnectionMode::AntigravityStored
                                    | crate::connections::ConnectionMode::MuseCli
                                    | crate::connections::ConnectionMode::MuseDevice
                            )
                        })
                        .filter_map(|c| c.profile.as_deref().map(PathBuf::from))
                        .filter_map(|p| p.parent().map(|p| p.to_path_buf())),
                );
                paths.extend(
                    settings
                        .connections
                        .iter()
                        .filter(|c| c.mode == crate::connections::ConnectionMode::GrokCli)
                        .filter_map(|c| c.profile.as_deref().map(PathBuf::from)),
                );
                if let Some(path) = connections.cursor_watch_path(&settings) {
                    paths.push(path.parent().unwrap_or(&path).to_path_buf());
                }
                for path in paths {
                    for p in [path.clone(), path.parent().unwrap_or(&path).to_path_buf()] {
                        if p.is_dir() && watched.insert(p.clone()) {
                            let _ = w.watch(&p, RecursiveMode::NonRecursive);
                        }
                    }
                }
            }
            // Subscribe before reading: a credential write between the read and watch registration
            // must not leave an old quota visible until the minute fallback.
            let mut local_scope_changes = connections.local_scope_changes(&settings).await;
            local_scope_changes.extend(connections.grok_scope_changes(&settings).await);
            let mut snapshot = state_tx.borrow().clone();
            snapshot.refreshing = true;
            snapshot.revision += 1;
            state_tx.send_replace(snapshot.clone());
            let blocked: HashSet<String> = state_tx
                .borrow()
                .accounts
                .iter()
                .flat_map(|state| {
                    let allowed = due
                        .get(&state.account.id)
                        .is_none_or(|d| d.due(Instant::now(), force));
                    state
                        .account
                        .sources
                        .iter()
                        .enumerate()
                        .filter(move |(i, _)| !allowed || *i > 0)
                        .map(|(_, source)| crate::storage::path_key(&source.path))
                })
                .collect();
            let discovery = if settings.codex_enabled {
                codex.discover_gated(&settings, false, &blocked).await
            } else {
                crate::codex::Discovery {
                    accounts: vec![],
                    folders: vec![],
                    prefetched: HashMap::new(),
                }
            };
            let mut accounts = discovery.accounts;
            if settings.claude_enabled {
                accounts.extend(claude.monitored_accounts(&settings));
            }
            accounts.extend(settings.connections.iter().map(|c| c.account(&settings)));
            // Match cache by verified current identity, never by folder or a display alias.
            let mut prior: HashMap<_, _> = snapshot
                .accounts
                .into_iter()
                .chain(cached.accounts.drain(..))
                .map(|a| (a.account.id.clone(), a))
                .collect();
            for id in &local_scope_changes {
                if let Some(s) = prior.get_mut(id) {
                    *s = AccountState::apply(s.account.clone(), Err(Fault::changed()), None);
                }
            }
            let codex_reader = &codex;
            let claude_reader = &claude;
            let connection_reader = &connections;
            let config = &settings;
            let deadlines = &due;
            let prefetch = &discovery.prefetched;
            let previous_states = &prior;
            let jobs = accounts.into_iter().map(|mut account| async move {
                let id = account.id.clone();
                let previous = previous_states.get(&id);
                let is_due = previous.is_none()
                    || deadlines
                        .get(&id)
                        .is_none_or(|d| d.due(Instant::now(), force));
                let prefetched = prefetch.get(&id).cloned();
                if !is_due
                    && prefetched.is_none()
                    && previous.is_some_and(|p| !p.account.same_scope(&account))
                {
                    let fault = previous
                        .and_then(|p| p.fault.as_ref())
                        .filter(|f| f.kind == FaultKind::RateLimited)
                        .cloned()
                        .unwrap_or_else(Fault::changed);
                    return (AccountState::apply(account, Err(fault), previous), None);
                }
                if is_due || prefetched.is_some() {
                    let fetched = match prefetched {
                        Some(r) => Ok(r),
                        None => match account.provider {
                            Provider::Codex => codex_reader.fetch(&account).await,
                            Provider::Claude => match claude_reader
                                .fetch_scoped(&account, config.claude_token_refresh)
                                .await
                            {
                                Ok((report, marker)) => {
                                    ClaudeAdapter::bind_scope(&mut account, &marker);
                                    Ok(report)
                                }
                                Err(fault) => Err(fault),
                            },
                            _ => match config.connections.iter().find(|c| c.id == account.id) {
                                Some(c) => connection_reader.fetch(c).await,
                                None => Err(Fault::new(
                                    FaultKind::NotSignedIn,
                                    "This connection was removed.",
                                )),
                            },
                        },
                    };
                    let mut schedule = deadlines
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| crate::schedule::Schedule::new(Instant::now()));
                    schedule.record(
                        Instant::now(),
                        &fetched,
                        config.refresh_minutes,
                        config.live_tracking,
                        &id,
                    );
                    if let Err(f) = &fetched {
                        tracing::warn!(provider=?account.provider,kind=?f.kind,"Usage read failed");
                    }
                    (
                        AccountState::apply(account, fetched, previous),
                        Some(schedule),
                    )
                } else {
                    let mut p = previous.unwrap().clone();
                    p.account = account;
                    (p, None)
                }
            });
            // At most three simultaneous CLI processes / network reads, regardless of account count.
            let completed = futures_util::stream::iter(jobs)
                .buffer_unordered(3)
                .collect::<Vec<_>>()
                .await;
            let mut next = vec![];
            for (current, deadline) in completed {
                if let Some(deadline) = deadline {
                    due.insert(current.account.id.clone(), deadline);
                }
                next.push(current);
            }
            next.sort_by_key(|a| {
                (
                    if a.account.provider == Provider::Codex {
                        0
                    } else {
                        1
                    },
                    a.account.id.clone(),
                )
            });
            let active: HashSet<_> = next.iter().map(|a| a.account.id.clone()).collect();
            due.retain(|id, _| active.contains(id));
            snapshot = state_tx.borrow().clone();
            snapshot.accounts = next;
            snapshot.folders = discovery.folders;
            snapshot.scanned_at = Some(Utc::now());
            snapshot.refreshing = false;
            snapshot.revision += 1;
            // Tokens live only in provider-local values and cannot enter Snapshot serialization.
            let fingerprint = crate::model::hash(
                &serde_json::to_string(&(&snapshot.accounts, &snapshot.folders))
                    .unwrap_or_default(),
            );
            if fingerprint != persisted {
                if atomic_json(&storage.dir.join("state.json"), &snapshot).is_err() {
                    tracing::warn!("Usage cache could not be saved");
                } else {
                    persisted = fingerprint;
                }
            }
            state_tx.send_replace(snapshot.clone());
            let store = storage.clone();
            let cfg = settings.clone();
            let revision = sources_tx.borrow().revision + 1;
            if let Ok(inventory) = tokio::task::spawn_blocking(move || {
                crate::monitoring::inventory(&store, &cfg, &snapshot, revision)
            })
            .await
            {
                sources_tx.send_replace(inventory);
            }
            next_scan = Instant::now() + Duration::from_secs(60);
            requested = false;
            force = false;
        }
        let command = tokio::select! {c=commands.recv()=>c,_=interval.tick()=>{
            if due.values().any(|d|d.at<=Instant::now()) {requested=true;}
            None
        }};
        match command {
            Some(Command::History(provider, enabled, consent, reply)) => {
                let result =
                    history_settings(&storage, &settings, provider, enabled, consent).map(|next| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            None,
                        );
                        requested = true;
                        crate::monitoring::Outcome {
                            provider,
                            settings: settings.clone(),
                            connected: 0,
                            history_enabled: enabled,
                        }
                    });
                let _ = reply.send(result);
            }
            Some(Command::Existing(provider, consent, muse_key_endpoint, cancel, reply)) => {
                let result = tokio::select! { biased;
                    _ = cancel.cancelled() => Err(anyhow::anyhow!("Source checking cancelled. No new account was connected.")),
                    result = async {
                        if muse_key_endpoint {
                            match provider{Provider::Muse=>connections.connect_muse_subscription(&settings,consent).await.map(|(s,a)|(s,vec![a])),Provider::Antigravity=>connections.connect_antigravity_cli_usage(&settings,consent).await.map(|(s,a)|(s,vec![a])),_=>Err(anyhow::anyhow!("This operation requires an explicit supported source."))}
                        } else { connect_existing(&storage,&settings,&connections,&mut codex,&claude,provider,consent).await }
                    } => result,
                }
                .map(|(next, accounts)| {
                    let connected = accounts.len();
                    if accounts.is_empty() {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            None,
                        );
                    } else {
                        for account in accounts {
                            connection_updated(
                                &mut settings,
                                &settings_tx,
                                &state_tx,
                                &mut due,
                                next.clone(),
                                Some(account),
                            );
                        }
                    }
                    requested = true;
                    crate::monitoring::Outcome {
                        provider,
                        settings: settings.clone(),
                        connected,
                        history_enabled: crate::monitoring::history_enabled(&settings, provider),
                    }
                });
                let _ = reply.send(result);
            }
            Some(Command::CursorChanged) => {
                if connections.cursor_auth_changed(&settings).await {
                    cursor_changed(&settings, &state_tx, &mut due);
                    requested = true;
                }
                if connections.local_watch_due(&settings)
                    && !connections.local_scope_changes(&settings).await.is_empty()
                {
                    requested = true;
                }
            }
            Some(Command::CursorProbe(consent, reply)) => {
                if !reply.is_closed() {
                    let _ = reply.send(connections.cursor_sign_in_marker(consent).await);
                }
            }
            Some(Command::VerificationWait(provider, reply)) => {
                let _ = reply.send(connections.verification_wait_secs(provider));
            }
            Some(Command::GrokProbe(consent, reply)) => {
                if !reply.is_closed() {
                    let _ = reply.send(connections.grok_sign_in_marker(consent).await);
                }
            }
            Some(Command::GrokPersonal(consent, reply)) => {
                let result = connections
                    .connect_grok_personal(&settings, consent)
                    .await
                    .map(|(next, account)| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            Some(account),
                        );
                        requested = true;
                        settings.clone()
                    });
                let _ = reply.send(result);
            }
            Some(Command::AntigravityProbe(client, consent, reply)) => {
                if !reply.is_closed() {
                    let _ = reply.send(
                        connections
                            .antigravity_recovery_marker(client, consent)
                            .await,
                    );
                }
            }
            Some(Command::AntigravityPersonal(client, consent, reply)) => {
                let result = connections
                    .connect_antigravity_client(&settings, client, consent)
                    .await
                    .map(|(next, account)| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            Some(account),
                        );
                        requested = true;
                        settings.clone()
                    });
                let _ = reply.send(result);
            }
            Some(Command::GithubProbe(consent, reply)) => {
                if !reply.is_closed() {
                    let _ = reply.send(connections.github_sign_in_marker(consent).await);
                }
            }
            Some(Command::GithubPersonal(consent, reply)) => {
                let result = connections
                    .connect_github_personal(&settings, consent)
                    .await
                    .map(|(next, account)| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            Some(account),
                        );
                        requested = true;
                        settings.clone()
                    });
                let _ = reply.send(result);
            }
            Some(Command::CursorPersonal(consent, reply)) => {
                let result = connections
                    .connect_cursor_personal(&settings, consent)
                    .await
                    .map(|(next, account)| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            Some(account),
                        );
                        requested = true;
                        settings.clone()
                    });
                let _ = reply.send(result);
            }
            Some(Command::CliConnect(profile, reply)) => {
                let result = verify_cli(
                    &storage,
                    &settings,
                    &connections,
                    &mut codex,
                    &claude,
                    profile,
                )
                .await
                .map(|(next, account)| {
                    connection_updated(
                        &mut settings,
                        &settings_tx,
                        &state_tx,
                        &mut due,
                        next,
                        Some(account),
                    );
                    requested = true;
                    settings.clone()
                });
                let _ = reply.send(result);
            }
            Some(Command::CliDisconnect(id, reply)) => {
                let result = disconnect_cli(&storage, &settings, &state_tx, &id).map(|next| {
                    connection_updated(
                        &mut settings,
                        &settings_tx,
                        &state_tx,
                        &mut due,
                        next,
                        None,
                    );
                    requested = true;
                    settings.clone()
                });
                let _ = reply.send(result);
            }
            Some(Command::Activity(sample)) => {
                activity(&sample, &settings, &state_tx, &mut due, started);
            }
            Some(Command::ApiStatus(error)) => {
                let mut s = state_tx.borrow().clone();
                s.api_error = error;
                s.revision += 1;
                state_tx.send_replace(s);
            }
            Some(Command::Refresh) => {
                for schedule in due.values_mut() {
                    schedule.request_manual(Instant::now());
                }
                force = true;
                requested = true;
            }
            Some(Command::Changed) => {
                // Debounce the CLI's atomic replace / rename event burst.
                if last_change.elapsed() > Duration::from_millis(500) {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    // A new login can clear an authentication fault, but never a server throttle.
                    let auth_faults: HashSet<_> = state_tx
                        .borrow()
                        .accounts
                        .iter()
                        .filter(|s| {
                            matches!(s.account.provider, Provider::Codex | Provider::Claude)
                                && s.fault.as_ref().is_some_and(|f| {
                                    matches!(
                                        f.kind,
                                        FaultKind::NotSignedIn
                                            | FaultKind::NeedsRelogin
                                            | FaultKind::IdentityChanged
                                    )
                                })
                        })
                        .map(|s| s.account.id.clone())
                        .collect();
                    for id in auth_faults {
                        if let Some(schedule) = due.get_mut(&id) {
                            schedule.credentials_changed(Instant::now());
                        }
                    }
                    requested = true;
                    last_change = Instant::now();
                }
            }
            Some(Command::Settings(mut s, reply)) => {
                s.connections = settings.connections.clone(); // Only targeted, verified commands can change secure connections.
                s.history_sources = settings.history_sources.clone();
                crate::cli_profiles::preserve_managed(&storage, &settings, &mut s);
                let result = storage.save_settings(&s);
                if result.is_ok() {
                    if !s.live_tracking {
                        for schedule in due.values_mut() {
                            schedule.disable_live(Instant::now(), s.refresh_minutes);
                        }
                    }
                    settings = *s;
                    settings_tx.send_replace(settings.clone());
                    force = true;
                    requested = true;
                }
                let _ = reply.send(result);
            }
            Some(Command::AccountOrder(order, reply)) => {
                let mut next = settings.clone();
                next.account_order = order;
                let result = storage.save_settings(&next);
                if result.is_ok() {
                    settings = next;
                    settings_tx.send_replace(settings.clone());
                }
                let _ = reply.send(result);
            }
            Some(Command::DashboardPinned(pinned, reply)) => {
                let mut next = settings.clone();
                next.dashboard_pinned = pinned;
                let result = storage.save_settings(&next);
                if result.is_ok() {
                    settings = next;
                    settings_tx.send_replace(settings.clone());
                }
                let _ = reply.send(result);
            }
            Some(Command::Connect(input, reply)) => {
                let result = connections
                    .connect(&settings, input)
                    .await
                    .map(|(next, account)| {
                        connection_updated(
                            &mut settings,
                            &settings_tx,
                            &state_tx,
                            &mut due,
                            next,
                            Some(account),
                        );
                        requested = true;
                        settings.clone()
                    });
                let _ = reply.send(result);
            }
            Some(Command::Disconnect(id, reply)) => {
                let result = connections.disconnect(&settings, &id).map(|next| {
                    connection_updated(
                        &mut settings,
                        &settings_tx,
                        &state_tx,
                        &mut due,
                        next,
                        None,
                    );
                    requested = true;
                    settings.clone()
                });
                let _ = reply.send(result);
            }
            Some(Command::Shutdown(reply)) => {
                let _ = reply.send(());
                break;
            }
            None => {}
        }
        // Coalesce refresh bursts without dropping settings/shutdown commands.
        if requested {
            while let Ok(cmd) = commands.try_recv() {
                match cmd {
                    Command::History(provider, enabled, consent, reply) => {
                        let result = history_settings(
                            &storage, &settings, provider, enabled, consent,
                        )
                        .map(|next| {
                            connection_updated(
                                &mut settings,
                                &settings_tx,
                                &state_tx,
                                &mut due,
                                next,
                                None,
                            );
                            crate::monitoring::Outcome {
                                provider,
                                settings: settings.clone(),
                                connected: 0,
                                history_enabled: enabled,
                            }
                        });
                        let _ = reply.send(result);
                    }
                    Command::Existing(provider, consent, muse_key_endpoint, cancel, reply) => {
                        let result = tokio::select! { biased;
                            _ = cancel.cancelled() => Err(anyhow::anyhow!("Source checking cancelled. No new account was connected.")),
                            result = async {
                                if muse_key_endpoint {
                                    match provider{Provider::Muse=>connections.connect_muse_subscription(&settings,consent).await.map(|(s,a)|(s,vec![a])),Provider::Antigravity=>connections.connect_antigravity_cli_usage(&settings,consent).await.map(|(s,a)|(s,vec![a])),_=>Err(anyhow::anyhow!("This operation requires an explicit supported source."))}
                                } else { connect_existing(&storage,&settings,&connections,&mut codex,&claude,provider,consent).await }
                            } => result,
                        }
                        .map(|(next, accounts)| {
                            let connected = accounts.len();
                            if accounts.is_empty() {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    None,
                                );
                            } else {
                                for account in accounts {
                                    connection_updated(
                                        &mut settings,
                                        &settings_tx,
                                        &state_tx,
                                        &mut due,
                                        next.clone(),
                                        Some(account),
                                    );
                                }
                            }
                            requested = true;
                            crate::monitoring::Outcome {
                                provider,
                                settings: settings.clone(),
                                connected,
                                history_enabled: crate::monitoring::history_enabled(
                                    &settings, provider,
                                ),
                            }
                        });
                        let _ = reply.send(result);
                    }
                    Command::Refresh => {
                        for schedule in due.values_mut() {
                            schedule.request_manual(Instant::now());
                        }
                        force = true;
                    }
                    Command::Changed => {}
                    Command::CursorChanged => {
                        if connections.cursor_auth_changed(&settings).await {
                            cursor_changed(&settings, &state_tx, &mut due);
                        }
                        if connections.local_watch_due(&settings)
                            && !connections.local_scope_changes(&settings).await.is_empty()
                        {
                            requested = true;
                        }
                    }
                    Command::CursorProbe(consent, reply) => {
                        if !reply.is_closed() {
                            let _ = reply.send(connections.cursor_sign_in_marker(consent).await);
                        }
                    }
                    Command::VerificationWait(provider, reply) => {
                        let _ = reply.send(connections.verification_wait_secs(provider));
                    }
                    Command::GrokProbe(consent, reply) => {
                        if !reply.is_closed() {
                            let _ = reply.send(connections.grok_sign_in_marker(consent).await);
                        }
                    }
                    Command::GrokPersonal(consent, reply) => {
                        let result = connections
                            .connect_grok_personal(&settings, consent)
                            .await
                            .map(|(next, account)| {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    Some(account),
                                );
                                settings.clone()
                            });
                        let _ = reply.send(result);
                    }
                    Command::AntigravityProbe(client, consent, reply) => {
                        if !reply.is_closed() {
                            let _ = reply.send(
                                connections
                                    .antigravity_recovery_marker(client, consent)
                                    .await,
                            );
                        }
                    }
                    Command::AntigravityPersonal(client, consent, reply) => {
                        let result = connections
                            .connect_antigravity_client(&settings, client, consent)
                            .await
                            .map(|(next, account)| {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    Some(account),
                                );
                                requested = true;
                                settings.clone()
                            });
                        let _ = reply.send(result);
                    }
                    Command::GithubProbe(consent, reply) => {
                        if !reply.is_closed() {
                            let _ = reply.send(connections.github_sign_in_marker(consent).await);
                        }
                    }
                    Command::GithubPersonal(consent, reply) => {
                        let result = connections
                            .connect_github_personal(&settings, consent)
                            .await
                            .map(|(next, account)| {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    Some(account),
                                );
                                settings.clone()
                            });
                        let _ = reply.send(result);
                    }
                    Command::CursorPersonal(consent, reply) => {
                        let result = connections
                            .connect_cursor_personal(&settings, consent)
                            .await
                            .map(|(next, account)| {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    Some(account),
                                );
                                settings.clone()
                            });
                        let _ = reply.send(result);
                    }
                    Command::CliConnect(profile, reply) => {
                        let result = verify_cli(
                            &storage,
                            &settings,
                            &connections,
                            &mut codex,
                            &claude,
                            profile,
                        )
                        .await
                        .map(|(next, account)| {
                            connection_updated(
                                &mut settings,
                                &settings_tx,
                                &state_tx,
                                &mut due,
                                next,
                                Some(account),
                            );
                            settings.clone()
                        });
                        let _ = reply.send(result);
                    }
                    Command::CliDisconnect(id, reply) => {
                        let result =
                            disconnect_cli(&storage, &settings, &state_tx, &id).map(|next| {
                                connection_updated(
                                    &mut settings,
                                    &settings_tx,
                                    &state_tx,
                                    &mut due,
                                    next,
                                    None,
                                );
                                settings.clone()
                            });
                        let _ = reply.send(result);
                    }
                    Command::Activity(sample) => {
                        activity(&sample, &settings, &state_tx, &mut due, started);
                    }
                    Command::ApiStatus(error) => {
                        let mut s = state_tx.borrow().clone();
                        s.api_error = error;
                        s.revision += 1;
                        state_tx.send_replace(s);
                    }
                    Command::Settings(mut s, reply) => {
                        s.connections = settings.connections.clone();
                        s.history_sources = settings.history_sources.clone();
                        crate::cli_profiles::preserve_managed(&storage, &settings, &mut s);
                        let result = storage.save_settings(&s);
                        if result.is_ok() {
                            if !s.live_tracking {
                                for schedule in due.values_mut() {
                                    schedule.disable_live(Instant::now(), s.refresh_minutes);
                                }
                            }
                            settings = *s;
                            settings_tx.send_replace(settings.clone());
                            force = true;
                        }
                        let _ = reply.send(result);
                    }
                    Command::AccountOrder(order, reply) => {
                        let mut next = settings.clone();
                        next.account_order = order;
                        let result = storage.save_settings(&next);
                        if result.is_ok() {
                            settings = next;
                            settings_tx.send_replace(settings.clone());
                        }
                        let _ = reply.send(result);
                    }
                    Command::DashboardPinned(pinned, reply) => {
                        let mut next = settings.clone();
                        next.dashboard_pinned = pinned;
                        let result = storage.save_settings(&next);
                        if result.is_ok() {
                            settings = next;
                            settings_tx.send_replace(settings.clone());
                        }
                        let _ = reply.send(result);
                    }
                    Command::Connect(input, reply) => {
                        let result =
                            connections
                                .connect(&settings, input)
                                .await
                                .map(|(next, account)| {
                                    connection_updated(
                                        &mut settings,
                                        &settings_tx,
                                        &state_tx,
                                        &mut due,
                                        next,
                                        Some(account),
                                    );
                                    settings.clone()
                                });
                        let _ = reply.send(result);
                    }
                    Command::Disconnect(id, reply) => {
                        let result = connections.disconnect(&settings, &id).map(|next| {
                            connection_updated(
                                &mut settings,
                                &settings_tx,
                                &state_tx,
                                &mut due,
                                next,
                                None,
                            );
                            settings.clone()
                        });
                        let _ = reply.send(result);
                    }
                    Command::Shutdown(reply) => {
                        let _ = reply.send(());
                        return;
                    }
                }
            }
        }
    }
}
fn history_settings(
    storage: &Storage,
    current: &Settings,
    provider: Provider,
    enabled: bool,
    consent: bool,
) -> Result<Settings> {
    if !consent {
        anyhow::bail!("Change local-history monitoring explicitly.");
    }
    let mut next = crate::monitoring::enable_history(current, provider)?;
    next.history_sources.insert(provider, enabled);
    storage.save_settings(&next)?;
    Ok(next)
}
fn cursor_changed(
    settings: &Settings,
    state_tx: &watch::Sender<Snapshot>,
    due: &mut HashMap<String, crate::schedule::Schedule>,
) {
    let mut snapshot = state_tx.borrow().clone();
    let mut changed = false;
    for state in snapshot.accounts.iter_mut().filter(|s| {
        settings.connections.iter().any(|c| {
            c.id == s.account.id && c.mode == crate::connections::ConnectionMode::CursorApp
        })
    }) {
        let throttled = state
            .fault
            .as_ref()
            .is_some_and(|f| f.kind == FaultKind::RateLimited);
        if !throttled {
            if let Some(schedule) = due.get_mut(&state.account.id) {
                schedule.credentials_changed(Instant::now());
            }
        }
        state.report = None;
        state.status = Status::Faulted;
        state.fault = Some(Fault::changed());
        changed = true;
    }
    if changed {
        snapshot.revision += 1;
        state_tx.send_replace(snapshot);
    }
}
async fn verify_cli(
    storage: &Storage,
    settings: &Settings,
    connections: &crate::connections::Service,
    codex: &mut CodexAdapter,
    claude: &ClaudeAdapter,
    profile: crate::cli_profiles::Profile,
) -> Result<(Settings, AccountState)> {
    use crate::cli_profiles::CliProvider;
    profile.validate(storage)?;
    connections.begin_verification(profile.provider.provider())?;
    let verified = tokio::time::timeout(Duration::from_secs(55), async {
        let mut isolated = settings.clone();
        isolated.auto_discover = false;
        isolated.ignored_homes.clear();
        let home = profile.home.to_string_lossy().into_owned();
        let (account, report) = match profile.provider {
            CliProvider::Codex => {
                isolated.codex_homes = vec![home];
                let mut found = codex.discover(&isolated, true).await;
                if found.accounts.len() != 1 { return Err(anyhow::anyhow!(Fault::new(FaultKind::NotSignedIn, "Codex has not completed a subscription sign-in. Finish in its sign-in window."))); }
                let account = found.accounts.remove(0);
                let report = match found.prefetched.remove(&account.id) { Some(report) => report, None => codex.fetch(&account).await? };
                (account, report)
            }
            CliProvider::Claude => {
                isolated.claude_homes = vec![home];
                let mut account = claude.monitored_accounts(&isolated).into_iter().next().ok_or_else(||anyhow::anyhow!("Claude sign-in is unavailable."))?;
                let (report,marker) = claude.fetch_scoped(&account, false).await?;
                ClaudeAdapter::bind_scope(&mut account,&marker);
                (account, report)
            }
        };
        let state = AccountState { account, status: Status::Ready, report: Some(report), fault: None };
        let next = profile.commit(storage, settings, &state)?;
        Ok((next, state))
    }).await.unwrap_or_else(|_|Err(anyhow::anyhow!("Sign-in verification timed out. Credentials were left with the provider; try again.")));
    if let Err(error) = &verified {
        if let Some(fault) = error.downcast_ref::<Fault>() {
            connections.pause(profile.provider.provider(), fault);
        }
    }
    verified
}
fn disconnect_cli(
    storage: &Storage,
    settings: &Settings,
    state_tx: &watch::Sender<Snapshot>,
    id: &str,
) -> Result<Settings> {
    let mut snapshot = state_tx.borrow().clone();
    let account = snapshot
        .accounts
        .iter()
        .find(|a| a.account.id == id)
        .ok_or_else(|| anyhow::anyhow!("That CLI account is no longer monitored."))?;
    let next = crate::cli_profiles::disconnect(storage, settings, &account.account)?;
    snapshot.accounts.retain(|a| a.account.id != id);
    snapshot.revision += 1;
    state_tx.send_replace(snapshot);
    Ok(next)
}
fn connection_updated(
    settings: &mut Settings,
    settings_tx: &watch::Sender<Settings>,
    state_tx: &watch::Sender<Snapshot>,
    due: &mut HashMap<String, crate::schedule::Schedule>,
    next: Settings,
    account: Option<AccountState>,
) {
    let mut snapshot = state_tx.borrow().clone();
    snapshot.accounts.retain(|a| {
        matches!(a.account.provider, Provider::Codex | Provider::Claude)
            || next.connections.iter().any(|c| c.id == a.account.id)
    });
    if let Some(mut account) = account {
        if matches!(account.account.provider, Provider::Codex | Provider::Claude) {
            if let Some(previous) = snapshot
                .accounts
                .iter()
                .find(|a| a.account.id == account.account.id)
            {
                for source in &previous.account.sources {
                    if !account.account.sources.iter().any(|s| {
                        crate::storage::path_key(&s.path) == crate::storage::path_key(&source.path)
                    }) {
                        account.account.sources.push(source.clone());
                    }
                }
            }
        }
        snapshot
            .accounts
            .retain(|a| a.account.id != account.account.id);
        let mut schedule = due
            .get(&account.account.id)
            .cloned()
            .unwrap_or_else(|| crate::schedule::Schedule::new(Instant::now()));
        schedule.record(
            Instant::now(),
            &Ok(account.report.clone().expect("verified connection")),
            next.refresh_minutes,
            next.live_tracking,
            &account.account.id,
        );
        due.insert(account.account.id.clone(), schedule);
        snapshot.accounts.push(account);
    }
    due.retain(|id, _| snapshot.accounts.iter().any(|a| &a.account.id == id));
    *settings = next;
    settings_tx.send_replace(settings.clone());
    snapshot.revision += 1;
    state_tx.send_replace(snapshot);
}
fn activity(
    sample: &crate::activity::Activity,
    settings: &Settings,
    state_tx: &watch::Sender<Snapshot>,
    due: &mut HashMap<String, crate::schedule::Schedule>,
    started: chrono::DateTime<Utc>,
) {
    if !settings.live_tracking || !sample.fresh(started, Utc::now()) {
        return;
    }
    let mut snapshot = state_tx.borrow().clone();
    let mut changed = false;
    for state in snapshot
        .accounts
        .iter_mut()
        .filter(|state| sample.matches_home(state))
    {
        let applied = sample.apply(state, Utc::now());
        changed |= applied;
        due.entry(state.account.id.clone())
            .or_insert_with(|| crate::schedule::Schedule::new(Instant::now()))
            .activity(Instant::now(), applied);
    }
    if changed {
        snapshot.revision += 1;
        state_tx.send_replace(snapshot);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn account(id: &str) -> Account {
        Account {
            id: id.into(),
            provider: Provider::Codex,
            label: "test".into(),
            email: None,
            identity: None,
            sources: vec![],
        }
    }
    #[test]
    fn muse_observations_expire_without_a_new_quota_read_or_reset_inference() {
        let now = Utc::now();
        let mut a = account("muse-fixture");
        a.provider = Provider::Muse;
        let s = AccountState::apply(
            a,
            Ok(Report {
                plan: None,
                quotas: vec![
                    Quota::window(
                        "short",
                        "Observed window",
                        40.,
                        Some(now - chrono::Duration::seconds(1)),
                    ),
                    Quota::window(
                        "week",
                        "Observed week",
                        70.,
                        Some(now + chrono::Duration::hours(1)),
                    ),
                ],
                observed_at: now,
                source: ReportSource::MuseCli,
                verified_at: Some(now),
            }),
            None,
        );
        let (tx, _rx) = watch::channel(Snapshot {
            accounts: vec![s],
            ..Default::default()
        });
        expire_muse_observations(&tx);
        assert_eq!(
            tx.borrow().accounts[0]
                .report
                .as_ref()
                .unwrap()
                .quotas
                .len(),
            1
        );
        assert_eq!(
            tx.borrow().accounts[0].report.as_ref().unwrap().quotas[0].percent,
            Some(70.)
        );
        let mut snapshot = tx.borrow().clone();
        snapshot.accounts[0].report.as_mut().unwrap().observed_at =
            now - chrono::Duration::hours(25);
        tx.send_replace(snapshot);
        expire_muse_observations(&tx);
        assert!(tx.borrow().accounts[0].report.is_none());
        assert_eq!(tx.borrow().accounts[0].status, Status::Faulted);
    }
    #[test]
    fn credential_scope_changes_under_the_same_profile_id_never_inherit_cached_quota() {
        let mut first = account("claude-source");
        first.provider = Provider::Claude;
        first.identity = Some(Identity {
            workspace: "claude-profile".into(),
            email: None,
            user: Some("credential:first".into()),
        });
        let ready = AccountState::apply(
            first.clone(),
            Ok(Report {
                plan: None,
                quotas: vec![],
                observed_at: Utc::now(),
                source: ReportSource::Provider,
                verified_at: Some(Utc::now()),
            }),
            None,
        );
        assert_eq!(
            AccountState::apply(first.clone(), Err(Fault::offline()), Some(&ready)).status,
            Status::Stale
        );
        first.identity.as_mut().unwrap().user = Some("credential:second".into());
        assert!(
            AccountState::apply(first, Err(Fault::offline()), Some(&ready))
                .report
                .is_none()
        );
    }
    #[test]
    fn identity_change_clears_cache() {
        let prev = AccountState::apply(
            account("a"),
            Ok(Report {
                plan: None,
                quotas: vec![],
                observed_at: Utc::now(),
                source: ReportSource::Provider,
                verified_at: Some(Utc::now()),
            }),
            None,
        );
        let next = AccountState::apply(account("a"), Err(Fault::changed()), Some(&prev));
        assert!(next.report.is_none());
        assert_eq!(next.status, Status::Faulted);
    }
    #[test]
    fn stale_only_for_matching_identity() {
        let prev = AccountState::apply(
            account("a"),
            Ok(Report {
                plan: None,
                quotas: vec![],
                observed_at: Utc::now(),
                source: ReportSource::Provider,
                verified_at: Some(Utc::now()),
            }),
            None,
        );
        assert_eq!(
            AccountState::apply(account("a"), Err(Fault::offline()), Some(&prev)).status,
            Status::Stale
        );
        assert!(
            AccountState::apply(account("b"), Err(Fault::offline()), Some(&prev))
                .report
                .is_none()
        );
    }
    struct SignInCli;
    #[async_trait::async_trait]
    impl crate::codex::CodexCli for SignInCli {
        fn available(&self) -> bool {
            true
        }
        async fn read(
            &self,
            _home: &std::path::Path,
        ) -> std::result::Result<crate::codex::Reading, Fault> {
            Ok(crate::codex::Reading {
                identity: Identity {
                    workspace: "managed-fixture".into(),
                    email: Some("fixture@example.test".into()),
                    user: None,
                },
                plan: Some("plus".into()),
                usage: serde_json::json!({"rateLimits":{"primary":{"usedPercent":22,"windowDurationMins":300}}}),
            })
        }
    }
    #[tokio::test]
    async fn cli_verification_commits_only_verified_managed_profiles_and_keeps_settings() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage {
            dir: temp.path().join("app"),
            user_home: temp.path().join("user"),
        };
        let settings = Settings {
            launch_at_startup: true,
            theme: "dark".into(),
            ..Default::default()
        };
        let profile = crate::cli_profiles::Profile::prepare(
            &storage,
            crate::cli_profiles::CliProvider::Codex,
            &settings,
        )
        .unwrap();
        let service = crate::connections::Service::new(
            storage.clone(),
            std::sync::Arc::new(crate::connections::UnavailableStore),
        );
        let http = reqwest::Client::new();
        let mut codex = CodexAdapter::new(storage.user_home.clone(), http.clone());
        codex.cli = Box::new(SignInCli);
        let claude = ClaudeAdapter {
            http,
            user_home: storage.user_home.clone(),
        };
        let (next, account) = verify_cli(
            &storage,
            &settings,
            &service,
            &mut codex,
            &claude,
            profile.clone(),
        )
        .await
        .unwrap();
        assert_eq!(account.status, Status::Ready);
        assert_eq!(account.report.unwrap().quotas[0].percent, Some(22.));
        assert_eq!(
            account.account.email.as_deref(),
            Some("fixture@example.test")
        );
        assert_eq!(next.theme, "dark");
        assert!(next.launch_at_startup);
        assert!(!next.dashboard_pinned);
        assert_eq!(
            next.codex_homes,
            vec![profile.home.to_string_lossy().into_owned()]
        );
        assert_eq!(
            storage.load_settings().unwrap().codex_homes,
            next.codex_homes
        );
        assert!(
            verify_cli(&storage, &next, &service, &mut codex, &claude, profile)
                .await
                .is_err(),
            "connection verification gate must still apply"
        );
    }
    #[tokio::test]
    async fn verified_cli_account_is_not_published_when_settings_persistence_fails() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage {
            dir: temp.path().join("app"),
            user_home: temp.path().join("user"),
        };
        let profile = crate::cli_profiles::Profile::prepare(
            &storage,
            crate::cli_profiles::CliProvider::Codex,
            &Settings::default(),
        )
        .unwrap();
        std::fs::create_dir(storage.dir.join("settings.json")).unwrap();
        let service = crate::connections::Service::new(
            storage.clone(),
            std::sync::Arc::new(crate::connections::UnavailableStore),
        );
        let http = reqwest::Client::new();
        let mut codex = CodexAdapter::new(storage.user_home.clone(), http.clone());
        codex.cli = Box::new(SignInCli);
        let claude = ClaudeAdapter {
            http,
            user_home: storage.user_home.clone(),
        };
        assert!(verify_cli(
            &storage,
            &Settings::default(),
            &service,
            &mut codex,
            &claude,
            profile.clone()
        )
        .await
        .is_err());
        assert!(profile.home.exists());
        assert!(profile
            .home
            .parent()
            .unwrap()
            .join("pending.json")
            .is_file());
    }
    #[test]
    fn backoff_is_bounded_and_honors_server() {
        let f = Fault {
            kind: FaultKind::RateLimited,
            message: "wait".into(),
            retry_after_secs: Some(600),
        };
        assert!(retry_delay(5, 1, Some(&f), "a") >= Duration::from_secs(600));
        assert!(retry_delay(5, 100, None, "a") <= Duration::from_secs(1810));
    }
}
