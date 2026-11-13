//! Original native implementation of opt-in Cursor desktop session monitoring.
//! Protocol references are recorded in docs/CURSOR-RESEARCH.md; no upstream code is embedded.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rusqlite::{
    limits::Limit, types::ValueRef, Connection as Database, OpenFlags, OptionalExtension,
};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use zeroize::Zeroizing;

#[cfg(test)]
#[path = "cursor_personal_tests.rs"]
mod tests;

pub(super) struct Personal {
    pub(super) path: PathBuf,
    pub(super) base: String,
    fingerprint: Mutex<Option<String>>,
    checked: Mutex<Option<tokio::time::Instant>>,
}
impl Personal {
    pub(super) fn new(storage: &Storage) -> Self {
        let roaming = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| storage.user_home.join("AppData/Roaming"));
        Self {
            path: roaming.join("Cursor/User/globalStorage/state.vscdb"),
            base: "https://cursor.com".into(),
            fingerprint: Mutex::new(None),
            checked: Mutex::new(None),
        }
    }
    async fn raw(&self) -> Result<Zeroizing<String>, Fault> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || read_token(&path))
            .await
            .map_err(|_| unreadable())?
    }
    async fn session(&self) -> Result<Session, Fault> {
        let raw = self.raw().await;
        *self.fingerprint.lock().unwrap() = Some(match &raw {
            Ok(token) => hash(token),
            Err(f) => format!("fault:{:?}", f.kind),
        });
        Session::parse(raw?)
    }
}
struct Session {
    token: Zeroizing<String>,
    subject: String,
}
impl Session {
    fn parse(token: Zeroizing<String>) -> Result<Self, Fault> {
        // Newer Cursor builds store the sign-in as an encrypted `a:b:ciphertext`
        // envelope instead of a JWT (which never contains ':'). Signing in again
        // just writes another encrypted value, so "expired, sign in again" would
        // send the user in circles. OpenToken does not decrypt other apps' secrets.
        if token.contains(':') {
            return Err(encrypted());
        }
        if !(16..=16384).contains(&token.len())
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(expired());
        }
        let parts: Vec<_> = token.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
            return Err(expired());
        }
        let bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| expired())?);
        #[derive(Deserialize)]
        struct Claims {
            sub: String,
            exp: i64,
        }
        let claims: Claims = serde_json::from_slice(&bytes).map_err(|_| expired())?;
        let Some(expires) = chrono::DateTime::from_timestamp(claims.exp, 0) else {
            return Err(expired());
        };
        if claims.exp <= Utc::now().timestamp().saturating_add(60) {
            // Say exactly what happened: the saved token is well-formed but past its
            // expiry. Cursor keeps a cached email, so it can look signed in when it isn't.
            return Err(Fault::new(
                FaultKind::NeedsRelogin,
                format!(
                    "Cursor's saved sign-in expired on {}. Cursor still shows your email, but the session is no longer valid. In Cursor, log out and log back in (profile menu → Log out). OpenToken picks up the new sign-in automatically.",
                    expires.format("%b %-d, %Y")
                ),
            ));
        }
        let subject = subject(&claims.sub).ok_or_else(expired)?;
        Ok(Self { token, subject })
    }
    fn request(
        &self,
        http: &reqwest::Client,
        base: &str,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, Fault> {
        let cookie = Zeroizing::new(format!(
            "WorkosCursorSessionToken={}%3A%3A{}",
            self.subject,
            self.token.as_str()
        ));
        let mut header = reqwest::header::HeaderValue::from_str(&cookie).map_err(|_| expired())?;
        header.set_sensitive(true);
        Ok(http
            .get(format!("{base}{path}"))
            .header(reqwest::header::COOKIE, header)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CACHE_CONTROL, "no-cache"))
    }
}
fn subject(value: &str) -> Option<String> {
    let id = value.rsplit('|').next()?;
    (!id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)))
    .then(|| id.to_owned())
}
fn expired() -> Fault {
    Fault::new(FaultKind::NeedsRelogin,"The Cursor app session is expired or invalid. Sign in again in Cursor, then reconnect. OpenToken never refreshes Cursor's tokens.")
}
fn encrypted() -> Fault {
    Fault::new(FaultKind::PermissionDenied,"This Cursor version encrypts its saved sign-in, so OpenToken can no longer read personal Cursor usage from the app. Your Cursor login is fine; signing in again won't change this. Cursor Enterprise members can still connect with an admin API key.")
}
fn unreadable() -> Fault {
    Fault::new(FaultKind::Unreadable,"Cursor's local sign-in could not be read safely. Open Cursor and try again. Its database was not copied or modified.")
}
fn no_session() -> Fault {
    Fault::new(FaultKind::NotSignedIn,"No Cursor desktop sign-in was found. Open Cursor, sign in normally, then connect its existing sign-in.")
}
fn reparse(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    s.into()
}
fn sidecars(path: &Path) -> Result<(bool, bool), Fault> {
    let mut exists = [false; 2];
    for (i, suffix) in ["-wal", "-shm"].iter().enumerate() {
        match std::fs::symlink_metadata(sidecar(path, suffix)) {
            Ok(m) if m.is_file() && !reparse(&m) => exists[i] = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(unreadable()),
        }
    }
    Ok((exists[0], exists[1]))
}
fn journal_clear(path: &Path) -> Result<bool, Fault> {
    match std::fs::symlink_metadata(sidecar(path, "-journal")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(unreadable()),
    }
}
fn read_token(path: &Path) -> Result<Zeroizing<String>, Fault> {
    if !path.is_absolute() {
        return Err(unreadable());
    }
    for parent in path.ancestors().skip(1).take(3) {
        if let Ok(meta) = std::fs::symlink_metadata(parent) {
            if reparse(&meta) || !meta.is_dir() {
                return Err(unreadable());
            }
        }
    }
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(no_session()),
        Ok(meta) if meta.is_file() && !reparse(&meta) && meta.len() <= 16 * 1024 * 1024 * 1024 => {}
        _ => return Err(unreadable()),
    }
    let before = sidecars(path)?;
    if !journal_clear(path)? || before.0 != before.1 {
        return Err(unreadable());
    } // Never create a missing SQLite sidecar or ignore a live WAL.
    let db = if before.0 {
        Database::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
    } else {
        let mut uri = reqwest::Url::from_file_path(path).map_err(|_| unreadable())?;
        uri.set_query(Some("immutable=1"));
        Database::open_with_flags(
            uri.as_str(),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )
    }
    .map_err(|_| unreadable())?;
    db.busy_timeout(Duration::from_millis(150))
        .map_err(|_| unreadable())?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, 32768)
        .map_err(|_| unreadable())?;
    let start = std::time::Instant::now();
    db.progress_handler(
        1000,
        Some(move || start.elapsed() > Duration::from_millis(300)),
    )
    .map_err(|_| unreadable())?;
    let bytes: Option<Zeroizing<Vec<u8>>> = db
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1",
            ["cursorAuth/accessToken"],
            |row| {
                let value = row.get_ref(0)?;
                let bytes = match value {
                    ValueRef::Text(v) | ValueRef::Blob(v) if v.len() <= 16384 => v,
                    ValueRef::Null => &[],
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(Zeroizing::new(bytes.to_vec()))
            },
        )
        .optional()
        .map_err(|_| unreadable())?;
    if !journal_clear(path)? || (!before.0 && sidecars(path)? != (false, false)) {
        return Err(unreadable());
    }
    let bytes = bytes.ok_or_else(no_session)?;
    if bytes.is_empty() {
        return Err(no_session());
    }
    let data = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(&bytes);
    let (pairs, remainder) = data.as_chunks::<2>();
    let token = if remainder.is_empty()
        && pairs
            .iter()
            .all(|v| v[0].is_ascii() && v[0] != 0 && v[1] == 0)
    {
        Zeroizing::new(pairs.iter().map(|p| p[0] as char).collect::<String>())
    } else {
        Zeroizing::new(
            std::str::from_utf8(&bytes)
                .map_err(|_| expired())?
                .to_owned(),
        )
    };
    if token.is_empty() {
        return Err(no_session());
    }
    Ok(token)
}
#[derive(Deserialize)]
struct User {
    sub: Option<String>,
    id: Option<serde_json::Value>,
    email: Option<String>,
}
impl User {
    fn identity(&self) -> Result<String, Fault> {
        let a = self.sub.as_deref().and_then(subject);
        // `sub` is the authenticated OIDC subject; an extra numeric database ID is not.
        let b = self
            .id
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .and_then(subject);
        if (self.sub.is_some() && a.is_none())
            || (a.is_none() && self.id.is_some() && b.is_none())
            || a.as_ref().zip(b.as_ref()).is_some_and(|(a, b)| a != b)
            || self.email.as_deref().is_some_and(|e| !email(e))
        {
            return Err(invalid());
        }
        a.or(b).ok_or_else(invalid)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    membership_type: Option<String>,
    billing_cycle_start: Option<String>,
    billing_cycle_end: Option<String>,
    individual_usage: Option<Individual>,
}
#[derive(Deserialize)]
struct Individual {
    plan: Option<Plan>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    total_percent_used: Option<f64>,
    auto_percent_used: Option<f64>,
    api_percent_used: Option<f64>,
}
fn report(summary: Summary) -> Result<Report, Fault> {
    if summary.membership_type.is_none()
        && summary.individual_usage.is_none()
        && summary.billing_cycle_start.is_none()
        && summary.billing_cycle_end.is_none()
    {
        return Err(invalid());
    }
    fn date(s: Option<String>) -> Result<Option<chrono::DateTime<Utc>>, Fault> {
        s.map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|_| invalid())
        })
        .transpose()
    }
    let start = date(summary.billing_cycle_start)?;
    let end = date(summary.billing_cycle_end)?;
    if start.zip(end).is_some_and(|(a, b)| a >= b) {
        return Err(invalid());
    }
    let mut quotas = vec![];
    if let Some(plan) = summary.individual_usage.and_then(|u| u.plan) {
        for (id, label, value) in [
            ("cursor_monthly", "Monthly plan", plan.total_percent_used),
            ("cursor_auto", "Auto", plan.auto_percent_used),
            ("cursor_api", "API", plan.api_percent_used),
        ] {
            if let Some(value) = value {
                nonnegative(value)?;
                quotas.push(Quota::window(id, label, value, end));
            }
        }
    }
    let plan = summary
        .membership_type
        .filter(|p| text(p, 64))
        .map(|p| match p.as_str() {
            "pro" => "Pro".into(),
            "pro_plus" => "Pro Plus".into(),
            "ultra" => "Ultra".into(),
            "free" | "hobby" => "Hobby".into(),
            _ => p,
        });
    Ok(Report {
        plan,
        quotas,
        observed_at: Utc::now(),
        source: ReportSource::CursorDashboard,
        verified_at: None,
    })
}
impl Service {
    pub fn cursor_watch_path(&self, settings: &Settings) -> Option<PathBuf> {
        settings
            .connections
            .iter()
            .any(|c| c.mode == ConnectionMode::CursorApp)
            .then(|| self.personal.path.clone())
    }
    pub async fn cursor_auth_changed(&self, settings: &Settings) -> bool {
        if self.cursor_watch_path(settings).is_none() {
            return false;
        }
        {
            let mut checked = self.personal.checked.lock().unwrap();
            let now = tokio::time::Instant::now();
            if checked
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
            {
                return false;
            }
            *checked = Some(now);
        }
        let raw = self.personal.raw().await;
        let marker = match raw {
            Ok(token) => hash(&token),
            Err(f) if f.kind == FaultKind::Unreadable => return false, // A transient SQLite lock is not proof of a different login.
            Err(f) => format!("fault:{:?}", f.kind),
        };
        let mut prior = self.personal.fingerprint.lock().unwrap();
        let changed = prior.as_ref() != Some(&marker);
        *prior = Some(marker);
        changed
    }
    pub async fn cursor_sign_in_marker(&self, consent: bool) -> Result<String, Fault> {
        if !consent {
            return Err(Fault::new(
                FaultKind::PermissionDenied,
                "Sign-in access was not requested.",
            ));
        }
        let session = Session::parse(self.personal.raw().await?)?;
        Ok(hash(&session.token))
    }
    pub async fn connect_cursor_personal(
        &self,
        settings: &Settings,
        consent: bool,
    ) -> Result<(Settings, AccountState)> {
        if !consent {
            bail!("Allow read-only access to Cursor's existing sign-in and its unofficial dashboard before connecting.");
        }
        self.begin_verification(Provider::Cursor)?;
        let result = tokio::time::timeout(Duration::from_secs(45), self.cursor_app_read(None))
            .await
            .unwrap_or_else(|_| Err(Fault::offline()));
        if let Err(f) = &result {
            self.pause(Provider::Cursor, f);
        }
        let (connection, report) = result?;
        let mut next = settings.clone();
        next.connections
            .retain(|c| c.mode != ConnectionMode::CursorApp || c.scope != connection.scope);
        next.connections.push(connection.clone());
        next.validate()?;
        self.storage.save_settings(&next).map_err(|_| {
            anyhow::anyhow!(
                "The Cursor connection could not be saved. Cursor's sign-in was not changed."
            )
        })?;
        Ok((
            next.clone(),
            AccountState::apply(connection.account(&next), Ok(report), None),
        ))
    }
    pub(super) async fn fetch_cursor_app(&self, c: &Connection) -> Result<Report, Fault> {
        self.cursor_app_read(Some(c)).await.map(|(_, r)| r)
    }
    async fn cursor_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        stage: &'static str,
    ) -> Result<T, Fault> {
        let value: serde_json::Value = self.json(request,false).await.map_err(|mut f| {
            if f.kind == FaultKind::NeedsRelogin { f.message="Cursor rejected its saved access token. Open Cursor so it can recover its own session, then try again. No Enterprise key is needed.".into(); }
            else if f.kind == FaultKind::PermissionDenied { f.message="Cursor's private dashboard refused this read. This does not by itself prove your app login is invalid; no Enterprise key is needed for this mode.".into(); }
            f.message=format!("Cursor {stage}: {}",f.message); f
        })?.ok_or_else(invalid)?;
        serde_json::from_value(value).map_err(|_|Fault::new(FaultKind::Unreadable,format!("Cursor {stage} returned an unsupported response format. No account or quota was inferred.")))
    }
    pub(super) async fn cursor_app_read(
        &self,
        expected: Option<&Connection>,
    ) -> Result<(Connection, Report), Fault> {
        let profile = self
            .personal
            .path
            .to_str()
            .filter(|p| text(p, 2048))
            .ok_or_else(unreadable)?
            .to_owned();
        let scope = format!(
            "app:{}",
            hash(&crate::storage::path_key(&self.personal.path))
        );
        if expected.is_some_and(|c| {
            c.provider != Provider::Cursor
                || c.mode != ConnectionMode::CursorApp
                || !c.profile.as_deref().is_some_and(|p| {
                    crate::storage::path_key(Path::new(p))
                        == crate::storage::path_key(&self.personal.path)
                })
                || c.scope != scope
        }) {
            return Err(Fault::changed());
        }
        let session = self.personal.session().await?;
        if expected.is_some_and(|c| c.subject != session.subject) {
            return Err(Fault::changed());
        }
        let before: User = self
            .cursor_json(
                session.request(&self.http, &self.personal.base, "/api/auth/me")?,
                "account identity",
            )
            .await?;
        let login = before
            .email
            .as_deref()
            .unwrap_or(&session.subject)
            .to_owned();
        if before.identity()? != session.subject
            || expected.is_some_and(|c| !c.login.eq_ignore_ascii_case(&login))
        {
            return Err(Fault::changed());
        }
        let summary: Summary = self
            .cursor_json(
                session.request(&self.http, &self.personal.base, "/api/usage-summary")?,
                "usage summary",
            )
            .await?;
        let after: User = self
            .cursor_json(
                session.request(&self.http, &self.personal.base, "/api/auth/me")?,
                "account recheck",
            )
            .await?;
        if after.identity()? != session.subject
            || !after
                .email
                .as_deref()
                .unwrap_or(&session.subject)
                .eq_ignore_ascii_case(&login)
        {
            return Err(Fault::changed());
        }
        let current = self.personal.session().await?;
        if current.subject != session.subject || current.token.as_str() != session.token.as_str() {
            return Err(Fault::changed());
        }
        let mut connection = Connection::new(Provider::Cursor, session.subject, login, scope);
        connection.mode = ConnectionMode::CursorApp;
        connection.profile = Some(profile);
        Ok((
            connection,
            report(summary).map_err(|mut f| {
                f.message = format!("Cursor allowance / billing period: {}", f.message);
                f
            })?,
        ))
    }
}
