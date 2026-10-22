//! Bounded native sign-in coordination. Waiting is off the WebView/UI thread.
//! Only an explicit action starts a run; probing is local/read-only, not repeated usage HTTP.
use crate::model::{Fault, FaultKind};
use async_trait::async_trait;
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time::Instant};

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Waiting,
    Connecting,
    Connected,
    Cancelled,
    Error,
}
impl Phase {
    pub fn active(self) -> bool {
        matches!(self, Self::Checking | Self::Waiting | Self::Connecting)
    }
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub revision: u64,
    pub phase: Phase,
    pub message: String,
    pub failure: Option<FaultKind>,
}
#[derive(Default)]
struct Inner {
    run: u64,
    status: Status,
}
#[derive(Default)]
pub struct Flow {
    inner: Mutex<Inner>,
    wake: Notify,
}
impl Flow {
    pub fn status(&self) -> Status {
        self.inner.lock().unwrap().status.clone()
    }
    pub fn begin(&self) -> Option<u64> {
        self.begin_named("Cursor")
    }
    pub fn begin_named(&self, name: &str) -> Option<u64> {
        let mut inner = self.inner.lock().unwrap();
        if inner.status.phase.active() {
            return None;
        }
        inner.run = inner.run.wrapping_add(1);
        inner.status.revision += 1;
        inner.status.phase = Phase::Checking;
        inner.status.message = format!("Checking {name} sign-in…");
        inner.status.failure = None;
        Some(inner.run)
    }
    pub fn current(&self, run: u64) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.run == run && inner.status.phase.active()
    }
    fn change(
        &self,
        run: u64,
        phase: Phase,
        message: impl Into<String>,
        failure: Option<FaultKind>,
    ) -> Option<Status> {
        let mut inner = self.inner.lock().unwrap();
        if inner.run != run || !inner.status.phase.active() {
            return None;
        }
        let message = message.into();
        if inner.status.phase == phase
            && inner.status.message == message
            && inner.status.failure == failure
        {
            return None;
        }
        inner.status = Status {
            revision: inner.status.revision + 1,
            phase,
            message,
            failure,
        };
        Some(inner.status.clone())
    }
    // Once verification begins, the targeted engine transaction is allowed to finish.
    // The UI offers Cancel only during read-only checking/waiting, never a false rollback promise.
    pub fn reset(&self) -> Status {
        let mut inner = self.inner.lock().unwrap();
        inner.run = inner.run.wrapping_add(1);
        inner.status = Status {
            revision: inner.status.revision + 1,
            ..Default::default()
        };
        self.wake.notify_waiters();
        inner.status.clone()
    }
    pub fn cancel(&self) -> Status {
        let mut inner = self.inner.lock().unwrap();
        if matches!(inner.status.phase, Phase::Checking | Phase::Waiting) {
            inner.run = inner.run.wrapping_add(1);
            inner.status.revision += 1;
            inner.status.phase = Phase::Cancelled;
            inner.status.message = "Sign-in cancelled.".into();
            inner.status.failure = None;
            self.wake.notify_waiters();
        }
        inner.status.clone()
    }
}
#[async_trait]
pub trait Host: Send + Sync {
    /// A private, in-memory marker; never include it in Status, events or logs.
    async fn probe(&self) -> Result<String, Fault>;
    /// Metadata-only provider pause. Recovery never bypasses normal quota gates.
    async fn cooldown_seconds(&self) -> u64 {
        0
    }
    async fn connect(&self) -> Result<(), Fault>;
    async fn open_cursor(&self) -> Result<(), Fault>;
    fn publish(&self, status: Status);
}
#[derive(Clone, Copy)]
pub struct Options {
    pub name: &'static str,
    pub waiting_message: &'static str,
    pub waiting_timeout: Duration,
    pub poll_interval: Duration,
    pub verification_spacing: Duration,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            name: "Cursor",
            waiting_message: "Finish signing in in Cursor. We’ll connect automatically.",
            waiting_timeout: Duration::from_secs(120),
            poll_interval: Duration::from_secs(1),
            verification_spacing: Duration::from_secs(15),
        }
    }
}
fn update(
    flow: &Flow,
    host: &dyn Host,
    run: u64,
    phase: Phase,
    message: impl Into<String>,
    failure: Option<FaultKind>,
) -> bool {
    let changed = flow.change(run, phase, message, failure);
    if let Some(status) = changed {
        host.publish(status);
    }
    flow.current(run)
}
pub fn failure(flow: &Flow, host: &dyn Host, run: u64, fault: Fault) {
    update(
        flow,
        host,
        run,
        Phase::Error,
        fault.message,
        Some(fault.kind),
    );
}
pub async fn run(flow: Arc<Flow>, run: u64, host: Arc<dyn Host>, options: Options) {
    let deadline = Instant::now() + options.waiting_timeout;
    let mut opened = false;
    let mut attempted: Option<String> = None;
    let mut last_verification: Option<Instant> = None;
    loop {
        if !flow.current(run) {
            return;
        }
        if Instant::now() >= deadline {
            failure(
                &flow,
                host.as_ref(),
                run,
                Fault::new(
                    FaultKind::NeedsRelogin,
                    format!(
                        "Sign-in timed out. Try again after signing in to {}.",
                        options.name
                    ),
                ),
            );
            return;
        }
        let cooldown = tokio::select! {value=host.cooldown_seconds()=>value,_=flow.wake.notified()=>continue,_=tokio::time::sleep_until(deadline)=>continue};
        if cooldown > 0 {
            if !update(&flow,host.as_ref(),run,Phase::Waiting,format!("{} provider pause · retrying in {cooldown}s. No credential or quota request is being made.",options.name),None){return;}
            tokio::select! {_=tokio::time::sleep(options.poll_interval)=>{},_=flow.wake.notified()=>{}}
            continue;
        }
        let probe = tokio::select! {
            value=host.probe()=>value,
            _=flow.wake.notified()=>continue,
            _=tokio::time::sleep_until(deadline)=>continue,
        };
        if !flow.current(run) {
            return;
        }
        match probe {
            Ok(marker)
                if attempted.as_deref() != Some(marker.as_str())
                    && last_verification
                        .is_none_or(|at| at.elapsed() >= options.verification_spacing) =>
            {
                // This phase transition is also the cancellation boundary before any write-capable command.
                if !update(
                    &flow,
                    host.as_ref(),
                    run,
                    Phase::Connecting,
                    format!("Connecting {}…", options.name),
                    None,
                ) {
                    return;
                }
                attempted = Some(marker);
                last_verification = Some(Instant::now());
                match host.connect().await {
                    Ok(()) => {
                        update(
                            &flow,
                            host.as_ref(),
                            run,
                            Phase::Connected,
                            format!("{} connected.", options.name),
                            None,
                        );
                        return;
                    }
                    Err(fault)
                        if matches!(
                            fault.kind,
                            FaultKind::NotSignedIn
                                | FaultKind::NeedsRelogin
                                | FaultKind::IdentityChanged
                        ) => {}
                    Err(fault) => {
                        failure(&flow, host.as_ref(), run, fault);
                        return;
                    }
                }
            }
            Err(fault)
                if !matches!(
                    fault.kind,
                    FaultKind::NotSignedIn | FaultKind::NeedsRelogin | FaultKind::Unreadable
                ) =>
            {
                failure(&flow, host.as_ref(), run, fault);
                return;
            }
            _ => {}
        }
        if !flow.current(run) {
            return;
        }
        if !opened {
            // Cancellation is possible while waiting, including while the native launch is queued.
            if !update(
                &flow,
                host.as_ref(),
                run,
                Phase::Waiting,
                options.waiting_message,
                None,
            ) {
                return;
            }
            if let Err(fault) = host.open_cursor().await {
                failure(&flow, host.as_ref(), run, fault);
                return;
            }
            opened = true;
        }
        if !update(
            &flow,
            host.as_ref(),
            run,
            Phase::Waiting,
            options.waiting_message,
            None,
        ) {
            return;
        }
        tokio::select! {_=tokio::time::sleep(options.poll_interval)=>{},_=flow.wake.notified()=>{}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::atomic::{AtomicUsize, Ordering},
    };
    struct Fake {
        probes: Mutex<VecDeque<Result<String, Fault>>>,
        last: Mutex<Option<String>>,
        results: Mutex<VecDeque<Result<(), Fault>>>,
        opened: AtomicUsize,
        connected: AtomicUsize,
        events: Mutex<Vec<Status>>,
        missing_app: bool,
        slow: bool,
    }
    impl Fake {
        fn new(probes: Vec<Result<String, Fault>>, results: Vec<Result<(), Fault>>) -> Self {
            Self {
                probes: Mutex::new(probes.into()),
                last: Mutex::new(None),
                results: Mutex::new(results.into()),
                opened: AtomicUsize::new(0),
                connected: AtomicUsize::new(0),
                events: Mutex::new(vec![]),
                missing_app: false,
                slow: false,
            }
        }
    }
    #[async_trait]
    impl Host for Fake {
        async fn probe(&self) -> Result<String, Fault> {
            if let Some(value) = self.probes.lock().unwrap().pop_front() {
                if let Ok(marker) = &value {
                    *self.last.lock().unwrap() = Some(marker.clone());
                }
                return value;
            }
            self.last
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| Fault::new(FaultKind::NotSignedIn, "No local sign-in"))
        }
        async fn connect(&self) -> Result<(), Fault> {
            self.connected.fetch_add(1, Ordering::SeqCst);
            if self.slow {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            self.results.lock().unwrap().pop_front().unwrap_or(Ok(()))
        }
        async fn open_cursor(&self) -> Result<(), Fault> {
            self.opened.fetch_add(1, Ordering::SeqCst);
            if self.missing_app {
                Err(Fault::new(
                    FaultKind::ToolMissing,
                    "Open Cursor manually or install it to sign in.",
                ))
            } else {
                Ok(())
            }
        }
        fn publish(&self, status: Status) {
            self.events.lock().unwrap().push(status);
        }
    }
    fn options() -> Options {
        Options {
            waiting_timeout: Duration::from_millis(500),
            poll_interval: Duration::from_millis(1),
            verification_spacing: Duration::from_millis(3),
            ..Default::default()
        }
    }
    async fn execute(fake: Arc<Fake>) -> Arc<Flow> {
        let flow = Arc::new(Flow::default());
        let id = flow.begin().unwrap();
        run(flow.clone(), id, fake, options()).await;
        flow
    }
    struct Paused(Fake);
    #[async_trait]
    impl Host for Paused {
        async fn cooldown_seconds(&self) -> u64 {
            60
        }
        async fn probe(&self) -> Result<String, Fault> {
            self.0.probe().await
        }
        async fn connect(&self) -> Result<(), Fault> {
            self.0.connect().await
        }
        async fn open_cursor(&self) -> Result<(), Fault> {
            self.0.open_cursor().await
        }
        fn publish(&self, status: Status) {
            self.0.publish(status)
        }
    }
    #[tokio::test]
    async fn provider_pause_defers_credentials_launch_and_network_and_remains_cancellable() {
        let host = Arc::new(Paused(Fake::new(
            vec![Ok("private-marker".into())],
            vec![Ok(())],
        )));
        let flow = Arc::new(Flow::default());
        let epoch = flow.begin().unwrap();
        let task = tokio::spawn(run(
            flow.clone(),
            epoch,
            host.clone(),
            Options {
                poll_interval: Duration::from_millis(2),
                ..Options::default()
            },
        ));
        for _ in 0..100 {
            if flow.status().phase == Phase::Waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert!(flow.status().message.contains("provider pause"));
        flow.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(host.0.opened.load(Ordering::SeqCst), 0);
        assert_eq!(host.0.connected.load(Ordering::SeqCst), 0);
        assert_eq!(host.0.probes.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn existing_signin_connects_without_opening_cursor_or_repeated_probing() {
        let host = Arc::new(Fake::new(vec![Ok("private-marker".into())], vec![]));
        let flow = execute(host.clone()).await;
        assert_eq!(flow.status().phase, Phase::Connected);
        assert_eq!(host.connected.load(Ordering::SeqCst), 1);
        assert_eq!(host.opened.load(Ordering::SeqCst), 0);
        assert!(!serde_json::to_string(&flow.status())
            .unwrap()
            .contains("private-marker"));
    }
    #[tokio::test]
    async fn signed_out_opens_once_and_finishes_automatically_after_local_signin() {
        let host = Arc::new(Fake::new(
            vec![
                Err(Fault::new(FaultKind::NotSignedIn, "missing")),
                Ok("new-session".into()),
            ],
            vec![],
        ));
        let flow = execute(host.clone()).await;
        assert_eq!(flow.status().phase, Phase::Connected);
        assert_eq!(host.opened.load(Ordering::SeqCst), 1);
        assert_eq!(host.connected.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn rejected_unchanged_session_is_not_repeatedly_sent_to_the_provider() {
        let host = Arc::new(Fake::new(
            vec![Ok("same-session".into())],
            vec![Err(Fault::new(FaultKind::NeedsRelogin, "rejected"))],
        ));
        let flow = execute(host.clone()).await;
        assert_eq!(flow.status().phase, Phase::Error);
        assert_eq!(host.opened.load(Ordering::SeqCst), 1);
        assert_eq!(host.connected.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn a_changed_signin_can_retry_after_the_spacing_guard() {
        let host = Arc::new(Fake::new(
            vec![Ok("old".into()), Ok("new".into())],
            vec![Err(Fault::new(FaultKind::NeedsRelogin, "rejected")), Ok(())],
        ));
        let flow = execute(host.clone()).await;
        assert_eq!(flow.status().phase, Phase::Connected);
        assert_eq!(host.connected.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn cancellation_stops_waiting_before_any_connection_write() {
        let host = Arc::new(Fake::new(vec![], vec![]));
        let flow = Arc::new(Flow::default());
        let id = flow.begin().unwrap();
        let task = tokio::spawn(run(flow.clone(), id, host.clone(), options()));
        while flow.status().phase != Phase::Waiting {
            tokio::task::yield_now().await;
        }
        assert_eq!(flow.cancel().phase, Phase::Cancelled);
        task.await.unwrap();
        assert_eq!(host.connected.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn verification_is_allowed_to_finish_instead_of_pretending_it_rolled_back() {
        let mut fake = Fake::new(vec![Ok("current".into())], vec![]);
        fake.slow = true;
        let host = Arc::new(fake);
        let flow = Arc::new(Flow::default());
        let id = flow.begin().unwrap();
        let task = tokio::spawn(run(flow.clone(), id, host.clone(), options()));
        while flow.status().phase != Phase::Connecting {
            tokio::task::yield_now().await;
        }
        assert_eq!(flow.cancel().phase, Phase::Connecting);
        task.await.unwrap();
        assert_eq!(flow.status().phase, Phase::Connected);
    }
    #[tokio::test]
    async fn missing_application_and_provider_failures_are_terminal_and_honest() {
        let mut fake = Fake::new(vec![], vec![]);
        fake.missing_app = true;
        let flow = execute(Arc::new(fake)).await;
        assert_eq!(flow.status().failure, Some(FaultKind::ToolMissing));
        let host = Arc::new(Fake::new(
            vec![Ok("current".into())],
            vec![Err(Fault::new(FaultKind::RateLimited, "Please wait"))],
        ));
        let flow = execute(host.clone()).await;
        assert_eq!(flow.status().failure, Some(FaultKind::RateLimited));
        assert_eq!(host.opened.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn disconnect_reset_invalidates_old_runs_and_clears_stale_success() {
        let flow = Flow::default();
        let old = flow.begin().unwrap();
        flow.change(old, Phase::Connecting, "connecting", None);
        let reset = flow.reset();
        assert_eq!(reset.phase, Phase::Idle);
        assert!(reset.message.is_empty());
        assert!(flow
            .change(old, Phase::Connected, "late success", None)
            .is_none());
        assert!(flow.begin().is_some());
    }
    #[test]
    fn duplicate_start_is_idempotent_and_stale_workers_cannot_publish() {
        let flow = Flow::default();
        let old = flow.begin().unwrap();
        assert!(flow.begin().is_none());
        flow.cancel();
        let new = flow.begin().unwrap();
        assert_ne!(old, new);
        assert!(flow.change(old, Phase::Connected, "stale", None).is_none());
        assert_eq!(flow.status().phase, Phase::Checking);
    }
}
