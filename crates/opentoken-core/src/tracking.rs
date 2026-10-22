//! One background owner for incremental usage indexing, independent of the dashboard lifetime.
use crate::{
    engine::Command,
    model::{Provider, Snapshot},
    spending::{self, Spending, UsageIndex},
    storage::{path_key, Settings, Storage},
};
use chrono::{DateTime, Utc};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, watch, Notify},
    time::Instant,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingState {
    pub revision: u64,
    pub mode: String,
    pub data: Option<Spending>,
    pub error: Option<String>,
    pub last_activity_at: BTreeMap<String, DateTime<Utc>>,
}
impl Default for TrackingState {
    fn default() -> Self {
        Self {
            revision: 0,
            mode: "starting".into(),
            data: None,
            error: None,
            last_activity_at: BTreeMap::new(),
        }
    }
}
#[derive(Clone)]
pub struct Tracker {
    pub state: watch::Receiver<TrackingState>,
    stop: watch::Sender<bool>,
    done: watch::Receiver<bool>,
    wake: Arc<Notify>,
    visible: Arc<AtomicBool>,
}
impl Tracker {
    pub fn disabled() -> Self {
        let (_, state) = watch::channel(TrackingState {
            mode: "off".into(),
            ..Default::default()
        });
        let (stop, _) = watch::channel(true);
        let (_, done) = watch::channel(true);
        Self {
            state,
            stop,
            done,
            wake: Arc::new(Notify::new()),
            visible: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn start(
        storage: Storage,
        settings: watch::Receiver<Settings>,
        accounts: watch::Receiver<Snapshot>,
        commands: mpsc::Sender<Command>,
    ) -> Self {
        let (output, state) = watch::channel(TrackingState::default());
        let (stop, stopping) = watch::channel(false);
        let (done_tx, done) = watch::channel(false);
        let wake = Arc::new(Notify::new());
        let visible = Arc::new(AtomicBool::new(false));
        let task_wake = wake.clone();
        let task_visible = visible.clone();
        tokio::spawn(async move {
            run(
                storage,
                settings,
                accounts,
                commands,
                output,
                stopping,
                task_wake,
                task_visible,
            )
            .await;
            done_tx.send_replace(true);
        });
        Self {
            state,
            stop,
            done,
            wake,
            visible,
        }
    }
    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Relaxed);
        if visible {
            self.wake.notify_one();
        }
    }
    pub async fn shutdown(&self) {
        self.stop.send_replace(true);
        let mut done = self.done.clone();
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while !*done.borrow_and_update() {
                if done.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
    }
}
fn scan_key(settings: &Settings, roots: &[(String, PathBuf)]) -> String {
    serde_json::to_string(&(settings.live_tracking, roots, &settings.model_prices))
        .unwrap_or_default()
}
fn account_key(accounts: &Snapshot) -> String {
    // Theme, card order, API health and cosmetic scan revisions don't cause extra history scans.
    serde_json::to_string(&accounts.accounts).unwrap_or_default()
}
fn inside(path: &Path, root: &Path) -> bool {
    let p = path_key(path);
    let r = path_key(root);
    p == r
        || p.strip_prefix(&r)
            .is_some_and(|s| s.starts_with(['/', '\\']))
}
fn histories(roots: &[(String, PathBuf)]) -> Vec<PathBuf> {
    roots
        .iter()
        .flat_map(|(p, home)| {
            if p == "codex" {
                vec![home.join("sessions"), home.join("archived_sessions")]
            } else if matches!(p.as_str(), "grok" | "kimi") {
                vec![home.join("sessions")]
            } else if p == "muse" {
                vec![home.clone()]
            } else {
                vec![home.join("projects")]
            }
        })
        .collect()
}
fn safe_dir(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
fn desired_watches(roots: &[(String, PathBuf)]) -> HashSet<(PathBuf, bool)> {
    let mut result = HashSet::new();
    for (provider, home) in roots {
        let mut parent = home.as_path();
        while !safe_dir(parent) {
            let Some(next) = parent.parent() else {
                break;
            };
            parent = next;
        }
        if safe_dir(parent) {
            result.insert((parent.to_path_buf(), false));
        }
        if safe_dir(home) {
            let names: &[&str] = if provider == "codex" {
                &["sessions", "archived_sessions"]
            } else if matches!(provider.as_str(), "grok" | "kimi") {
                &["sessions"]
            } else if provider == "muse" {
                &[""]
            } else {
                &["projects"]
            };
            for name in names {
                let history = home.join(name);
                if safe_dir(&history) {
                    result.insert((history, true));
                }
            }
        }
    }
    result
}
fn new_watcher(
    notice_tx: mpsc::Sender<notify::Result<Event>>,
    missed: Arc<AtomicBool>,
) -> Option<RecommendedWatcher> {
    RecommendedWatcher::new(
        move |event: notify::Result<Event>| {
            if event
                .as_ref()
                .is_ok_and(|e| matches!(e.kind, EventKind::Access(_)))
            {
                return;
            }
            if notice_tx.try_send(event).is_err() {
                missed.store(true, Ordering::Relaxed);
            }
        },
        Config::default().with_follow_symlinks(false),
    )
    .ok()
}
#[allow(clippy::too_many_arguments)]
async fn run(
    storage: Storage,
    mut settings_rx: watch::Receiver<Settings>,
    mut account_rx: watch::Receiver<Snapshot>,
    commands: mpsc::Sender<Command>,
    output: watch::Sender<TrackingState>,
    mut stop: watch::Receiver<bool>,
    wake: Arc<Notify>,
    visible: Arc<AtomicBool>,
) {
    let (notice_tx, mut notices) = mpsc::channel::<notify::Result<Event>>(128);
    let overflow = Arc::new(AtomicBool::new(false));
    let mut watcher = new_watcher(notice_tx.clone(), overflow.clone());
    let mut retry_watch_at = Instant::now();
    let mut watched = HashSet::new();
    let mut healthy = watcher.is_some();
    let mut state = TrackingState::default();
    let mut index = None;
    let mut settings = settings_rx.borrow_and_update().clone();
    let mut accounts = account_rx.borrow_and_update().clone();
    let mut roots = spending::sources(&settings, &accounts.accounts, &storage.user_home);
    let mut config_key = scan_key(&settings, &roots);
    let mut prior_accounts = account_key(&accounts);
    let started = Utc::now();
    let mut full = true;
    let mut dirty = HashSet::new();
    let mut first_dirty = None;
    let mut deadline = Instant::now();
    let mut next_full = Instant::now();
    loop {
        if *stop.borrow() {
            break;
        }
        if Instant::now() >= next_full {
            full = true;
            deadline = Instant::now();
        }
        if overflow.swap(false, Ordering::Relaxed) {
            full = true;
            healthy = false;
            deadline = Instant::now();
        }
        if (full || !dirty.is_empty()) && Instant::now() >= deadline {
            if full {
                if !settings.live_tracking {
                    watcher = None;
                    watched.clear();
                    healthy = false;
                }
                if settings.live_tracking && watcher.is_none() && Instant::now() >= retry_watch_at {
                    watcher = new_watcher(notice_tx.clone(), overflow.clone());
                    retry_watch_at = Instant::now() + Duration::from_secs(15);
                }
                let desired = if settings.live_tracking {
                    desired_watches(&roots)
                } else {
                    HashSet::new()
                };
                if desired != watched || !healthy {
                    healthy = watcher.is_some();
                    if let Some(w) = watcher.as_mut() {
                        for (path, _) in watched.difference(&desired) {
                            let _ = w.unwatch(path);
                        }
                        let mut registered = watched
                            .intersection(&desired)
                            .cloned()
                            .collect::<HashSet<_>>();
                        for (path, recursive) in desired.difference(&registered.clone()) {
                            if w.watch(
                                path,
                                if *recursive {
                                    RecursiveMode::Recursive
                                } else {
                                    RecursiveMode::NonRecursive
                                },
                            )
                            .is_ok()
                            {
                                registered.insert((path.clone(), *recursive));
                            } else {
                                healthy = false;
                            }
                        }
                        watched = registered;
                    }
                }
            }
            let was_full = full;
            let paths: Option<Vec<PathBuf>> = if full {
                None
            } else {
                Some(dirty.drain().collect())
            };
            dirty.clear();
            full = false;
            first_dirty = None;
            let dir = storage.dir.clone();
            let job_roots = roots.clone();
            let cfg = settings.clone();
            let states = accounts.accounts.clone();
            let old_index = index.take();
            let job = tokio::task::spawn_blocking(move || {
                let mut owned = match old_index {
                    Some(i) => i,
                    None => match UsageIndex::open(&dir) {
                        Ok(i) => i,
                        Err(_) => return (None, Err(()), vec![]),
                    },
                };
                let result = owned
                    .scan_paths(&job_roots, &cfg, &states, Utc::now(), paths.as_deref())
                    .map_err(|_| ());
                let activity = owned.take_activity();
                (Some(owned), result, activity)
            })
            .await;
            match job {
                Ok((owned, result, activity)) => {
                    index = owned;
                    match result {
                        Ok(mut data) => {
                            if !was_full {
                                if let Some(previous) = &state.data {
                                    data.files_indexed = previous.files_indexed;
                                }
                            }
                            if data.indexing {
                                full = true;
                                deadline = Instant::now() + Duration::from_millis(600);
                            }
                            state.data = Some(data);
                            state.error = None;
                            if settings.live_tracking {
                                for sample in activity
                                    .into_iter()
                                    .filter(|a| a.fresh(started, Utc::now()))
                                {
                                    let provider = if sample.provider == Provider::Codex {
                                        "codex"
                                    } else {
                                        "claude"
                                    };
                                    let last = state
                                        .last_activity_at
                                        .entry(provider.into())
                                        .or_insert(sample.observed_at);
                                    *last = (*last).max(sample.observed_at);
                                    let _ = commands.try_send(Command::Activity(Box::new(sample)));
                                }
                            }
                        }
                        Err(()) => {
                            state.error = Some(
                                "Local history could not be indexed. CLI files were not changed."
                                    .into(),
                            );
                        }
                    }
                }
                Err(_) => {
                    state.error =
                        Some("Local history worker stopped. Retrying automatically.".into());
                }
            }
            state.mode = if !settings.live_tracking {
                "off"
            } else if healthy {
                "watching"
            } else {
                "fallback"
            }
            .into();
            state.revision += 1;
            output.send_replace(state.clone());
            if was_full {
                next_full = Instant::now()
                    + Duration::from_secs(if settings.live_tracking && !healthy {
                        15
                    } else {
                        60
                    });
            }
        }
        let wake_at = if full || !dirty.is_empty() {
            deadline.min(next_full)
        } else {
            next_full
        };
        tokio::select! {
            result = stop.changed() => { if result.is_err() || *stop.borrow() { break; } }
            result = settings_rx.changed() => {
                if result.is_err() { break; }
                settings = settings_rx.borrow_and_update().clone();
                roots = spending::sources(&settings, &accounts.accounts, &storage.user_home);
                let key = scan_key(&settings, &roots);
                if key != config_key { config_key = key; full = true; deadline = Instant::now(); if !settings.live_tracking { state.last_activity_at.clear(); } }
            }
            result = account_rx.changed() => {
                if result.is_err() { break; }
                accounts = account_rx.borrow_and_update().clone();
                let key = account_key(&accounts);
                if key != prior_accounts { prior_accounts = key; roots = spending::sources(&settings, &accounts.accounts, &storage.user_home); config_key = scan_key(&settings, &roots); full = true; deadline = Instant::now(); }
            }
            Some(notice) = notices.recv() => {
                if !settings.live_tracking { continue; }
                match notice {
                    Ok(event) => {
                        let history = histories(&roots); let now = Instant::now(); let mut relevant = false;
                        for path in event.paths {
                            if history.iter().any(|h| inside(&path, h)) {
                                relevant = true;
                                if path.extension().is_some_and(|e| e == "jsonl") && dirty.len() < 256 { dirty.insert(path); } else { full = true; }
                            } else if roots.iter().any(|(_, home)| inside(home, &path)) { relevant = true; full = true; }
                        }
                        if relevant {
                            let first = *first_dirty.get_or_insert(now);
                            let shown = visible.load(Ordering::Relaxed);
                            deadline = (now + Duration::from_millis(if shown { 250 } else { 800 })).min(first + Duration::from_millis(if shown { 1000 } else { 2000 }));
                        }
                    }
                    Err(_) => { watcher = None; watched.clear(); healthy = false; retry_watch_at = Instant::now() + Duration::from_secs(15); full = true; deadline = Instant::now(); }
                }
            }
            _ = wake.notified() => { full = true; deadline = Instant::now(); }
            _ = tokio::time::sleep_until(wake_at) => {}
        }
    }
}
