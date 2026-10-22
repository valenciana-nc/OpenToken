//! Bounded, read-only exact-key reads of provider-owned VS Code state databases.
//! Never copy/checkpoint a database, create sidecars, or enumerate credential rows.
use crate::model::{Fault, FaultKind};
use rusqlite::{limits::Limit, types::ValueRef, Connection, OpenFlags, OptionalExtension};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
fn unreadable() -> Fault {
    Fault::new(FaultKind::Unreadable,"The provider's state database could not be read safely. No database was copied or modified.")
}
fn missing() -> Fault {
    Fault::new(
        FaultKind::NotSignedIn,
        "The selected provider state has no saved sign-in.",
    )
}
fn linked(m: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        m.file_type().is_symlink()
    }
}
pub(crate) fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(suffix);
    p.into()
}
fn sidecars(path: &Path) -> Result<(bool, bool), Fault> {
    let mut found = [false; 2];
    for (i, suffix) in ["-wal", "-shm"].iter().enumerate() {
        match std::fs::symlink_metadata(sidecar(path, suffix)) {
            Ok(m) if m.is_file() && !linked(&m) => found[i] = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(unreadable()),
        }
    }
    Ok((found[0], found[1]))
}
fn journal_clear(path: &Path) -> bool {
    matches!(std::fs::symlink_metadata(sidecar(path,"-journal")),Err(e)if e.kind()==std::io::ErrorKind::NotFound)
}
pub(crate) fn read(path: &Path, key: &str, max: usize) -> Result<Zeroizing<String>, Fault> {
    if !path.is_absolute() || max > 256 * 1024 {
        return Err(unreadable());
    }
    for p in path.ancestors().skip(1) {
        if let Ok(m) = std::fs::symlink_metadata(p) {
            if linked(&m) || !m.is_dir() {
                return Err(unreadable());
            }
        }
    }
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
        Ok(m) if m.is_file() && !linked(&m) && m.len() <= 16 * 1024 * 1024 * 1024 => {}
        _ => return Err(unreadable()),
    }
    let before = sidecars(path)?;
    if !journal_clear(path) || before.0 != before.1 {
        return Err(unreadable());
    }
    let db = if before.0 {
        Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
    } else {
        let mut uri = reqwest::Url::from_file_path(path).map_err(|_| unreadable())?;
        uri.set_query(Some("immutable=1"));
        Connection::open_with_flags(
            uri.as_str(),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )
    }
    .map_err(|_| unreadable())?;
    db.busy_timeout(Duration::from_millis(150))
        .map_err(|_| unreadable())?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, (max + 2048) as i32)
        .map_err(|_| unreadable())?;
    let start = Instant::now();
    db.progress_handler(
        1000,
        Some(move || start.elapsed() > Duration::from_millis(300)),
    )
    .map_err(|_| unreadable())?;
    let bytes: Option<Zeroizing<Vec<u8>>> = db
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1",
            [key],
            |row| match row.get_ref(0)? {
                ValueRef::Text(b) | ValueRef::Blob(b) if b.len() <= max => {
                    Ok(Zeroizing::new(b.to_vec()))
                }
                ValueRef::Null => Ok(Zeroizing::new(vec![])),
                _ => Err(rusqlite::Error::InvalidQuery),
            },
        )
        .optional()
        .map_err(|_| unreadable())?;
    if !journal_clear(path) || (!before.0 && sidecars(path)? != (false, false)) {
        return Err(unreadable());
    }
    let bytes = bytes.filter(|b| !b.is_empty()).ok_or_else(missing)?;
    let data = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(&bytes);
    let (pairs, remainder) = data.as_chunks::<2>();
    if remainder.is_empty()
        && (bytes.starts_with(&[0xff, 0xfe])
            || pairs
                .iter()
                .all(|p| p[0].is_ascii() && p[0] != 0 && p[1] == 0))
    {
        let words = Zeroizing::new(
            pairs
                .iter()
                .map(|p| u16::from_le_bytes(*p))
                .collect::<Vec<_>>(),
        );
        String::from_utf16(&words)
            .map(Zeroizing::new)
            .map_err(|_| unreadable())
    } else {
        std::str::from_utf8(&bytes)
            .map(|s| Zeroizing::new(s.to_owned()))
            .map_err(|_| unreadable())
    }
}
