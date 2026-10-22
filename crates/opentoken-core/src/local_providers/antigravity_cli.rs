//! Read-only, explicitly authorized agy session reuse. No OAuth registration, renewal or writes.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
pub(super) trait SessionStore: Send + Sync {
    fn read(&self) -> Result<Zeroizing<Vec<u8>>, Fault>;
}
pub(super) struct NativeStore {
    pub allowed: bool,
}
impl SessionStore for NativeStore {
    fn read(&self) -> Result<Zeroizing<Vec<u8>>, Fault> {
        if !self.allowed {
            return Err(denied(
                "An isolated source home cannot read the real user's provider credential store.",
            ));
        }
        #[cfg(windows)]
        {
            super::windows::antigravity_credential()
        }
        #[cfg(not(windows))]
        {
            Err(Fault::new(
                FaultKind::ToolMissing,
                "Antigravity CLI credential reuse is supported on native Windows only.",
            ))
        }
    }
}
pub(super) fn profile(storage: &Storage) -> Option<PathBuf> {
    let mut roots = vec![storage.user_home.join("AppData/Local/agy/bin/agy.exe")];
    if storage.user_home == Storage::system().user_home {
        if let Some(p) = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            roots.push(p.join("agy/bin/agy.exe"));
        }
    }
    roots.into_iter().find(|p| crate::monitoring::safe_file(p))
}
struct Session {
    token: Zeroizing<String>,
    stamp: String,
}
fn parse(bytes: &[u8]) -> Result<Session, Fault> {
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    let data = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes);
    let (pairs, rest) = data.as_chunks::<2>();
    let text = if rest.is_empty()
        && (bytes.starts_with(&[0xff, 0xfe]) || pairs.iter().all(|p| p[0].is_ascii() && p[1] == 0))
    {
        let words = Zeroizing::new(
            pairs
                .iter()
                .map(|p| u16::from_le_bytes(*p))
                .collect::<Vec<_>>(),
        );
        Zeroizing::new(String::from_utf16(&words).map_err(|_| invalid())?)
    } else {
        Zeroizing::new(
            std::str::from_utf8(bytes)
                .map_err(|_| invalid())?
                .to_owned(),
        )
    };
    let payload = text.trim_end_matches('\0').trim();
    let decoded = if let Some(s) = payload.strip_prefix("go-keyring-base64:") {
        Zeroizing::new(STANDARD.decode(s.trim()).map_err(|_| invalid())?)
    } else {
        Zeroizing::new(payload.as_bytes().to_vec())
    };
    #[derive(Deserialize)]
    struct Envelope {
        token: Token,
    }
    #[derive(Deserialize)]
    struct Token {
        access_token: String,
    }
    impl Drop for Token {
        fn drop(&mut self) {
            zeroize::Zeroize::zeroize(&mut self.access_token)
        }
    }
    let envelope:Envelope=serde_json::from_slice(&decoded).map_err(|_|denied("Antigravity CLI's saved credential format is unsupported. No refresh token, other vault entry or browser session was used."))?;
    let token = &envelope.token;
    // Local expiry may be stale or omitted (Go's zero-time also means unknown).
    // Only the fixed provider's authenticated response establishes current access.
    if !(16..=16384).contains(&token.access_token.len())
        || !token.access_token.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(invalid());
    }
    Ok(Session {
        token: Zeroizing::new(token.access_token.clone()),
        stamp: marker(bytes),
    })
}
impl Service {
    /// Consent-first native maintenance only. Does not enumerate the vault, export a blob or make HTTP requests.
    pub fn inspect_antigravity_cli_namespace(&self, consent: bool) -> Result<bool, Fault> {
        if !consent || self.storage.user_home != Storage::system().user_home {
            return Err(denied(
                "Explicit real-source inspection is required. No credential was read.",
            ));
        }
        #[cfg(windows)]
        {
            super::windows::antigravity_cli_namespace_present()
        }
        #[cfg(not(windows))]
        {
            Err(Fault::new(
                FaultKind::ToolMissing,
                "This namespace diagnostic is Windows-only.",
            ))
        }
    }
    pub async fn antigravity_recovery_marker(
        &self,
        client: AntigravityClient,
        consent: bool,
    ) -> Result<String, Fault> {
        if !consent {
            return Err(denied("Start official Antigravity recovery explicitly."));
        }
        if client == AntigravityClient::Cli {
            return self.antigravity_session().await.map(|s| s.stamp);
        }
        let path = roaming(&self.storage).join(if client == AntigravityClient::Ide {
            "Antigravity IDE"
        } else {
            "Antigravity"
        });
        let endpoints = super::antigravity::endpoints(self.local.desktop.clone(), &path).await?;
        if endpoints.is_empty() {
            return Err(Fault::new(
                FaultKind::NotSignedIn,
                "Waiting for the selected official Antigravity client.",
            ));
        }
        if endpoints
            .iter()
            .map(|e| e.pid)
            .collect::<HashSet<_>>()
            .len()
            != 1
        {
            return Err(denied("Multiple servers use the selected Antigravity profile. No instance was selected by list order."));
        }
        let auth = if client == AntigravityClient::Ide {
            super::antigravity_stored::read(
                &self.storage,
                &super::antigravity_stored::profile(&self.storage),
            )
            .await
            .map(|s| s.stamp)
        } else {
            self.antigravity_session().await.map(|s| s.stamp)
        };
        Ok(hash(&format!(
            "{}:{}",
            endpoints[0].stamp(),
            auth.unwrap_or_default()
        )))
    }
    pub async fn connect_antigravity_client(
        &self,
        settings: &Settings,
        client: AntigravityClient,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            return Err(denied("Start official Antigravity recovery explicitly.").into());
        }
        self.begin_verification(Provider::Antigravity)?;
        let result = tokio::time::timeout(Duration::from_secs(45), async {
            let (path, reading, mode) = if client == AntigravityClient::Cli {
                let p = profile(&self.storage).ok_or_else(|| {
                    Fault::new(
                        FaultKind::ToolMissing,
                        "The installed agy CLI was not found. Nothing was installed.",
                    )
                })?;
                let r = self.read_antigravity_cli(&p).await?;
                (p, r, ConnectionMode::AntigravityCli)
            } else {
                let p = roaming(&self.storage).join(if client == AntigravityClient::Ide {
                    "Antigravity IDE"
                } else {
                    "Antigravity"
                });
                let r = self.read_antigravity(&p).await?;
                (p, r, ConnectionMode::AntigravityApp)
            };
            self.persist_local(settings, Provider::Antigravity, &path, reading, mode)
        })
        .await
        .unwrap_or_else(|_| Err(Fault::offline().into()));
        if let Err(e) = &result {
            if let Some(f) = e.downcast_ref::<Fault>() {
                self.pause(Provider::Antigravity, f);
            }
        }
        result
    }
    async fn antigravity_session(&self) -> Result<Session, Fault> {
        let store = self.local.credential.clone();
        let bytes = tokio::task::spawn_blocking(move || store.read())
            .await
            .map_err(|_| Fault::offline())??;
        parse(&bytes)
    }
    pub(super) async fn connect_antigravity(
        &self,
        settings: &Settings,
    ) -> Result<(Settings, AccountState)> {
        let (path, reading, mode) = self.antigravity_existing().await?;
        self.persist_local(settings, Provider::Antigravity, &path, reading, mode)
    }
    pub(super) async fn antigravity_existing(
        &self,
    ) -> Result<(PathBuf, Reading, ConnectionMode), Fault> {
        // Consider both native profiles. An old Roaming folder must not shadow the running IDE.
        let mut active = vec![];
        for p in [
            roaming(&self.storage).join("Antigravity"),
            roaming(&self.storage).join("Antigravity IDE"),
        ] {
            if !super::antigravity::endpoints(self.local.desktop.clone(), &p)
                .await?
                .is_empty()
            {
                active.push(p)
            }
        }
        if active.len() > 1 {
            return Err(denied("Both Antigravity desktop profiles are active. Keep the intended app instance running before connecting; no account was selected by folder order."));
        }
        if let Some(p) = active.first() {
            let reading = self.read_antigravity(p).await?;
            return Ok((p.clone(), reading, ConnectionMode::AntigravityApp));
        }
        let cached = super::antigravity_stored::profile(&self.storage);
        match super::antigravity_stored::read(&self.storage, &cached).await {
            Ok(_) => {
                let reading = self.read_antigravity_stored(&cached).await?;
                return Ok((cached, reading, ConnectionMode::AntigravityStored));
            }
            Err(f) if f.kind == FaultKind::NotSignedIn => {}
            Err(f) => return Err(f),
        }
        let p=profile(&self.storage).ok_or_else(||Fault::new(FaultKind::Offline,"No supported Antigravity service is running. Open your signed-in Antigravity app/IDE, then try again. Finding an installed app is not a running quota source; no login was started."))?;
        let reading = self.read_antigravity_cli(&p).await?;
        Ok((p, reading, ConnectionMode::AntigravityCli))
    }
    pub(super) async fn antigravity_identity(
        &self,
        token: &str,
    ) -> Result<(String, String), Fault> {
        let value: Value = self
            .local_json(
                self.http
                    .get(&self.local.google_identity_url)
                    .bearer_auth(token),
                false,
            )
            .await?
            .ok_or_else(invalid)?;
        let sub=value["sub"].as_str().filter(|s|text(s,256)).ok_or_else(||denied("Google did not establish the existing Antigravity CLI account identity. No quota was assigned."))?;
        let login = value["email"].as_str().filter(|s| email(s)).unwrap_or(sub);
        Ok((sub.into(), login.into()))
    }
    pub(super) async fn antigravity_cloud_report(&self, token: &str) -> Result<Report, Fault> {
        let value = self
            .local_json(
                self.http
                    .post(&self.local.google_quota_url)
                    .bearer_auth(token)
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({})),
                false,
            )
            .await?
            .ok_or_else(invalid)?;
        super::antigravity::parse_summary(&value)
    }
    pub(super) async fn read_antigravity_cli(&self, path: &Path) -> Result<Reading, Fault> {
        if !profile(&self.storage)
            .is_some_and(|p| crate::storage::path_key(&p) == crate::storage::path_key(path))
        {
            return Err(Fault::changed());
        }
        let session = self.antigravity_session().await?;
        let before = self.antigravity_identity(&session.token).await?;
        // Installed first-party 2.23.1 explicitly selects DAILY. Production Gemini is a
        // different pool: never merge it merely because the bearer belongs to the same user.
        let mut report = self.antigravity_cloud_report(&session.token).await?;
        let after = self.antigravity_identity(&session.token).await?;
        if before != after || self.antigravity_session().await?.stamp != session.stamp {
            return Err(Fault::changed());
        }
        report.source = ReportSource::AntigravityCli;
        Ok(Reading {
            subject: before.0,
            login: before.1,
            report,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vendor_credential_encodings_expiry_and_bounds() {
        let value=serde_json::json!({"token":{"access_token":"fixture-only-valid-access-token","expiry":(Utc::now()+chrono::Duration::hours(1)).to_rfc3339(),"refresh_token":"fixture-refresh-must-not-be-used"}}).to_string();
        assert_eq!(
            parse(value.as_bytes()).unwrap().token.as_str(),
            "fixture-only-valid-access-token"
        );
        let wrapped = format!("go-keyring-base64:{}", STANDARD.encode(value.as_bytes()));
        assert!(parse(wrapped.as_bytes()).is_ok());
        let utf16 = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(parse(&utf16).is_ok());
        assert!(parse(br#"{"token":{"access_token":"fixture-only-valid-access-token","expiry":"2020-01-01T00:00:00Z"}}"#).is_ok());
        assert!(parse(&vec![b'x'; 65537]).is_err());
    }
}
