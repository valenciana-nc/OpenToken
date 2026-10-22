use super::parse::{self, State};
use crate::model::hash;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_LINE: usize = 2 * 1024 * 1024;
const FILE_BUDGET: usize = 8 * 1024 * 1024;
const SCAN_BUDGET: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 40_000;
pub struct UsageIndex {
    pub(super) db: Connection,
    activity: HashMap<String, crate::activity::Activity>,
}
struct LogFile {
    provider: String,
    home: PathBuf,
    path: PathBuf,
    modified: u64,
    size: u64,
}
impl UsageIndex {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let db = Connection::open(dir.join("usage.sqlite3"))?;
        db.busy_timeout(std::time::Duration::from_secs(3))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,provider TEXT NOT NULL,session TEXT NOT NULL,model TEXT NOT NULL,at INTEGER NOT NULL,kind TEXT NOT NULL,input INTEGER NOT NULL,cached INTEGER NOT NULL,write5 INTEGER NOT NULL,write1 INTEGER NOT NULL,output INTEGER NOT NULL,tier TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS events_date ON events(at);
            CREATE INDEX IF NOT EXISTS events_echo ON events(provider,session,kind,at);
            CREATE TABLE IF NOT EXISTS files(id TEXT PRIMARY KEY,offset INTEGER NOT NULL,modified INTEGER NOT NULL,prefix TEXT NOT NULL,state TEXT NOT NULL,discard INTEGER NOT NULL,skipped INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS credits(account TEXT NOT NULL,provider TEXT NOT NULL,unit TEXT NOT NULL,at INTEGER NOT NULL,balance REAL NOT NULL,PRIMARY KEY(account,unit,at));")?;
        Ok(Self {
            db,
            activity: HashMap::new(),
        })
    }
    pub fn take_activity(&mut self) -> Vec<crate::activity::Activity> {
        std::mem::take(&mut self.activity).into_values().collect()
    }
    pub(super) fn update(
        &mut self,
        roots: &[(String, PathBuf)],
        paths: Option<&[PathBuf]>,
    ) -> Result<(usize, usize, u64)> {
        self.activity.clear();
        let mut files = vec![];
        let mut seen = HashSet::new();
        let mut directories = 0;
        for (provider, root) in roots {
            let children: &[&str] = if provider == "codex" {
                &["sessions", "archived_sessions"]
            } else if matches!(provider.as_str(), "grok" | "kimi") {
                &["sessions"]
            } else if provider == "muse" {
                &[""]
            } else {
                &["projects"]
            };
            for child in children {
                let history = root.join(child);
                if let Some(paths) = paths {
                    for path in paths {
                        if safe_history_file(&history, path) {
                            push_file(provider, root, path, &mut files, &mut seen);
                        }
                    }
                } else {
                    walk(
                        provider,
                        root,
                        &history,
                        0,
                        &mut files,
                        &mut seen,
                        &mut directories,
                    );
                }
            }
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.modified));
        let mut budget = SCAN_BUDGET;
        let mut pending = 0;
        let mut indexed = 0;
        for f in files {
            let key = hash(&format!("{}:{}", f.provider, f.path.to_string_lossy()));
            let previous: Option<(u64, u64, String, String, bool, u64)> = self
                .db
                .query_row(
                    "SELECT offset,modified,prefix,state,discard,skipped FROM files WHERE id=?1",
                    [&key],
                    |r| {
                        Ok((
                            super::counter(r, 0)?,
                            super::counter(r, 1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            super::counter(r, 5)?,
                        ))
                    },
                )
                .optional()?;
            if previous
                .as_ref()
                .is_some_and(|p| p.0 == f.size && p.1 == f.modified)
            {
                indexed += 1;
                continue;
            }
            if budget == 0 {
                pending += 1;
                continue;
            }
            let Ok(mut input) = File::open(&f.path) else {
                continue;
            };
            let mut first = vec![0; (f.size as usize).min(128)];
            if input.read_exact(&mut first).is_err() {
                continue;
            }
            let prefix = hash(&String::from_utf8_lossy(&first));
            let (mut offset, mut state, mut discard, mut skipped) = match previous {
                Some((o, m, p, s, d, k))
                    if o <= f.size && p == prefix && !(o == f.size && m != f.modified) =>
                {
                    (o, serde_json::from_str(&s).unwrap_or_default(), d, k)
                }
                _ => (
                    0,
                    State {
                        session: session_key(&f.provider, &f.path),
                        ..Default::default()
                    },
                    false,
                    0,
                ),
            };
            input.seek(SeekFrom::Start(offset))?;
            let mut reader = BufReader::with_capacity(64 * 1024, input);
            let mut allowed = FILE_BUDGET.min(budget);
            let start = allowed;
            let mut eof = false;
            let tx = self.db.transaction()?;
            while allowed > 0 {
                let line_start = offset;
                let mut line = Vec::new();
                let mut complete = false;
                while allowed > 0 {
                    let bytes = reader.fill_buf()?;
                    if bytes.is_empty() {
                        eof = true;
                        break;
                    }
                    let n = bytes.len().min(allowed);
                    let end = bytes[..n].iter().position(|c| *c == b'\n').map(|p| p + 1);
                    let take = end.unwrap_or(n);
                    if !discard {
                        if line.len() + take <= MAX_LINE {
                            line.extend_from_slice(&bytes[..take]);
                        } else {
                            if parse::relevant(&line, &f.provider) {
                                skipped += 1;
                            }
                            line.clear();
                            discard = true;
                        }
                    }
                    reader.consume(take);
                    offset += take as u64;
                    allowed -= take;
                    if end.is_some() {
                        complete = true;
                        break;
                    }
                }
                if !complete {
                    // An incomplete append must be retried after the writer finishes the record.
                    if !discard {
                        offset = line_start;
                    }
                    break;
                }
                if discard {
                    discard = false;
                    continue;
                }
                if !parse::relevant(&line, &f.provider) {
                    continue;
                }
                let quota = if f.provider == "codex" {
                    crate::activity::quota_record(&line, f.home.clone())
                } else {
                    None
                };
                if let Some(sample) = &quota {
                    record_activity(&mut self.activity, sample.clone());
                }
                match parse::many(&line, &f.provider, &mut state) {
                    Ok(events) => {
                        for e in events {
                            if quota.is_none() && matches!(f.provider.as_str(), "codex" | "claude")
                            {
                                if let Some(observed_at) =
                                    chrono::DateTime::from_timestamp_millis(e.at)
                                {
                                    record_activity(
                                        &mut self.activity,
                                        crate::activity::Activity {
                                            provider: if f.provider == "codex" {
                                                crate::model::Provider::Codex
                                            } else {
                                                crate::model::Provider::Claude
                                            },
                                            home: f.home.clone(),
                                            observed_at,
                                            identity: None,
                                            quota: None,
                                        },
                                    );
                                }
                            }
                            if e.at < chrono::Utc::now().timestamp_millis() - 32 * 86_400_000 {
                                continue;
                            }
                            let t = e.tokens;
                            // Streaming Claude snapshots are revisions, not separate calls. MAX counters
                            // and the stable response ID also deduplicate copied/imported session files.
                            tx.execute("INSERT INTO events(id,provider,session,model,at,kind,input,cached,write5,write1,output,tier) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                            ON CONFLICT(id) DO UPDATE SET input=MAX(input,excluded.input),cached=MAX(cached,excluded.cached),write5=MAX(write5,excluded.write5),write1=MAX(write1,excluded.write1),output=MAX(output,excluded.output),at=MIN(at,excluded.at),model=CASE WHEN model='unknown' THEN excluded.model ELSE model END", params![e.id,f.provider,e.session,e.model,e.at,e.kind,t.input as i64,t.cached as i64,t.write as i64,t.write_hour as i64,t.output as i64,e.tier])?;
                        }
                    }
                    Err(_) => skipped += 1,
                }
            }
            budget -= start - allowed;
            tx.execute("INSERT INTO files(id,offset,modified,prefix,state,discard,skipped) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET offset=excluded.offset,modified=excluded.modified,prefix=excluded.prefix,state=excluded.state,discard=excluded.discard,skipped=excluded.skipped", params![key,offset as i64,f.modified as i64,prefix,serde_json::to_string(&state)?,discard,skipped as i64])?;
            tx.commit()?;
            if offset < f.size && !eof {
                pending += 1;
            } else {
                indexed += 1;
            }
        }
        let skipped: u64 =
            self.db
                .query_row("SELECT COALESCE(SUM(skipped),0) FROM files", [], |r| {
                    super::counter(r, 0)
                })?;
        // Only counters are retained. Bound event retention, but keep cursor/dedup metadata.
        self.db.execute(
            "DELETE FROM events WHERE at<?1",
            [chrono::Utc::now().timestamp_millis() - 32 * 86_400_000],
        )?;
        Ok((indexed, pending, skipped))
    }
}
fn session_key(provider: &str, path: &Path) -> String {
    let parent = path.parent().unwrap_or(path);
    let id = if provider == "kimi"
        && parent
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "agents")
    {
        format!(
            "{}:{}",
            parent
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy(),
            parent.file_name().unwrap_or_default().to_string_lossy()
        )
    } else if matches!(provider, "grok" | "kimi" | "muse") {
        parent
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    hash(&id)
}
fn walk(
    provider: &str,
    home: &Path,
    path: &Path,
    depth: usize,
    files: &mut Vec<LogFile>,
    seen: &mut HashSet<PathBuf>,
    directories: &mut usize,
) {
    if depth == 0
        && fs::symlink_metadata(home).map_or(true, |m| m.file_type().is_symlink() || reparse(&m))
    {
        return;
    }
    if depth > 8 || files.len() >= MAX_FILES || *directories >= 8000 {
        return;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.file_type().is_symlink() || reparse(&meta) {
        return;
    }
    if !meta.is_dir() {
        return;
    }
    *directories += 1;
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= MAX_FILES {
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let p = entry.path();
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(provider, home, &p, depth + 1, files, seen, directories);
        } else if kind.is_file() && p.extension().is_some_and(|e| e == "jsonl") {
            push_file(provider, home, &p, files, seen);
        }
    }
}
fn record_activity(
    activity: &mut HashMap<String, crate::activity::Activity>,
    sample: crate::activity::Activity,
) {
    let key = format!(
        "{:?}:{}",
        sample.provider,
        crate::storage::path_key(&sample.home)
    );
    if activity.get(&key).is_some_and(|old| {
        old.observed_at > sample.observed_at
            || (old.observed_at == sample.observed_at && old.quota.is_some())
    }) {
        return;
    }
    if activity.contains_key(&key) || activity.len() < 256 {
        activity.insert(key, sample);
    }
}
fn push_file(
    provider: &str,
    home: &Path,
    path: &Path,
    files: &mut Vec<LogFile>,
    seen: &mut HashSet<PathBuf>,
) {
    if files.len() >= MAX_FILES
        || path.extension().is_none_or(|e| e != "jsonl")
        || (provider == "muse" && path.file_name().is_none_or(|n| n != "session.jsonl"))
        || (provider == "grok" && path.file_name().is_none_or(|n| n != "updates.jsonl"))
        || (provider == "kimi" && path.file_name().is_none_or(|n| n != "wire.jsonl"))
    {
        return;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) {
        return;
    }
    let Ok(canonical) = path.canonicalize() else {
        return;
    };
    if !seen.insert(canonical.clone()) {
        return;
    }
    let modified = meta
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as u64;
    files.push(LogFile {
        provider: provider.into(),
        home: home.into(),
        path: canonical,
        modified,
        size: meta.len(),
    });
}
fn safe_history_file(root: &Path, path: &Path) -> bool {
    let (Ok(actual_root), Ok(actual_file)) = (root.canonicalize(), path.canonicalize()) else {
        return false;
    };
    if !actual_file.starts_with(actual_root) {
        return false;
    }
    for parent in path.ancestors() {
        let Ok(meta) = fs::symlink_metadata(parent) else {
            return false;
        };
        if meta.file_type().is_symlink() || reparse(&meta) {
            return false;
        }
        if root
            .parent()
            .is_some_and(|home| crate::storage::path_key(parent) == crate::storage::path_key(home))
        {
            return true;
        }
    }
    false
}
#[cfg(windows)]
fn reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool {
    false
}
