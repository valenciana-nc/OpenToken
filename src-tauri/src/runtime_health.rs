//! One in-process UI heartbeat, not a second process or a repeating app launcher.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_secs(5);
const WARN_AFTER: Duration = Duration::from_secs(30);
const FAIL_AFTER: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Stage {
    Idle,
    Trays,
    Motion,
    Preferences,
}
static STAGE: AtomicU8 = AtomicU8::new(Stage::Idle as u8);
pub struct Activity(u8);
pub fn activity(stage: Stage) -> Activity {
    Activity(STAGE.swap(stage as u8, Ordering::Relaxed))
}
impl Drop for Activity {
    fn drop(&mut self) {
        STAGE.store(self.0, Ordering::Relaxed);
    }
}
fn stage_name() -> &'static str {
    match STAGE.load(Ordering::Relaxed) {
        1 => "tray reconciliation",
        2 => "flyout motion",
        3 => "native preferences",
        _ => "event dispatch / native modal call",
    }
}

#[derive(Default)]
pub struct Monitor(Arc<AtomicBool>);
impl Monitor {
    pub fn start(&self, app: tauri::AppHandle, flyout: Arc<crate::flyout::State>) {
        let ui_thread = crate::platform::current_thread_id();
        let stopping = self.0.clone();
        std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                if app
                    .run_on_main_thread(move || {
                        let _ = tx.send(());
                    })
                    .is_err()
                {
                    break;
                }
                let mut probe = Probe::new(Instant::now());
                loop {
                    if stopping.load(Ordering::Relaxed) {
                        return;
                    }
                    match rx.recv_timeout(POLL) {
                        Ok(()) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    // Native menu/move/modal loops may intentionally defer Tao user tasks.
                    // An unattended context menu is not an unresponsive application.
                    if flyout.modal_count() > 0
                        || crate::platform::native_interaction_active(ui_thread)
                    {
                        probe = Probe::new(Instant::now());
                        continue;
                    }
                    match probe.check(Instant::now()) {
                        Some(Signal::Warn) => {
                            crate::logging::record_failure(&format!(
                                "UI heartbeat delayed for 30 seconds; stage={}",
                                stage_name()
                            ));
                        }
                        Some(Signal::Fail) => {
                            crate::logging::record_failure(&format!(
                                "UI heartbeat unresponsive for 120 seconds; stage={}; exiting with code 1 for Scheduled Task recovery", stage_name()
                            ));
                            std::process::exit(1);
                        }
                        None => {}
                    }
                }
                std::thread::sleep(POLL);
            }
        });
    }
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop();
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Signal {
    Warn,
    Fail,
}
struct Probe {
    since: Instant,
    last_check: Instant,
    warned: bool,
}
impl Probe {
    fn new(now: Instant) -> Self {
        Self {
            since: now,
            last_check: now,
            warned: false,
        }
    }
    fn check(&mut self, now: Instant) -> Option<Signal> {
        // Suspend/resume (or severe machine-wide starvation) is not evidence of an app hang.
        // Require a fresh, continuously observed grace period after a monitoring gap.
        if now.duration_since(self.last_check) > POLL * 3 {
            self.since = now;
            self.warned = false;
        }
        self.last_check = now;
        let elapsed = now.duration_since(self.since);
        if elapsed >= FAIL_AFTER {
            Some(Signal::Fail)
        } else if elapsed >= WARN_AFTER && !self.warned {
            self.warned = true;
            Some(Signal::Warn)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warns_once_then_fails_only_after_a_continuous_two_minute_stall() {
        let now = Instant::now();
        let mut probe = Probe::new(now);
        let mut signals = vec![];
        for seconds in (5..=120).step_by(5) {
            if let Some(signal) = probe.check(now + Duration::from_secs(seconds)) {
                signals.push((seconds, signal));
            }
        }
        assert_eq!(signals, [(30, Signal::Warn), (120, Signal::Fail)]);
    }
    #[test]
    fn suspend_resume_gets_a_new_grace_period() {
        let now = Instant::now();
        let mut probe = Probe::new(now);
        for seconds in (5..=115).step_by(5) {
            probe.check(now + Duration::from_secs(seconds));
        }
        let resumed = now + Duration::from_secs(7200);
        assert_eq!(probe.check(resumed), None);
        for seconds in (5..120).step_by(5) {
            assert_ne!(
                probe.check(resumed + Duration::from_secs(seconds)),
                Some(Signal::Fail)
            );
        }
        assert_eq!(probe.check(resumed + FAIL_AFTER), Some(Signal::Fail));
    }
}
