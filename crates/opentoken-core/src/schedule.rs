//! Per-identity quota budget. Local log indexing never consumes this network/CLI budget.
use crate::{
    engine::retry_delay,
    model::{Fault, Report},
};
use chrono::Utc;
use std::{collections::VecDeque, time::Duration};
use tokio::time::Instant;
const MIN_READ: Duration = Duration::from_secs(60);
const ACTIVE_FOR: Duration = Duration::from_secs(600);
const HOUR: Duration = Duration::from_secs(3600);
const MAX_READS: usize = 60;
#[derive(Clone)]
pub struct Schedule {
    pub at: Instant,
    pub failures: u32,
    last_attempt: Option<Instant>,
    active_until: Option<Instant>,
    reads: VecDeque<Instant>,
}
impl Schedule {
    pub fn new(now: Instant) -> Self {
        Self {
            at: now,
            failures: 0,
            last_attempt: None,
            active_until: None,
            reads: VecDeque::new(),
        }
    }
    fn budget(&self, now: Instant) -> Instant {
        let recent: Vec<_> = self
            .reads
            .iter()
            .filter(|t| now.saturating_duration_since(**t) < HOUR)
            .collect();
        if recent.len() >= MAX_READS {
            *recent[0] + HOUR
        } else {
            now
        }
    }
    pub fn due(&self, now: Instant, manual: bool) -> bool {
        now >= self.budget(now)
            && (now >= self.at
                || (manual
                    && self.failures == 0
                    && self.last_attempt.is_none_or(|last| {
                        now.saturating_duration_since(last) >= Duration::from_secs(2)
                    })))
    }
    pub fn request_manual(&mut self, now: Instant) {
        if self.failures == 0 {
            let gate = self
                .last_attempt
                .map_or(now, |last| last + Duration::from_secs(2))
                .max(self.budget(now));
            self.at = self.at.min(gate);
        }
    }
    pub fn activity(&mut self, now: Instant, confirmation: bool) {
        self.active_until = Some(now + ACTIVE_FOR);
        if self.failures != 0 {
            return;
        }
        let soon = now + Duration::from_secs(if confirmation { 20 } else { 2 });
        let gate = self
            .last_attempt
            .map_or(soon, |last| (last + MIN_READ).max(soon))
            .max(self.budget(now));
        self.at = self.at.min(gate);
    }
    pub fn credentials_changed(&mut self, now: Instant) {
        self.failures = 0;
        self.at = now
            .max(
                self.last_attempt
                    .map_or(now, |last| last + Duration::from_secs(2)),
            )
            .max(self.budget(now));
    }
    pub fn disable_live(&mut self, now: Instant, minutes: u32) {
        self.active_until = None;
        if self.failures == 0 {
            self.at = self
                .last_attempt
                .map_or(now, |last| {
                    last + Duration::from_secs(u64::from(minutes) * 60)
                })
                .max(self.budget(now));
        }
    }
    pub fn record(
        &mut self,
        now: Instant,
        result: &Result<Report, Fault>,
        minutes: u32,
        live: bool,
        id: &str,
    ) {
        self.last_attempt = Some(now);
        while self
            .reads
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) >= HOUR)
        {
            self.reads.pop_front();
        }
        self.reads.push_back(now);
        self.failures = if result.is_ok() {
            0
        } else {
            self.failures.saturating_add(1)
        };
        let mut delay = retry_delay(minutes, self.failures, result.as_ref().err(), id);
        if result.is_ok() && live {
            if self.active_until.is_some_and(|until| until > now) {
                delay = delay.min(MIN_READ);
            }
            if let Ok(report) = result {
                // Refresh shortly after an actual advertised reset; never invent a restored quota.
                for reset in report.quotas.iter().filter_map(|q| q.resets_at) {
                    if let Ok(until) = (reset - Utc::now()).to_std() {
                        delay = delay.min((until + Duration::from_secs(2)).max(MIN_READ));
                    }
                }
            }
        }
        self.at = (now + delay).max(self.budget(now));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FaultKind, ReportSource};
    fn report() -> Result<Report, Fault> {
        Ok(Report {
            plan: None,
            quotas: vec![],
            observed_at: Utc::now(),
            source: ReportSource::Provider,
            verified_at: Some(Utc::now()),
        })
    }
    #[test]
    fn active_quota_reads_are_spaced_and_relax_when_disabled() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.record(now, &report(), 5, true, "a");
        s.activity(now + Duration::from_secs(3), false);
        assert_eq!(s.at, now + MIN_READ);
        assert!(!s.due(now + Duration::from_secs(59), false));
        assert!(s.due(now + MIN_READ, false));
        s.disable_live(now + MIN_READ, 5);
        assert_eq!(s.at, now + Duration::from_secs(300));
    }
    #[test]
    fn activity_and_manual_requests_do_not_bypass_server_backoff() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.record(
            now,
            &Err(Fault {
                kind: FaultKind::RateLimited,
                message: "wait".into(),
                retry_after_secs: Some(600),
            }),
            5,
            true,
            "a",
        );
        let due = s.at;
        s.activity(now + Duration::from_secs(1), false);
        assert_eq!(s.at, due);
        assert!(!s.due(now + MIN_READ, true));
    }
    #[test]
    fn manual_bursts_queue_one_read_after_the_short_click_guard() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.record(now, &report(), 5, true, "a");
        s.request_manual(now + Duration::from_millis(100));
        assert_eq!(s.at, now + Duration::from_secs(2));
        assert!(!s.due(now + Duration::from_secs(1), false));
        assert!(s.due(now + Duration::from_secs(2), false));
    }
    #[test]
    fn credential_changes_keep_the_read_budget_and_short_spacing_guard() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.record(now, &report(), 1, true, "a");
        s.credentials_changed(now + Duration::from_millis(10));
        assert!(!s.due(now + Duration::from_secs(1), false));
        for i in 1..60 {
            s.record(now + Duration::from_secs(i * 2), &report(), 1, true, "a");
        }
        s.credentials_changed(now + Duration::from_secs(200));
        assert!(!s.due(now + Duration::from_secs(200), false));
        assert!(s.due(now + HOUR, false));
    }
    #[test]
    fn budget_caps_manual_and_automatic_reads_per_identity() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        for i in 0..60 {
            s.record(now + Duration::from_secs(i * 2), &report(), 1, true, "a");
        }
        assert!(!s.due(now + Duration::from_secs(200), true));
        assert!(s.due(now + HOUR, true));
    }
}
