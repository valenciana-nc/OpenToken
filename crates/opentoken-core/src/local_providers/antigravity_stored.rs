//! Original parser for the installed Antigravity IDE's persisted USS OAuth topic.
//! Schema facts corroborated against first-party public resources; no vendor runtime copied.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
struct Field<'a> {
    number: u64,
    bytes: Option<&'a [u8]>,
    integer: Option<u64>,
}
fn fields(bytes: &[u8]) -> Result<Vec<Field<'_>>, Fault> {
    if bytes.len() > 256 * 1024 {
        return Err(invalid());
    }
    fn varint(bytes: &[u8], i: &mut usize) -> Result<u64, Fault> {
        let mut out = 0u64;
        for n in 0..10 {
            let b = *bytes.get(*i).ok_or_else(invalid)?;
            *i += 1;
            if n == 9 && b > 1 {
                return Err(invalid());
            }
            out |= ((b & 127) as u64) << (n * 7);
            if b & 128 == 0 {
                return Ok(out);
            }
        }
        Err(invalid())
    }
    let mut i = 0;
    let mut out = vec![];
    while i < bytes.len() {
        if out.len() >= 512 {
            return Err(invalid());
        }
        let tag = varint(bytes, &mut i)?;
        let (number, wire) = (tag >> 3, tag & 7);
        if number == 0 {
            return Err(invalid());
        }
        let mut field = Field {
            number,
            bytes: None,
            integer: None,
        };
        match wire {
            0 => field.integer = Some(varint(bytes, &mut i)?),
            1 | 5 => {
                i = i
                    .checked_add(if wire == 1 { 8 } else { 4 })
                    .filter(|n| *n <= bytes.len())
                    .ok_or_else(invalid)?;
            }
            2 => {
                let len = usize::try_from(varint(bytes, &mut i)?).map_err(|_| invalid())?;
                let end = i
                    .checked_add(len)
                    .filter(|n| *n <= bytes.len())
                    .ok_or_else(invalid)?;
                field.bytes = Some(&bytes[i..end]);
                i = end;
            }
            _ => return Err(invalid()),
        }
        out.push(field)
    }
    Ok(out)
}
fn unique_bytes<'a>(values: &[Field<'a>], number: u64) -> Result<Option<&'a [u8]>, Fault> {
    let mut selected = None;
    for f in values.iter().filter(|f| f.number == number) {
        if selected.is_some() || f.bytes.is_none() {
            return Err(invalid());
        }
        selected = f.bytes;
    }
    Ok(selected)
}
fn decode(text: &str) -> Result<Zeroizing<Vec<u8>>, Fault> {
    if text.len() > 256 * 1024 {
        return Err(invalid());
    }
    STANDARD
        .decode(text)
        .map(Zeroizing::new)
        .map_err(|_| invalid())
}
pub(super) struct Stored {
    pub token: Zeroizing<String>,
    pub stamp: String,
}
fn parse(text: &str) -> Result<Stored, Fault> {
    let topic = decode(text)?;
    let mut payload = None;
    for f in fields(&topic)?.iter().filter(|f| f.number == 1) {
        let entry = fields(f.bytes.ok_or_else(invalid)?)?;
        if unique_bytes(&entry, 1)? != Some(b"oauthTokenInfoSentinelKey".as_slice()) {
            continue;
        }
        if payload.is_some() {
            return Err(invalid());
        }
        let row = fields(unique_bytes(&entry, 2)?.ok_or_else(invalid)?)?;
        let encoded = std::str::from_utf8(unique_bytes(&row, 1)?.ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        payload = Some(decode(encoded)?);
    }
    let payload=payload.ok_or_else(||Fault::new(FaultKind::NotSignedIn,"Antigravity IDE has no persisted OAuth observation in its exact auth record. No CLI login or browser import was started."))?;
    let info = fields(&payload)?;
    // OAuthTokenInfo: access_token=1, token_type=2, refresh_token=3, expiry=4, is_gcp_tos=6.
    if info
        .iter()
        .any(|f| f.number == 6 && f.integer.is_none_or(|n| n != 0))
    {
        return Err(denied("This Antigravity IDE session uses GCP/enterprise onboarding. No personal daily quota or project was inferred."));
    }
    let token =
        std::str::from_utf8(unique_bytes(&info, 1)?.ok_or_else(invalid)?).map_err(|_| invalid())?;
    if !(16..=16384).contains(&token.len()) || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid());
    }
    if let Some(kind) = unique_bytes(&info, 2)? {
        if !kind.is_empty() && !kind.eq_ignore_ascii_case(b"bearer") {
            return Err(invalid());
        }
    }
    // Expiry metadata is not an authentication result. The fixed Google userinfo
    // request must establish acceptance; no refresh field is read or used.
    Ok(Stored {
        token: Zeroizing::new(token.into()),
        stamp: marker(text.as_bytes()),
    })
}
pub(super) fn profile(storage: &Storage) -> PathBuf {
    roaming(storage).join("Antigravity IDE/User/globalStorage/state.vscdb")
}
pub(super) async fn read(storage: &Storage, path: &Path) -> Result<Stored, Fault> {
    if crate::storage::path_key(path) != crate::storage::path_key(&profile(storage)) {
        return Err(Fault::changed());
    }
    let p = path.to_owned();
    tokio::task::spawn_blocking(move || {
        verify_config(&p)?;
        let row = crate::source_db::read(&p, "antigravityUnifiedStateSync.oauthToken", 256 * 1024)?;
        parse(&row)
    })
    .await
    .map_err(|_| Fault::offline())?
}
fn verify_config(path: &Path) -> Result<(), Fault> {
    let settings = path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(invalid)?
        .join("settings.json");
    match std::fs::symlink_metadata(&settings) {
        Ok(_) => {
            let raw = raw_file(&settings)?;
            let clean = super::devin_app::strip_jsonc(&raw)?;
            let value:Value=serde_json::from_slice(&clean).map_err(|_|Fault::new(FaultKind::Unreadable,"Antigravity IDE's deployment configuration could not be verified. No bearer was sent."))?;
            if !value.is_object() {
                return Err(invalid());
            }
            for url in [
                value.get("jetski.cloudCodeUrl"),
                value.get("jetski").and_then(|v| v.get("cloudCodeUrl")),
            ]
            .into_iter()
            .flatten()
            .filter(|v| !v.is_null())
            {
                if !url.as_str().is_some_and(|s| {
                    s.is_empty()
                        || s.trim_end_matches('/') == "https://daily-cloudcode-pa.googleapis.com"
                }) {
                    return Err(denied("Antigravity IDE selects a different Cloud Code deployment. OpenToken will not substitute daily quota or forward its bearer to a custom host."));
                }
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(invalid()),
    }
}
fn daily_context(value: &Value) -> Result<(), Fault> {
    let tier=value.get("paidTier").filter(|v|v.is_object()).or_else(||value.get("currentTier").filter(|v|v.is_object())).ok_or_else(||denied("Antigravity IDE's active quota tier was not established. No onboarding or alternate quota pool was requested."))?;
    if !tier["id"]
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 128)
    {
        return Err(invalid());
    }
    if tier
        .get("usesGcpTos")
        .is_some_and(|v| v.as_bool() != Some(false))
        || value["currentTier"]["usesGcpTos"] == true
    {
        return Err(denied("Antigravity IDE selects the production/GCP quota context. It is not interchangeable with daily Antigravity quota; no quota pool was substituted."));
    }
    Ok(())
}
impl Service {
    pub(super) async fn read_antigravity_stored(&self, path: &Path) -> Result<Reading, Fault> {
        let session = read(&self.storage, path).await?;
        let before = self.antigravity_identity(&session.token).await.map_err(|mut f|{if f.kind==FaultKind::NeedsRelogin{f.message="Google rejected Antigravity IDE's saved session. Choose Recover with Antigravity IDE so the official app can restore its own login; OpenToken will verify that client's quota afterward.".into();}f})?;
        // The IDE's first-party selector loads the active tier before choosing DAILY.
        // This is context lookup only: never call onboardUser or production quota.
        let context = self
            .local_json(
                self.http
                    .post(&self.local.google_context_url)
                    .bearer_auth(session.token.as_str())
                    .json(&serde_json::json!({})),
                false,
            )
            .await?
            .ok_or_else(invalid)?;
        daily_context(&context)?;
        let report = self.antigravity_cloud_report(&session.token).await?;
        let after = self.antigravity_identity(&session.token).await?;
        if before != after || read(&self.storage, path).await?.stamp != session.stamp {
            return Err(Fault::changed());
        }
        self.local
            .stored_marks
            .lock()
            .unwrap()
            .insert(crate::storage::path_key(path), session.stamp);
        let mut report = report;
        report.source = ReportSource::AntigravityStored;
        Ok(Reading {
            subject: before.0,
            login: before.1,
            report,
        })
    }
}
#[cfg(test)]
pub(super) fn fixture_topic(token: &str, enterprise: bool) -> String {
    tests::fixture(token, enterprise)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn field(number: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![(number << 3) | 2];
        let mut len = data.len();
        while len > 127 {
            out.push((len as u8 & 127) | 128);
            len >>= 7;
        }
        out.push(len as u8);
        out.extend_from_slice(data);
        out
    }
    pub(super) fn fixture(token: &str, enterprise: bool) -> String {
        let mut info = field(1, token.as_bytes());
        info.extend(field(2, b"Bearer"));
        info.extend(field(3, b"fixture-refresh-never-used"));
        if enterprise {
            info.extend([48, 1]);
        }
        let row = field(1, STANDARD.encode(info).as_bytes());
        let mut entry = field(1, b"oauthTokenInfoSentinelKey");
        entry.extend(field(2, &row));
        STANDARD.encode(field(1, &entry))
    }
    #[test]
    fn daily_context_rejects_unknown_onboarding_and_production_pools() {
        assert!(daily_context(&serde_json::json!({})).is_err());
        assert!(daily_context(&serde_json::json!({"allowedTiers":[{"id":"free-tier"}]})).is_err());
        assert!(daily_context(
            &serde_json::json!({"paidTier":{"id":"enterprise","usesGcpTos":true}})
        )
        .is_err());
        assert!(daily_context(
            &serde_json::json!({"paidTier":{"id":"pro","usesGcpTos":"unknown"}})
        )
        .is_err());
        assert!(daily_context(
            &serde_json::json!({"paidTier":{"id":"g1-pro-tier","usesGcpTos":false}})
        )
        .is_ok());
    }
    #[test]
    fn non_daily_configuration_is_rejected_before_any_bearer_forwarding() {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("User/globalStorage/state.vscdb");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let settings = root.path().join("User/settings.json");
        std::fs::write(&settings,"{// public default\n\"jetski.cloudCodeUrl\":\"https://daily-cloudcode-pa.googleapis.com\",}").unwrap();
        assert!(verify_config(&db).is_ok());
        for text in [
            r#"{"jetski.cloudCodeUrl":"https://cloudcode-pa.googleapis.com"}"#,
            r#"{"jetski":{"cloudCodeUrl":"https://custom.invalid"}}"#,
        ] {
            std::fs::write(&settings, text).unwrap();
            assert!(verify_config(&db).is_err());
        }
    }
    #[test]
    fn exact_proto_record_is_bounded_and_never_uses_refresh_or_enterprise_scope() {
        let f = fixture("fixture-only-google-access-token", false);
        assert_eq!(
            parse(&f).unwrap().token.as_str(),
            "fixture-only-google-access-token"
        );
        assert!(parse(&fixture("fixture-only-google-access-token", true)).is_err());
        assert!(parse(&STANDARD.encode([10, 255, 255])).is_err());
        assert!(parse(&"x".repeat(256 * 1024 + 1)).is_err());
        assert!(parse("not base64").is_err());
    }
}
