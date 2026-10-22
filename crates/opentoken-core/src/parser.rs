//! Wire-format adapters. Both Codex protocols map directly to the same domain model.
use crate::model::{Fault, FaultKind, Quota, Report};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;

pub fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}
pub fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}
pub fn date(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        })
        .or_else(|| {
            let n = number(v)?;
            DateTime::from_timestamp_millis(if n.abs() < 1e10 {
                (n * 1000.0) as i64
            } else {
                n as i64
            })
        })
}
pub fn plan(value: Option<String>) -> Option<String> {
    value.map(|s| match s.to_lowercase().as_str() {
        "prolite" => "Pro 100".into(),
        "pro" => "Pro 200".into(),
        "promax" => "Pro 500".into(),
        "self_serve_business_prolite" => "Business Premium".into(),
        _ => s
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|a| a.to_uppercase().to_string() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    })
}
fn label(seconds: Option<f64>, fallback: &str, spark: bool) -> String {
    let label = match seconds {
        Some(18000.0) => "Session".to_string(),
        Some(604800.0) => "Weekly".to_string(),
        Some(n) if (82080.0..=90720.0).contains(&n) => "Daily".into(),
        Some(n) if (2419200.0..=2764800.0).contains(&n) => "Monthly".into(),
        Some(n) if (29959200.0..=33112800.0).contains(&n) => "Annual".into(),
        Some(n) if n > 0.0 => {
            if n >= 3600.0 {
                format!("{}-hour usage", n / 3600.0)
            } else {
                format!("{}-minute usage", n / 60.0)
            }
        }
        _ => fallback.into(),
    };
    if spark {
        if label == "Session" {
            "Spark".into()
        } else {
            format!("Spark {label}")
        }
    } else {
        label
    }
}
fn windows(
    quotas: &mut Vec<Quota>,
    v: &Value,
    cli: bool,
    spark: bool,
    headers: &HashMap<String, String>,
    now: DateTime<Utc>,
) {
    for (name, fallback) in [("primary", "Session"), ("secondary", "Weekly")] {
        let w = &v[if cli {
            name.to_string()
        } else {
            format!("{name}_window")
        }];
        let percent = number(&w[if cli { "usedPercent" } else { "used_percent" }]).or_else(|| {
            if spark {
                None
            } else {
                headers
                    .get(&format!("x-codex-{name}-used-percent"))
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|n| n.is_finite())
            }
        });
        if let Some(percent) = percent {
            let seconds = if cli {
                number(&w["windowDurationMins"]).map(|m| m * 60.0)
            } else {
                number(&w["limit_window_seconds"])
            };
            let label = label(seconds, fallback, spark);
            let reset = date(&w[if cli { "resetsAt" } else { "reset_at" }]).or_else(|| {
                let seconds = number(&w["reset_after_seconds"])?;
                if !(0.0..=315360000.0).contains(&seconds) {
                    return None;
                }
                now.checked_add_signed(Duration::seconds(seconds as i64))
            });
            let id = label.to_lowercase().replace(' ', "-");
            if !quotas.iter().any(|q| q.id == id) {
                quotas.push(Quota::window(&id, &label, percent, reset));
            }
        }
    }
}
fn balances(
    quotas: &mut Vec<Quota>,
    credits: &Value,
    resets: &Value,
    cli: bool,
    headers: &HashMap<String, String>,
) {
    let balance = number(&credits["balance"])
        .or_else(|| {
            if credits[if cli { "hasCredits" } else { "has_credits" }].as_bool() == Some(false) {
                Some(0.0)
            } else {
                headers
                    .get("x-codex-credits-balance")
                    .and_then(|s| s.parse().ok())
            }
        })
        .filter(|n| n.is_finite());
    if let Some(n) = balance {
        quotas.push(Quota::value(
            "credits",
            "Credits",
            n.max(0.0).floor(),
            "credits",
        ));
    }
    if let Some(n) = number(
        &resets[if cli {
            "availableCount"
        } else {
            "available_count"
        }],
    )
    .filter(|n| *n >= 0.0)
    {
        quotas.push(Quota::value(
            "resets",
            "Rate limit resets",
            n.floor(),
            "available",
        ));
    }
}
fn finish(plan: Option<String>, quotas: Vec<Quota>, now: DateTime<Utc>) -> Result<Report, Fault> {
    if quotas.is_empty() {
        return Err(Fault::new(
            FaultKind::Unknown,
            "The provider returned no recognized usage windows. Try updating OpenToken or the CLI.",
        ));
    }
    Ok(Report {
        plan,
        quotas,
        observed_at: now,
        source: crate::model::ReportSource::Provider,
        verified_at: Some(now),
    })
}
pub fn codex_http(
    v: &Value,
    headers: &HashMap<String, String>,
    resets: Option<&Value>,
    now: DateTime<Utc>,
) -> Result<Report, Fault> {
    let mut quotas = vec![];
    windows(&mut quotas, &v["rate_limit"], false, false, headers, now);
    if let Some(additional) = v["additional_rate_limits"].as_array() {
        for bucket in additional {
            if ["limit_name", "metered_feature"].iter().any(|key| {
                bucket[key]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("spark")
            }) {
                windows(
                    &mut quotas,
                    &bucket["rate_limit"],
                    false,
                    true,
                    headers,
                    now,
                );
                break;
            }
        }
    }
    balances(
        &mut quotas,
        &v["credits"],
        resets
            .filter(|r| number(&r["available_count"]).is_some())
            .unwrap_or(&v["rate_limit_reset_credits"]),
        false,
        headers,
    );
    finish(plan(text(&v["plan_type"])), quotas, now)
}
pub fn codex_cli(
    v: &Value,
    account_plan: Option<String>,
    now: DateTime<Utc>,
) -> Result<Report, Fault> {
    let headers = HashMap::new();
    let mut quotas = vec![];
    let mut p = account_plan;
    if let Some(buckets) = v["rateLimitsByLimitId"].as_object() {
        for (id, bucket) in buckets {
            if id == "codex" {
                windows(&mut quotas, bucket, true, false, &headers, now);
                p = text(&bucket["planType"]).or(p);
                balances(
                    &mut quotas,
                    &bucket["credits"],
                    &Value::Null,
                    true,
                    &headers,
                );
            } else if id.to_lowercase().contains("spark")
                || bucket["limitName"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("spark")
            {
                windows(&mut quotas, bucket, true, true, &headers, now);
            }
        }
    } else {
        let main = &v["rateLimits"];
        windows(&mut quotas, main, true, false, &headers, now);
        p = text(&main["planType"]).or(p);
        balances(&mut quotas, &main["credits"], &Value::Null, true, &headers);
    }
    balances(
        &mut quotas,
        &Value::Null,
        &v["rateLimitResetCredits"],
        true,
        &headers,
    );
    finish(plan(p), quotas, now)
}
pub fn claude(
    v: &Value,
    subscription: Option<String>,
    tier: Option<String>,
    now: DateTime<Utc>,
) -> Result<Report, Fault> {
    let mut quotas = vec![];
    for (key, label) in [
        ("five_hour", "Session"),
        ("seven_day", "Weekly"),
        ("seven_day_sonnet", "Sonnet"),
        ("seven_day_opus", "Opus"),
    ] {
        if let Some(n) = number(&v[key]["utilization"]) {
            quotas.push(Quota::window(key, label, n, date(&v[key]["resets_at"])));
        }
    }
    if let Some(limits) = v["limits"].as_array() {
        for limit in limits {
            if limit["kind"] == "weekly_scoped" {
                if let (Some(name), Some(n)) = (
                    text(&limit["scope"]["model"]["display_name"]),
                    number(&limit["percent"]),
                ) {
                    quotas.push(Quota::window(
                        &format!("model-{}", name.to_lowercase()),
                        &name,
                        n,
                        date(&limit["resets_at"]),
                    ));
                }
            }
        }
    }
    let extra = &v["extra_usage"];
    if extra["is_enabled"].as_bool() == Some(true) {
        if let Some(used) = number(&extra["used_credits"]) {
            let mut q = Quota::value("extra", "Extra usage spent", used / 100.0, "USD");
            q.percent = number(&extra["monthly_limit"])
                .filter(|n| *n > 0.0)
                .map(|limit| (used / limit * 100.0).clamp(0.0, 100.0));
            quotas.push(q);
        }
    }
    let mut p = subscription.map(|s| {
        s.split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|a| a.to_uppercase().to_string() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    });
    if let (Some(p), Some(tier)) = (&mut p, tier) {
        if let Some(multiplier) = tier
            .split('_')
            .find(|s| s.ends_with('x') && s.trim_end_matches('x').parse::<u32>().is_ok())
        {
            p.push(' ');
            p.push_str(multiplier);
        }
    }
    finish(p, quotas, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn maps_http_session_weekly_balances_plan() {
        let v = json!({"plan_type":"pro","rate_limit":{"primary_window":{"used_percent":15,"limit_window_seconds":18000,"reset_after_seconds":300},"secondary_window":{"used_percent":40,"limit_window_seconds":604800}},"credits":{"balance":821.9},"rate_limit_reset_credits":{"available_count":2}});
        let r = codex_http(&v, &HashMap::new(), None, DateTime::UNIX_EPOCH).unwrap();
        assert_eq!(r.plan.as_deref(), Some("Pro 200"));
        assert_eq!(r.quotas.len(), 4);
        assert_eq!(r.quotas[0].percent, Some(15.0));
        assert_eq!(r.quotas[0].resets_at.unwrap().timestamp(), 300);
    }
    #[test]
    fn weekly_only_and_monthly_not_session() {
        for (seconds, name) in [
            (604800, "Weekly"),
            (2592000, "Monthly"),
            (86400, "Daily"),
            (31536000, "Annual"),
        ] {
            let v = json!({"rate_limit":{"primary_window":{"used_percent":34,"limit_window_seconds":seconds}}});
            let r = codex_http(&v, &HashMap::new(), None, Utc::now()).unwrap();
            assert_eq!(r.quotas[0].label, name);
        }
    }
    #[test]
    fn cli_spark_and_reset() {
        let v = json!({"rateLimitsByLimitId":{"codex":{"planType":"plus","primary":{"usedPercent":42,"windowDurationMins":10080},"credits":{"hasCredits":false}},"spark":{"primary":{"usedPercent":8,"windowDurationMins":300}}},"rateLimitResetCredits":{"availableCount":2}});
        let r = codex_cli(&v, None, Utc::now()).unwrap();
        assert!(r
            .quotas
            .iter()
            .any(|q| q.label == "Weekly" && q.percent == Some(42.0)));
        assert!(r.quotas.iter().any(|q| q.label == "Spark"));
        assert!(!r.quotas.iter().any(|q| q.label == "Session"));
    }
    #[test]
    fn claude_extra_scoped_and_tier() {
        let v = json!({"five_hour":{"utilization":12},"extra_usage":{"is_enabled":true,"used_credits":250,"monthly_limit":1000},"limits":[{"kind":"weekly_scoped","percent":44,"scope":{"model":{"display_name":"Fable"}}}]});
        let r = claude(
            &v,
            Some("pro".into()),
            Some("default_claude_max_20x".into()),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(r.plan.as_deref(), Some("Pro 20x"));
        assert!(r
            .quotas
            .iter()
            .any(|q| q.id == "extra" && q.percent == Some(25.0)));
        assert!(r.quotas.iter().any(|q| q.label == "Fable"));
    }
    #[test]
    fn bad_dates_and_numbers_do_not_panic() {
        assert!(date(&json!(1e300)).is_none());
        assert!(number(&json!("NaN")).is_none());
        assert!(codex_cli(&json!({}), None, Utc::now()).is_err());
    }
    #[test]
    fn headers_and_zero_credits() {
        let r = codex_http(
            &json!({"credits":{"has_credits":false}}),
            &HashMap::from([("x-codex-primary-used-percent".into(), "11".into())]),
            None,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(r.quotas[0].percent, Some(11.0));
        assert_eq!(r.quotas[1].value, Some(0.0));
    }
}
