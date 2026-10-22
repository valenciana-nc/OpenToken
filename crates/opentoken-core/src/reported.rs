//! Original fixed-host adapters for API-key-scoped reported quotas/balances.
//! Authentication proves a key scope, not an invented person/email.
use super::*;
use crate::parser;
use serde_json::Value;
#[derive(Clone)]
pub(super) struct Reported {
    pub zai: String,
    pub zai_cn: String,
    pub kimi: String,
    pub kimi_intl: String,
    pub deepseek: String,
    pub opencode: String,
}
impl Default for Reported {
    fn default() -> Self {
        Self {
            zai: "https://api.z.ai".into(),
            zai_cn: "https://open.bigmodel.cn".into(),
            kimi: "https://api.kimi.com".into(),
            kimi_intl: "https://api.kimi.ai".into(),
            deepseek: "https://api.deepseek.com".into(),
            opencode: "https://opencode.ai".into(),
        }
    }
}
pub fn supported(provider: Provider) -> bool {
    matches!(
        provider,
        Provider::Zai | Provider::Kimi | Provider::Deepseek | Provider::Opencode
    )
}
pub fn region(provider: Provider, value: &str) -> Result<&'static str> {
    match (provider, value) {
        (Provider::Zai, "" | "global") => Ok("global"),
        (Provider::Zai, "cn") => Ok("cn"),
        (Provider::Kimi, "" | "cn") => Ok("cn"),
        (Provider::Kimi, "international") => Ok("international"),
        (Provider::Deepseek | Provider::Opencode, "" | "global") => Ok("global"),
        _ => bail!("Choose a supported provider region."),
    }
}
pub fn key_scope(
    provider: Provider,
    token: &str,
    selected: &str,
) -> Result<(String, String, String)> {
    let selected = region(provider, selected)?;
    let fingerprint = hash(token);
    Ok((
        format!("key-{fingerprint}"),
        format!("{} · API key {}", provider.name(), &fingerprint[..6]),
        format!("{selected}:{fingerprint}"),
    ))
}
pub fn valid_scope(provider: Provider, scope: &str, subject: &str) -> bool {
    scope.split_once(':').is_some_and(|(r, key)| {
        region(provider, r).is_ok_and(|normalized| normalized == r)
            && key.len() == 24
            && key.bytes().all(|b| b.is_ascii_hexdigit())
            && subject == format!("key-{key}")
    })
}
fn malformed() -> Fault {
    Fault::new(FaultKind::Unreadable,"The provider did not return a recognized quota or balance. No missing usage was assumed to be zero.")
}
fn numeric(value: &Value) -> Result<f64, Fault> {
    parser::number(value)
        .filter(|n| n.is_finite() && *n >= 0. && *n <= 1e15)
        .ok_or_else(malformed)
}
fn reset(value: &Value) -> Option<chrono::DateTime<Utc>> {
    if value.is_null() {
        None
    } else if let Some(text) = value.as_str() {
        chrono::DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|d| d.with_timezone(&Utc))
    } else {
        value
            .as_i64()
            .and_then(chrono::DateTime::from_timestamp_millis)
    }
}
fn quota(
    id: impl Into<String>,
    label: impl Into<String>,
    percent: f64,
    resets_at: Option<chrono::DateTime<Utc>>,
) -> Result<Quota, Fault> {
    if !percent.is_finite() || !(0. ..=1e15).contains(&percent) {
        return Err(malformed());
    }
    Ok(Quota {
        id: id.into(),
        label: label.into(),
        percent: Some(percent),
        value: None,
        unit: None,
        resets_at,
    })
}
fn window(minutes: Option<u64>) -> String {
    match minutes {
        Some(300) => "5-hour quota".into(),
        Some(10080) => "Weekly quota".into(),
        Some(n) => format!("{n}-minute quota"),
        None => "Reported quota".into(),
    }
}
fn report(plan: Option<String>, quotas: Vec<Quota>) -> Result<Report, Fault> {
    if quotas.is_empty() {
        return Err(malformed());
    }
    let now = Utc::now();
    Ok(Report {
        plan,
        quotas,
        observed_at: now,
        verified_at: Some(now),
        source: ReportSource::Provider,
    })
}
fn plan(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100 && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
pub fn parse(provider: Provider, value: &Value) -> Result<Report, Fault> {
    match provider {
        Provider::Deepseek => {
            if !value["is_available"].is_boolean() {
                return Err(malformed());
            }
            let balances = value["balance_infos"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 20)
                .ok_or_else(malformed)?;
            let mut quotas = vec![];
            let mut currencies = HashSet::new();
            for entry in balances {
                let currency = entry["currency"]
                    .as_str()
                    .filter(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase()))
                    .ok_or_else(malformed)?;
                if !currencies.insert(currency) {
                    return Err(malformed());
                }
                for (field, label) in [
                    ("total_balance", "Available balance"),
                    ("granted_balance", "Granted balance"),
                    ("topped_up_balance", "Paid balance"),
                ] {
                    quotas.push(Quota {
                        id: format!("deepseek_{currency}_{field}"),
                        label: format!("{label} · {currency}"),
                        percent: None,
                        value: Some(numeric(&entry[field])?),
                        unit: Some(currency.into()),
                        resets_at: None,
                    });
                }
            }
            report(
                Some(
                    if value["is_available"] == true {
                        "API balance"
                    } else {
                        "Balance unavailable for API calls"
                    }
                    .into(),
                ),
                quotas,
            )
        }
        Provider::Zai => {
            if value["success"] != true || value["code"] != 200 {
                return Err(Fault::new(FaultKind::PermissionDenied,"Z.ai did not grant Coding Plan quota access to this key. Check the selected region and plan."));
            }
            let limits = value["data"]["limits"]
                .as_array()
                .filter(|a| a.len() <= 32)
                .ok_or_else(malformed)?;
            let mut quotas = vec![];
            for (index, limit) in limits.iter().enumerate() {
                let kind = limit["type"].as_str().ok_or_else(malformed)?;
                if !["TOKENS_LIMIT", "CREDIT_LIMIT", "TIME_LIMIT"].contains(&kind) {
                    continue;
                }
                let mut used = numeric(&limit["percentage"])?;
                if let Ok(cap) = numeric(&limit["usage"]) {
                    if cap > 0. {
                        if let Ok(current) = numeric(&limit["currentValue"]) {
                            used = current / cap * 100.;
                        } else if let Ok(remaining) = numeric(&limit["remaining"]) {
                            used = (cap - remaining).max(0.) / cap * 100.;
                        }
                    }
                }
                let multiplier = match limit["unit"].as_u64() {
                    Some(1) => Some(1440),
                    Some(3) => Some(60),
                    Some(5) => Some(1),
                    Some(6) => Some(10080),
                    _ => None,
                };
                let minutes = limit["number"]
                    .as_u64()
                    .zip(multiplier)
                    .and_then(|(n, m)| n.checked_mul(m));
                let reset_at = reset(&limit["nextResetTime"]).filter(|d| {
                    minutes != Some(300) || *d <= Utc::now() + chrono::Duration::minutes(301)
                });
                quotas.push(quota(
                    format!("zai_{index}"),
                    if kind == "TIME_LIMIT" {
                        if limit["unit"] == 5 && limit["number"] == 1 {
                            "Monthly MCP quota".into()
                        } else {
                            format!("MCP · {}", window(minutes))
                        }
                    } else {
                        window(minutes)
                    },
                    used,
                    reset_at,
                )?);
            }
            report(
                ["planName", "plan", "plan_type", "packageName", "level"]
                    .into_iter()
                    .find_map(|key| plan(&value["data"][key])),
                quotas,
            )
        }
        Provider::Kimi => {
            let mut quotas = vec![];
            for (key, id, label) in [
                ("limit_5h", "kimi_session", "5-hour quota"),
                ("limit_7d", "kimi_weekly", "Weekly quota"),
                ("limit_month_total", "kimi_monthly", "Monthly total"),
            ] {
                let pool = &value["usages"][key];
                if !pool.is_null() {
                    quotas.push(quota(
                        id,
                        label,
                        numeric(&pool["used_ratio"])? * 100.,
                        reset(&pool["reset_time"]),
                    )?);
                }
            }
            let mut add_counts = |detail: &Value, id: &str, label: String| -> Result<(), Fault> {
                if detail.is_null() {
                    return Ok(());
                }
                let cap = numeric(&detail["limit"])?;
                if cap <= 0. {
                    return Err(malformed());
                }
                let used = if !detail["used"].is_null() {
                    numeric(&detail["used"])?
                } else {
                    let remaining = numeric(&detail["remaining"])?;
                    if remaining > cap {
                        return Err(malformed());
                    }
                    cap - remaining
                };
                let at = reset(&detail["resetTime"]);
                let percent = used / cap * 100.;
                if let Some(existing) = quotas.iter_mut().find(|q| q.id == id) {
                    if existing
                        .resets_at
                        .zip(at)
                        .is_some_and(|(a, b)| (a - b).num_milliseconds().abs() <= 2000)
                    {
                        existing.percent = Some(existing.percent.unwrap().max(percent));
                    }
                } else {
                    quotas.push(quota(id, label, percent, at)?);
                }
                Ok(())
            };
            add_counts(&value["usage"], "kimi_weekly", "Weekly quota".into())?;
            if let Some(limits) = value["limits"].as_array() {
                if limits.len() > 32 {
                    return Err(malformed());
                }
                for (index, limit) in limits.iter().enumerate() {
                    let minutes = limit["window"]["duration"].as_u64().and_then(|n| {
                        match limit["window"]["timeUnit"].as_str() {
                            Some("TIME_UNIT_MINUTE") => Some(n),
                            Some("TIME_UNIT_HOUR") => n.checked_mul(60),
                            Some("TIME_UNIT_DAY") => n.checked_mul(1440),
                            _ => None,
                        }
                    });
                    let id = if minutes == Some(300) {
                        "kimi_session".into()
                    } else {
                        format!("kimi_limit_{index}")
                    };
                    add_counts(&limit["detail"], &id, window(minutes))?;
                }
            }
            report(plan(&value["user"]["membership"]["level"]), quotas)
        }
        Provider::Opencode => {
            let mut quotas = vec![];
            let usage = value["usage"].as_object().ok_or_else(malformed)?;
            for (key, label) in [
                ("rolling", "5-hour quota"),
                ("weekly", "Weekly quota"),
                ("monthly", "Monthly quota"),
            ] {
                if let Some(pool) = usage.get(key).filter(|v| !v.is_null()) {
                    let percent = if !pool["percent"].is_null() {
                        numeric(&pool["percent"])?
                    } else {
                        let cap = numeric(&pool["limit"])?;
                        if cap <= 0. {
                            return Err(malformed());
                        }
                        numeric(&pool["used"])? / cap * 100.
                    };
                    let at = reset(&pool["resetAt"])
                        .or_else(|| reset(&pool["resetsAt"]))
                        .or_else(|| {
                            pool["resetInSec"]
                                .as_i64()
                                .filter(|n| *n >= 0 && *n < 366 * 86400)
                                .and_then(|n| {
                                    Utc::now().checked_add_signed(chrono::Duration::seconds(n))
                                })
                        });
                    quotas.push(quota(format!("opencode_{key}"), label, percent, at)?);
                }
            }
            report(Some("OpenCode Go".into()), quotas)
        }
        _ => Err(malformed()),
    }
}
impl Service {
    pub(super) async fn reported_usage(
        &self,
        provider: Provider,
        token: &str,
        region: &str,
    ) -> Result<Report, Fault> {
        let base = match (provider, region) {
            (Provider::Deepseek, _) => &self.reported.deepseek,
            (Provider::Zai, "cn") => &self.reported.zai_cn,
            (Provider::Zai, _) => &self.reported.zai,
            (Provider::Kimi, "international") => &self.reported.kimi_intl,
            (Provider::Kimi, _) => &self.reported.kimi,
            (Provider::Opencode, _) => &self.reported.opencode,
            _ => return Err(malformed()),
        };
        let path = match provider {
            Provider::Deepseek => "/user/balance",
            Provider::Zai => "/api/monitor/usage/quota/limit",
            Provider::Kimi => "/coding/v1/usages",
            Provider::Opencode => "/zen/go/v1/usage",
            _ => return Err(malformed()),
        };
        let response: Value = self
            .json(
                self.http
                    .get(format!("{base}{path}"))
                    .bearer_auth(token)
                    .header(reqwest::header::ACCEPT, "application/json"),
                false,
            )
            .await?
            .ok_or_else(malformed)?;
        parse(provider, &response)
    }
}
#[cfg(test)]
#[path = "reported/tests.rs"]
mod tests;
