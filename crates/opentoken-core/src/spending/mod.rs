//! Local, read-only CLI usage indexing. Token value is an estimate, never a subscription invoice.
mod index;
mod parse;
use crate::{model::AccountState, storage::Settings};
use anyhow::{bail, Result};
use chrono::{DateTime, Days, Local, TimeZone, Utc};
pub use index::UsageIndex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelPrice {
    pub input: f64,
    pub cached: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_write_hour: f64,
}
impl ModelPrice {
    pub fn validate(&self) -> Result<()> {
        if [
            self.input,
            self.cached,
            self.output,
            self.cache_write,
            self.cache_write_hour,
        ]
        .iter()
        .any(|n| !n.is_finite() || *n < 0.0 || *n > 1_000_000.0)
        {
            bail!("Model rates must be finite USD amounts between 0 and 1,000,000 per million tokens.");
        }
        Ok(())
    }
    fn value(&self, t: &parse::Tokens) -> f64 {
        (t.input as f64 * self.input
            + t.cached as f64 * self.cached
            + t.output as f64 * self.output
            + t.write as f64 * self.cache_write
            + t.write_hour as f64 * self.cache_write_hour)
            / 1_000_000.0
    }
}
#[derive(Deserialize)]
struct Catalog {
    date: String,
    models: BTreeMap<String, BTreeMap<String, f64>>,
}
fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("prices.json")).expect("bundled price catalog")
    })
}
fn reference_price(model: &str, input: u64, tier: &str) -> Option<ModelPrice> {
    let rates = catalog().models.get(model)?;
    let context = if input > 272_000 && rates.contains_key("input_cost_per_token_above_272k_tokens")
    {
        "_above_272k_tokens"
    } else if input > 200_000 && rates.contains_key("input_cost_per_token_above_200k_tokens") {
        "_above_200k_tokens"
    } else {
        ""
    };
    let tier = match tier {
        "batch" | "batches" => "_batches",
        "flex" => "_flex",
        "priority" => "_priority",
        "ultrafast" => "_ultrafast",
        _ => "",
    };
    let get = |key: &str| {
        rates
            .get(&format!("{key}{context}{tier}"))
            .or_else(|| rates.get(&format!("{key}{context}")))
            .or_else(|| rates.get(&format!("{key}{tier}")))
            .or_else(|| rates.get(key))
            .copied()
            .unwrap_or(0.0)
            * 1_000_000.0
    };
    Some(ModelPrice {
        input: get("input_cost_per_token"),
        cached: get("cache_read_input_token_cost"),
        output: get("output_cost_per_token"),
        cache_write: get("cache_creation_input_token_cost"),
        cache_write_hour: get("cache_creation_input_token_cost_above_1hr"),
    })
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSpend {
    pub provider: String,
    pub tokens: u64,
    pub estimated_usd: Option<f64>,
    pub unpriced_tokens: u64,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditChange {
    pub provider: String,
    pub unit: String,
    pub decrease: f64,
    pub increase: f64,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendPeriod {
    pub tokens: u64,
    pub estimated_usd: Option<f64>,
    pub unpriced_tokens: u64,
    pub providers: Vec<ProviderSpend>,
    pub credit_changes: Vec<CreditChange>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spending {
    pub updated_at: DateTime<Utc>,
    pub indexing: bool,
    pub files_indexed: usize,
    pub files_pending: usize,
    pub skipped_records: u64,
    pub pricing_date: String,
    pub periods: BTreeMap<String, SpendPeriod>,
    pub unpriced_models: Vec<String>,
    pub reference_prices: BTreeMap<String, ModelPrice>,
    pub credit_tracking_since: Option<DateTime<Utc>>,
}
pub fn sources(
    settings: &Settings,
    accounts: &[AccountState],
    user_home: &Path,
) -> Vec<(String, PathBuf)> {
    let mut roots = BTreeSet::new();
    for (provider, enabled, homes, default) in [
        (
            "codex",
            settings.codex_enabled,
            &settings.codex_homes,
            ".codex",
        ),
        (
            "claude",
            settings.claude_enabled,
            &settings.claude_homes,
            ".claude",
        ),
    ] {
        let key = if provider == "codex" {
            crate::model::Provider::Codex
        } else {
            crate::model::Provider::Claude
        };
        if !settings
            .history_sources
            .get(&key)
            .copied()
            .unwrap_or(enabled)
        {
            continue;
        }
        if homes.is_empty() || (provider == "codex" && settings.auto_discover) {
            let variable = if provider == "codex" {
                "CODEX_HOME"
            } else {
                "CLAUDE_CONFIG_DIR"
            };
            let home = std::env::var_os(variable)
                .map(PathBuf::from)
                .unwrap_or_else(|| user_home.join(default));
            roots.insert((provider.to_string(), home));
        }
        for p in homes {
            roots.insert((
                provider.to_string(),
                crate::storage::normalize(p, user_home),
            ));
        }
        for a in accounts
            .iter()
            .filter(|a| provider_name(a.account.provider) == provider)
        {
            for source in &a.account.sources {
                roots.insert((provider.to_string(), source.path.clone()));
            }
        }
    }
    let store = crate::storage::Storage {
        dir: PathBuf::new(),
        user_home: user_home.into(),
    };
    for (provider, enabled) in &settings.history_sources {
        if *enabled {
            for root in crate::monitoring::history_roots(&store, *provider) {
                roots.insert((provider.key().to_string(), root));
            }
        }
    }
    let ignored: BTreeSet<_> = settings
        .ignored_homes
        .iter()
        .map(|p| crate::storage::path_key(&crate::storage::normalize(p, user_home)))
        .collect();
    roots
        .into_iter()
        .filter(|(_, p)| !ignored.contains(&crate::storage::path_key(p)))
        .take(256)
        .collect()
}
fn counter(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    value
        .try_into()
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn provider_name(provider: crate::model::Provider) -> &'static str {
    provider.key()
}
fn midnight(date: chrono::NaiveDate) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).expect("midnight");
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| naive.and_utc().timestamp_millis())
}
fn periods(now: DateTime<Utc>) -> [(String, i64, i64); 3] {
    let today = now.with_timezone(&Local).date_naive();
    [
        ("Today".into(), midnight(today), now.timestamp_millis()),
        (
            "Yesterday".into(),
            midnight(today - Days::new(1)),
            midnight(today) - 1,
        ),
        (
            "30 Days".into(),
            midnight(today - Days::new(29)),
            now.timestamp_millis(),
        ),
    ]
}
impl UsageIndex {
    pub fn scan(
        &mut self,
        roots: &[(String, PathBuf)],
        settings: &Settings,
        accounts: &[AccountState],
        now: DateTime<Utc>,
    ) -> Result<Spending> {
        self.scan_paths(roots, settings, accounts, now, None)
    }
    pub fn scan_paths(
        &mut self,
        roots: &[(String, PathBuf)],
        settings: &Settings,
        accounts: &[AccountState],
        now: DateTime<Utc>,
        paths: Option<&[PathBuf]>,
    ) -> Result<Spending> {
        let (files_indexed, files_pending, skipped_records) = self.update(roots, paths)?;
        observe_credits(&mut self.db, accounts, now)?;
        let mut result = Spending {
            updated_at: now,
            indexing: files_pending > 0,
            files_indexed,
            files_pending,
            skipped_records,
            pricing_date: catalog().date.clone(),
            periods: BTreeMap::new(),
            unpriced_models: vec![],
            reference_prices: BTreeMap::new(),
            credit_tracking_since: None,
        };
        let ranges = periods(now);
        let since = ranges[2].1;
        // New Codex response records supersede matching legacy token_count echoes, not additional usage.
        let mut statement = self.db.prepare("SELECT e.provider,e.model,e.at,e.input,e.cached,e.write5,e.write1,e.output,e.tier FROM events e WHERE e.at>=?1 AND e.at<=?2 AND NOT(e.kind='fallback' AND EXISTS(SELECT 1 FROM events d WHERE d.kind='direct' AND d.provider=e.provider AND d.session=e.session AND d.at BETWEEN e.at-2000 AND e.at+2000 AND d.input=e.input AND d.cached=e.cached AND d.write5=e.write5 AND d.write1=e.write1 AND d.output=e.output))")?;
        let mut rows = statement.query(params![since, now.timestamp_millis()])?;
        let mut buckets: BTreeMap<String, BTreeMap<String, ProviderSpend>> = BTreeMap::new();
        let mut unpriced = BTreeSet::new();
        while let Some(row) = rows.next()? {
            let provider: String = row.get(0)?;
            let model: String = row.get(1)?;
            let at: i64 = row.get(2)?;
            let t = parse::Tokens {
                input: counter(row, 3)?,
                cached: counter(row, 4)?,
                write: counter(row, 5)?,
                write_hour: counter(row, 6)?,
                output: counter(row, 7)?,
            };
            let tier: String = row.get(8)?;
            let price = settings.model_prices.get(&model).cloned().or_else(|| {
                reference_price(&model, t.input + t.cached + t.write + t.write_hour, &tier)
            });
            if let Some(base) = reference_price(&model, 0, "") {
                result.reference_prices.insert(model.clone(), base);
            }
            if price.is_none() {
                unpriced.insert(model);
            }
            for (name, from, to) in &ranges {
                if at >= *from && at <= *to {
                    let p = buckets
                        .entry(name.clone())
                        .or_default()
                        .entry(provider.clone())
                        .or_insert_with(|| ProviderSpend {
                            provider: provider.clone(),
                            ..Default::default()
                        });
                    p.tokens += t.total();
                    if let Some(price) = &price {
                        *p.estimated_usd.get_or_insert(0.0) += price.value(&t);
                    } else {
                        p.unpriced_tokens += t.total();
                    }
                }
            }
        }
        for (name, _, _) in &ranges {
            let providers: Vec<_> = buckets
                .remove(name)
                .unwrap_or_default()
                .into_values()
                .collect();
            let priced: Vec<_> = providers.iter().filter_map(|p| p.estimated_usd).collect();
            result.periods.insert(
                name.clone(),
                SpendPeriod {
                    tokens: providers.iter().map(|p| p.tokens).sum(),
                    unpriced_tokens: providers.iter().map(|p| p.unpriced_tokens).sum(),
                    estimated_usd: (!priced.is_empty()).then(|| priced.iter().sum()),
                    providers,
                    credit_changes: vec![],
                },
            );
        }
        let mut credits = self.db.prepare("SELECT provider,unit,at,balance,LAG(balance) OVER(PARTITION BY account,unit ORDER BY at) FROM credits ORDER BY at")?;
        let mut rows = credits.query([])?;
        while let Some(row) = rows.next()? {
            let provider: String = row.get(0)?;
            let unit: String = row.get(1)?;
            let at: i64 = row.get(2)?;
            let balance: f64 = row.get(3)?;
            let previous: Option<f64> = row.get(4)?;
            if result.credit_tracking_since.is_none() {
                result.credit_tracking_since = DateTime::from_timestamp_millis(at);
            }
            let Some(previous) = previous else {
                continue;
            };
            for (name, from, to) in &ranges {
                if at >= *from && at <= *to {
                    let changes = &mut result.periods.get_mut(name).expect("period").credit_changes;
                    let i = changes
                        .iter()
                        .position(|c| c.provider == provider && c.unit == unit)
                        .unwrap_or_else(|| {
                            changes.push(CreditChange {
                                provider: provider.clone(),
                                unit: unit.clone(),
                                ..Default::default()
                            });
                            changes.len() - 1
                        });
                    changes[i].decrease += (previous - balance).max(0.0);
                    changes[i].increase += (balance - previous).max(0.0);
                }
            }
        }
        result.unpriced_models = unpriced.into_iter().collect();
        Ok(result)
    }
}
fn observe_credits(
    db: &mut Connection,
    accounts: &[AccountState],
    now: DateTime<Utc>,
) -> Result<()> {
    let tx = db.transaction()?;
    for state in accounts {
        // Never derive a new credit observation from cached/stale or unverified readings.
        if state.status != crate::model::Status::Ready
            || state
                .report
                .as_ref()
                .is_some_and(|r| r.source != crate::model::ReportSource::Provider)
        {
            continue;
        }
        let Some(report) = &state.report else {
            continue;
        };
        for q in report.quotas.iter().filter(|q| q.id == "credits") {
            let Some(balance) = q.value.filter(|v| v.is_finite() && *v >= 0.0) else {
                continue;
            };
            let unit = q.unit.as_deref().unwrap_or("credits");
            let previous = tx.query_row(
                "SELECT balance FROM credits WHERE account=?1 AND unit=?2 ORDER BY at DESC LIMIT 1",
                params![state.account.id, unit],
                |r| r.get::<_, f64>(0),
            );
            if previous.ok() != Some(balance) {
                tx.execute("INSERT OR IGNORE INTO credits(account,provider,unit,at,balance) VALUES(?1,?2,?3,?4,?5)", params![state.account.id, provider_name(state.account.provider), unit, now.timestamp_millis(), balance])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}
