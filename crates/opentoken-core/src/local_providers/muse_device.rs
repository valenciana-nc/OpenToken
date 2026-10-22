//! Explicitly permissioned Meta key endpoint. This is NOT an innocuous read or MSP fallback.
use super::*;
use std::ops::Deref;
#[derive(Deserialize)]
#[serde(transparent)]
struct SecretResponse(Value);
impl Deref for SecretResponse {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.0
    }
}
impl Drop for SecretResponse {
    fn drop(&mut self) {
        fn wipe(v: &mut Value) {
            match v {
                Value::String(s) => zeroize::Zeroize::zeroize(s),
                Value::Array(a) => a.iter_mut().for_each(wipe),
                Value::Object(m) => m.values_mut().for_each(wipe),
                _ => {}
            }
        }
        wipe(&mut self.0);
    }
}
impl Service {
    pub async fn connect_muse_subscription(
        &self,
        settings: &Settings,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            bail!(
                "Approve the credential-issuing Muse endpoint explicitly. Nothing was requested."
            );
        }
        self.begin_verification(Provider::Muse)?;
        let result = tokio::time::timeout(Duration::from_secs(45), async {
            let path = profile(&self.storage, Provider::Muse);
            let read = self.read_muse_device(&path).await?;
            self.persist_local(
                settings,
                Provider::Muse,
                &path,
                read,
                ConnectionMode::MuseDevice,
            ).map_err(|_|anyhow::anyhow!("Muse monitoring could not be saved. The endpoint request may already have issued a key and cannot be undone. No returned key was stored or written to Muse."))
        })
        .await
        .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(error) = &result {
            if let Some(fault) = error.downcast_ref::<Fault>() {
                self.pause(Provider::Muse, fault);
            }
        }
        result
    }
    pub(super) async fn read_muse_device(&self, path: &Path) -> Result<Reading, Fault> {
        if !path.exists() {
            return Err(missing(Provider::Muse));
        }
        let bytes = file(path).await?;
        let token = muse::device_token(&bytes)?;
        let fingerprint = hash(&token);
        // Authentication remains CLI-owned. No OAuth registration, renewal, token export or credential write.
        let value:SecretResponse=self.json(self.http.post(format!("{}/muse-code/key",self.local.muse_base))
            .bearer_auth(token.as_str()).header("x-api-version","1.0.0").json(&serde_json::json!({})),false).await.map_err(|fault|match fault.kind{
                FaultKind::NeedsRelogin=>Fault::new(FaultKind::NeedsRelogin,"Meta rejected the CLI device login. Recover authentication in Muse, then reconnect."),
                FaultKind::PermissionDenied=>denied("Meta did not permit this subscription endpoint. No browser, payment operation or alternative host was used."),
                _=>fault,
            })?.ok_or_else(invalid)?;
        if hash(&muse::device_token(&file(path).await?)?) != fingerprint
            || marker(&file(path).await?) != marker(&bytes)
        {
            return Err(Fault::changed());
        }
        if value["require_payment"].as_bool() == Some(true) {
            return Err(denied("Meta requires billing setup. OpenToken did not add payment details or follow a payment action."));
        }
        if value["is_subs_active"].as_bool() != Some(true) {
            return Err(denied(
                "Meta did not establish an active Muse subscription for this CLI login.",
            ));
        }
        let login=value["user_email"].as_str().filter(|s|email(s)).ok_or_else(||denied("Meta did not supply the authenticated Muse subscription identity. No account was guessed."))?.to_lowercase();
        let report = parse(&value)?;
        Ok(Reading {
            subject: format!("credential:{fingerprint}:{}", hash(&login)),
            login,
            report,
        })
    }
}
fn parse(value: &Value) -> Result<Report, Fault> {
    let now = Utc::now();
    let mut quotas = vec![];
    for (key, label) in [
        ("window", "Subscription window"),
        ("weekly", "Weekly subscription"),
    ] {
        let row = &value["subs_usage"][key];
        let Some(used) = crate::parser::number(&row["used_percent"])
            .filter(|n| n.is_finite() && (0.0..=1e9).contains(n))
        else {
            continue;
        };
        let label = if key == "window" {
            let Some(minutes) = crate::parser::number(&row["window_duration_mins"])
                .filter(|n| n.is_finite() && n.fract() == 0. && (1.0..=10080.).contains(n))
            else {
                continue;
            };
            format!("{}-minute subscription window", minutes as u32)
        } else {
            label.into()
        };
        let resets_at = timestamp(&row["resets_at"]);
        if resets_at.is_some_and(|at| at <= now) {
            continue;
        }
        quotas.push(Quota {
            id: format!("muse_{key}"),
            label,
            percent: Some(used),
            value: None,
            unit: None,
            resets_at,
        });
    }
    if quotas.is_empty() {
        return Err(denied("Meta did not include subscription quota in this response (common for an idle window). No full allowance, reset or browser team quota was inferred."));
    }
    let now = Utc::now();
    Ok(Report {
        plan: value["subs_tier_name"]
            .as_str()
            .filter(|s| text(s, 100))
            .map(str::to_owned),
        quotas,
        observed_at: now,
        verified_at: Some(now),
        source: ReportSource::MuseDevice,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, routing::post, Json, Router};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn explicit_muse_key_mode_discards_returned_key_and_keeps_vendor_file_and_vault_untouched(
    ) {
        let root = tempfile::Builder::new()
            .prefix("muse-key-fixture-")
            .tempdir()
            .unwrap();
        let storage = Storage {
            dir: root.path().join("app"),
            user_home: root.path().join("home"),
        };
        let path = profile(&storage, Provider::Muse);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let auth=json!({"providers":{"meta":{"mechanism":"oauth","access_token":"dca:synthetic-device-fixture-only"}}}).to_string();
        std::fs::write(&path, &auth).unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let app=Router::new().route("/muse-code/key",post(|State(hits):State<Arc<AtomicUsize>>,headers:axum::http::HeaderMap|async move{
            assert_eq!(headers["authorization"],"Bearer dca:synthetic-device-fixture-only");assert_eq!(headers["x-api-version"],"1.0.0");hits.fetch_add(1,Ordering::SeqCst);
            Json(json!({"api_key":"LLM|synthetic-issued-key-never-store","is_subs_active":true,"user_email":"muse@fixture.test","subs_tier_name":"High","subs_usage":{"window":{"used_percent":25,"window_duration_mins":300},"weekly":{"used_percent":60}}}))
        })).with_state(hits.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut service = Service::new(storage.clone(), Arc::new(UnavailableStore));
        Arc::get_mut(&mut service.local).unwrap().muse_base = base;
        assert!(service
            .connect_muse_subscription(&Settings::default(), false)
            .await
            .is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        let (next, state) = service
            .connect_muse_subscription(&Settings::default(), true)
            .await
            .unwrap();
        assert_eq!(state.status, Status::Ready);
        assert_eq!(state.report.unwrap().source, ReportSource::MuseDevice);
        assert_eq!(next.connections[0].mode, ConnectionMode::MuseDevice);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), auth);
        let saved = serde_json::to_string(&next).unwrap();
        assert!(!saved.contains("LLM|"));
        assert!(!saved.contains("dca:"));
        assert!(!std::fs::read_to_string(storage.dir.join("state.json"))
            .unwrap_or_default()
            .contains("LLM|"));
        assert_eq!(
            service.fetch(&next.connections[0]).await.unwrap().quotas[1].percent,
            Some(60.)
        );
        assert_eq!(
            storage.load_settings().unwrap().connections[0].mode,
            ConnectionMode::MuseDevice
        );
        let after = service.disconnect(&next, &next.connections[0].id).unwrap();
        assert!(after.connections.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), auth);
        let backup: Settings =
            crate::storage::read_json(&storage.dir.join("settings.json.bak")).unwrap();
        assert!(
            backup
                .connections
                .iter()
                .all(|c| c.mode != ConnectionMode::MuseDevice),
            "automatic backup recovery cannot resurrect key-endpoint permission"
        );
        std::fs::write(
            storage.dir.join("settings.json"),
            "deliberately corrupt fixture",
        )
        .unwrap();
        assert!(storage
            .load_settings()
            .unwrap()
            .connections
            .iter()
            .all(|c| c.mode != ConnectionMode::MuseDevice));
        task.abort();
    }
    #[test]
    fn missing_idle_quota_never_becomes_a_full_subscription() {
        assert!(parse(&json!({"is_subs_active":true,"subs_tier_name":"High"})).is_err());
        let r = parse(&json!({"subs_usage":{"weekly":{"used_percent":0}}})).unwrap();
        assert_eq!(r.quotas.len(), 1);
        assert_eq!(r.quotas[0].percent, Some(0.));
        assert!(r.quotas[0].resets_at.is_none());
    }
}
