use super::*;

pub(super) struct Endpoint {
    pub pid: u32,
    pub born: u64,
    pub port: u16,
    pub csrf: Zeroizing<String>,
}
impl Endpoint {
    pub fn stamp(&self) -> String {
        hash(&format!(
            "{}:{}:{}:{}",
            self.pid,
            self.born,
            self.port,
            self.csrf.as_str()
        ))
    }
}
pub(super) trait Desktop: Send + Sync {
    fn locate(&self, profile: &Path) -> Result<Vec<Endpoint>, Fault>;
}
#[cfg(not(windows))]
pub(super) struct Unavailable;
#[cfg(not(windows))]
impl Desktop for Unavailable {
    fn locate(&self, _: &Path) -> Result<Vec<Endpoint>, Fault> {
        Err(denied("Antigravity desktop ownership verification is currently implemented for native Windows only."))
    }
}
pub(super) async fn endpoints(
    host: Arc<dyn Desktop>,
    profile: &Path,
) -> Result<Vec<Endpoint>, Fault> {
    let profile = profile.to_owned();
    tokio::task::spawn_blocking(move || host.locate(&profile))
        .await
        .map_err(|_| Fault::offline())?
}
fn identity(value: &Value) -> Result<String, Fault> {
    if value["userStatus"]["email"].is_null() {
        return Err(Fault::new(FaultKind::NotSignedIn,"Finish sign-in in the selected official Antigravity client. Its local service has not supplied an authenticated account yet."));
    }
    value["userStatus"]["email"].as_str().filter(|s|email(s)).map(|s|s.to_lowercase())
        .ok_or_else(||denied("Antigravity did not establish a supported signed-in account identity. No anonymous quota was assigned to an account."))
}
impl Service {
    pub(super) async fn read_antigravity(&self, profile: &Path) -> Result<Reading, Fault> {
        let found = endpoints(self.local.desktop.clone(), profile).await?;
        if found.is_empty() {
            return Err(Fault::new(FaultKind::Offline,"Antigravity's monitored desktop quota service is not running for this profile. Open the same signed-in app/IDE; no login or account switch was started."));
        }
        if found.iter().map(|e| e.pid).collect::<HashSet<_>>().len() > 1 {
            return Err(denied("More than one Antigravity server uses this profile. Keep only the intended instance running before connecting; no account was selected by list order."));
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(1))
            .user_agent("OpenToken/1.0.0 (Antigravity local observer)")
            .build()
            .map_err(|_| Fault::offline())?;
        // This client is never passed a URL from settings, a response or the WebView.
        let mut last = Fault::offline();
        for endpoint in found.into_iter().take(8) {
            for scheme in ["https", "http"] {
                let base = format!(
                    "{scheme}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService",
                    endpoint.port
                );
                let request = |method: &str| -> Result<reqwest::RequestBuilder, Fault> {
                    let mut csrf = reqwest::header::HeaderValue::from_str(&endpoint.csrf)
                        .map_err(|_| invalid())?;
                    csrf.set_sensitive(true);
                    let body = if method == "RetrieveUserQuotaSummary" {
                        serde_json::json!({"forceRefresh":true})
                    } else {
                        serde_json::json!({"metadata":{"ideName":"antigravity","extensionName":"opentoken","ideVersion":"unknown","locale":"en"}})
                    };
                    Ok(http
                        .post(format!("{base}/{method}"))
                        .header("Content-Type", "application/json")
                        .header("Connect-Protocol-Version", "1")
                        .header("x-codeium-csrf-token", csrf)
                        .json(&body))
                };
                let before = match self.local_json(request("GetUserStatus")?, false).await {
                    Ok(Some(v)) => v,
                    Ok(None) => continue,
                    Err(e) => {
                        if matches!(
                            e.kind,
                            FaultKind::RateLimited
                                | FaultKind::NeedsRelogin
                                | FaultKind::PermissionDenied
                        ) {
                            return Err(e);
                        }
                        last = e;
                        continue;
                    }
                };
                let subject = identity(&before)?;
                let summary = self
                    .local_json(request("RetrieveUserQuotaSummary")?, true)
                    .await?;
                let mut report = match summary {
                    Some(v)
                        if v.get("groups").is_some() || v["response"].get("groups").is_some() =>
                    {
                        parse_summary(&v)?
                    }
                    Some(v) if v["code"] == "unimplemented" => parse_legacy(&before)?,
                    None => parse_legacy(&before)?,
                    _ => return Err(invalid()),
                };
                let after = self
                    .local_json(request("GetUserStatus")?, false)
                    .await?
                    .ok_or_else(invalid)?;
                if identity(&after)? != subject
                    || !endpoints(self.local.desktop.clone(), profile)
                        .await?
                        .iter()
                        .any(|e| e.stamp() == endpoint.stamp())
                {
                    return Err(Fault::changed());
                }
                report.plan = before["userStatus"]["userTier"]["name"]
                    .as_str()
                    .or_else(|| before["userStatus"]["planStatus"]["planInfo"]["planName"].as_str())
                    .filter(|s| text(s, 100))
                    .map(str::to_owned);
                return Ok(Reading {
                    subject: subject.clone(),
                    login: subject,
                    report,
                });
            }
        }
        Err(last)
    }
}
fn reset(v: &Value) -> Option<chrono::DateTime<Utc>> {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
}
pub(super) fn parse_summary(value: &Value) -> Result<Report, Fault> {
    let groups = value
        .get("groups")
        .or_else(|| value["response"].get("groups"))
        .and_then(Value::as_array)
        .filter(|a| a.len() <= 32)
        .ok_or_else(invalid)?;
    let mut quotas: Vec<Quota> = vec![];
    for group in groups {
        let Some(buckets) = group["buckets"].as_array().filter(|a| a.len() <= 128) else {
            continue;
        };
        for b in buckets {
            if b["disabled"].as_bool() == Some(true) {
                continue;
            }
            if !b["disabled"].is_null() && !b["disabled"].is_boolean() {
                return Err(invalid());
            }
            let id = b["bucketId"].as_str().unwrap_or("");
            let label = match id {
                "gemini-5h" => "Gemini · 5 hours",
                "gemini-weekly" => "Gemini · weekly",
                "3p-5h" => "Third-party models · 5 hours",
                "3p-weekly" => "Third-party models · weekly",
                _ => continue,
            };
            if group["displayName"] == "Gemini 4 Argon" {
                continue;
            }
            let Some(left) = crate::parser::number(&b["remainingFraction"])
                .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            else {
                continue;
            };
            let q = Quota::window(id, label, 100. * (1. - left), reset(&b["resetTime"]));
            if let Some(old) = quotas.iter().find(|q| q.id == id) {
                if old != &q {
                    return Err(invalid());
                }
            } else {
                quotas.push(q);
            }
        }
    }
    finish(quotas)
}
fn parse_legacy(value: &Value) -> Result<Report, Fault> {
    let rows = value["userStatus"]["cascadeModelConfigData"]["clientModelConfigs"]
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or_else(invalid)?;
    let mut quotas = vec![];
    let mut seen = HashSet::new();
    for row in rows {
        let Some(label) = row["label"].as_str().filter(|s| text(s, 100)) else {
            continue;
        };
        let Some(left) = crate::parser::number(&row["quotaInfo"]["remainingFraction"])
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
        else {
            continue;
        };
        let model = row["modelOrAlias"]["model"].as_str().unwrap_or(label);
        if seen.insert(model.to_owned()) {
            quotas.push(Quota::window(
                &format!("antigravity_model_{}", hash(model)),
                label,
                100. * (1. - left),
                reset(&row["quotaInfo"]["resetTime"]),
            ));
        }
    }
    finish(quotas)
}
fn finish(quotas: Vec<Quota>) -> Result<Report, Fault> {
    if quotas.is_empty() {
        return Err(denied("Antigravity did not report a recognized quota. Missing buckets were not filled from another account or interpreted as full/depleted."));
    }
    let now = Utc::now();
    Ok(Report {
        plan: None,
        quotas,
        observed_at: now,
        verified_at: Some(now),
        source: ReportSource::AntigravityLocal,
    })
}
