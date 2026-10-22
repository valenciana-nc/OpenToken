//! Maintainer probe: usage-summary <isolated-index-dir> codex=<home> [claude=<home>].
//! Reads CLI logs without changing them; prints aggregate counters/value, never transcript content.
use opentoken_core::{spending::UsageIndex, storage::Settings};
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().ok_or_else(|| {
        anyhow::anyhow!("Supply an isolated index directory and provider=home sources.")
    })?);
    let mut roots = vec![];
    for arg in args.take(256) {
        let (provider, home) = arg
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Use provider=home."))?;
        anyhow::ensure!(
            ["codex", "claude"].contains(&provider),
            "Unsupported provider."
        );
        roots.push((provider.to_owned(), std::path::PathBuf::from(home)));
    }
    anyhow::ensure!(!roots.is_empty(), "Supply at least one source.");
    let mut index = UsageIndex::open(&dir)?;
    for _ in 0..1000 {
        let summary = index.scan(&roots, &Settings::default(), &[], chrono::Utc::now())?;
        if !summary.indexing {
            println!("{}", serde_json::to_string(&summary)?);
            return Ok(());
        }
    }
    anyhow::bail!("Indexing budget exhausted; rerun to resume.");
}
