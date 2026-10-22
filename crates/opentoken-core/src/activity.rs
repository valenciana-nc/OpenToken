//! Ephemeral activity hints. A directory is not proof of a historical signed-in identity.
use crate::model::{AccountState, Identity, Provider, Quota, ReportSource, Status, Store};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Activity {
    pub provider: Provider,
    pub home: PathBuf,
    pub observed_at: DateTime<Utc>,
    pub identity: Option<Identity>,
    pub quota: Option<LogQuota>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct LogQuota {
    pub primary: Option<LogWindow>,
    pub secondary: Option<LogWindow>,
    pub limit_id: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct LogWindow {
    pub used_percent: f64,
    pub window_minutes: u32,
    pub resets_at: i64,
}
#[derive(Deserialize)]
struct Record {
    timestamp: DateTime<Utc>,
    r#type: String,
    payload: Payload,
}
#[derive(Deserialize)]
struct Payload {
    r#type: Option<String>,
    rate_limits: Option<LogQuota>,
    // Optional explicit proof only. Current anonymous Codex rollouts normally omit this.
    account: Option<Identity>,
}
pub fn quota_record(line: &[u8], home: PathBuf) -> Option<Activity> {
    let record: Record = serde_json::from_slice(line).ok()?;
    if record.r#type != "event_msg" || record.payload.r#type.as_deref() != Some("token_count") {
        return None;
    }
    let quota = record.payload.rate_limits?;
    if quota.limit_id.as_deref().is_some_and(|id| id != "codex") {
        return None;
    }
    let mut windows = quota.primary.iter().chain(quota.secondary.iter());
    if !windows.clone().any(|_| true)
        || !windows.all(|w| {
            w.used_percent.is_finite()
                && (0.0..=100.0).contains(&w.used_percent)
                && [300, 10080].contains(&w.window_minutes)
                && DateTime::from_timestamp(w.resets_at, 0).is_some()
        })
    {
        return None;
    }
    Some(Activity {
        provider: Provider::Codex,
        home,
        observed_at: record.timestamp,
        identity: record.payload.account,
        quota: Some(quota),
    })
}
impl Activity {
    pub fn fresh(&self, since: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        self.observed_at >= since
            && self.observed_at <= now + chrono::Duration::seconds(5)
            && self.observed_at >= now - chrono::Duration::minutes(2)
    }
    pub fn matches_home(&self, account: &AccountState) -> bool {
        account.account.provider == self.provider
            && account
                .account
                .sources
                .iter()
                .any(|s| crate::storage::path_key(&s.path) == crate::storage::path_key(&self.home))
    }
    /// Fail closed: anonymous records only request provider confirmation, never change an icon.
    pub fn apply(&self, state: &mut AccountState, now: DateTime<Utc>) -> bool {
        if state.status != Status::Ready
            || self.provider != Provider::Codex
            || !self.matches_home(state)
        {
            return false;
        }
        let (Some(proof), Some(expected), Some(limits), Some(report)) = (
            self.identity.as_ref(),
            state.account.identity.as_ref(),
            self.quota.as_ref(),
            state.report.as_ref(),
        ) else {
            return false;
        };
        if !proof.matches(expected)
            || (proof.email.is_none() && proof.user.is_none())
            || limits.limit_id.as_deref().is_some_and(|id| id != "codex")
            || proof.workspace.starts_with("home:")
            || proof.workspace.len() > 256
            || self.observed_at <= report.observed_at
            || !self.fresh(now - chrono::Duration::minutes(2), now)
            || !state.account.sources.iter().any(|s| {
                s.store == Store::File
                    && crate::storage::path_key(&s.path) == crate::storage::path_key(&self.home)
            })
        {
            return false;
        }
        let Ok(before) = crate::codex::read_auth(&self.home) else {
            return false;
        };
        if !before.identity.matches(expected) {
            return false;
        }
        let mut next = report.clone();
        let mut changed = false;
        for window in limits.primary.iter().chain(limits.secondary.iter()) {
            let (id, label) = match window.window_minutes {
                300 => ("session", "Session"),
                10080 => ("weekly", "Weekly"),
                _ => return false,
            };
            let Some(reset) = DateTime::from_timestamp(window.resets_at, 0) else {
                return false;
            };
            if !window.used_percent.is_finite()
                || !(0.0..=100.0).contains(&window.used_percent)
                || reset <= self.observed_at
                || reset > now + chrono::Duration::days(15)
            {
                return false;
            }
            let Some(existing) = next.quotas.iter_mut().find(|q| q.id == id) else {
                return false;
            };
            // Do not infer a reset, decreasing consumption, or a changed window from a log hint.
            if existing.resets_at != Some(reset)
                || existing
                    .percent
                    .is_none_or(|used| window.used_percent < used)
            {
                return false;
            }
            changed |= existing.percent != Some(window.used_percent);
            *existing = Quota::window(id, label, window.used_percent, Some(reset));
        }
        if !changed {
            return false;
        }
        let Ok(after) = crate::codex::read_auth(&self.home) else {
            return false;
        };
        if !after.identity.matches(expected) {
            return false;
        }
        next.verified_at = report.verified_at.or(Some(report.observed_at));
        next.observed_at = self.observed_at;
        next.source = ReportSource::CliLog;
        state.report = Some(next);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn validates_typed_quota_records_and_never_treats_text_as_usage() {
        let value = json!({"timestamp":Utc::now(),"type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":25,"window_minutes":300,"resets_at":Utc::now().timestamp()+3600}}}});
        let event =
            quota_record(&serde_json::to_vec(&value).unwrap(), PathBuf::from("home")).unwrap();
        assert!(event.identity.is_none());
        let mut bad = value.clone();
        bad["payload"]["rate_limits"]["primary"]["used_percent"] = json!(101);
        assert!(quota_record(&serde_json::to_vec(&bad).unwrap(), "home".into()).is_none());
        bad = value;
        bad["type"] = json!("user_message");
        assert!(quota_record(&serde_json::to_vec(&bad).unwrap(), "home".into()).is_none());
    }
}
