//! Original metadata-only readers for recorded CLI usage, independent of account authentication.
use super::{valid, Event, State, Tokens};
use crate::model::hash;
use serde::Deserialize;
use std::collections::BTreeMap;
fn bad() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Unsupported or inconsistent CLI token record.",
    ))
}
fn event(
    provider: &str,
    id: &str,
    session: &str,
    at: i64,
    model: Option<String>,
    tokens: Tokens,
) -> Result<Option<Event>, serde_json::Error> {
    if at <= 0
        || [
            tokens.input,
            tokens.cached,
            tokens.write,
            tokens.write_hour,
            tokens.output,
        ]
        .into_iter()
        .any(|n| n > 1_000_000_000_000)
    {
        return Err(bad());
    }
    if tokens.total() == 0 {
        return Ok(None);
    }
    Ok(Some(Event {
        id: hash(&format!("{provider}:{id}")),
        session: session.into(),
        at,
        model: valid(model)
            .unwrap_or_else(|| "unknown".into())
            .to_lowercase(),
        tokens,
        kind: "client",
        tier: String::new(),
    }))
}
#[derive(Deserialize)]
struct Grok {
    #[serde(default)]
    params: Option<GrokParams>,
    #[serde(default)]
    timestamp: Option<f64>,
}
#[derive(Deserialize)]
struct GrokParams {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    update: Option<GrokUpdate>,
    #[serde(rename = "_meta", default)]
    meta: Option<GrokMeta>,
}
#[derive(Deserialize)]
struct GrokMeta {
    #[serde(rename = "eventId")]
    event_id: Option<String>,
    #[serde(rename = "agentTimestampMs")]
    at: Option<f64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokUpdate {
    session_update: Option<String>,
    usage: Option<GrokUsage>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokUsage {
    #[serde(flatten)]
    totals: GrokCounts,
    #[serde(default)]
    model_usage: BTreeMap<String, GrokCounts>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokCounts {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    #[serde(default)]
    cached_read_tokens: u64,
    #[serde(default)]
    cache_creation_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}
fn grok_tokens(c: GrokCounts) -> Result<Tokens, serde_json::Error> {
    let (input, output) = (
        c.input_tokens.ok_or_else(bad)?,
        c.output_tokens.ok_or_else(bad)?,
    );
    if c.cached_read_tokens > input
        || c.cache_creation_tokens > input - c.cached_read_tokens
        || c.reasoning_tokens > output
    {
        return Err(bad());
    }
    Ok(Tokens {
        input: input - c.cached_read_tokens - c.cache_creation_tokens,
        cached: c.cached_read_tokens,
        write: c.cache_creation_tokens,
        output,
        ..Default::default()
    })
}
pub(super) fn grok(line: &[u8], state: &mut State) -> Result<Vec<Event>, serde_json::Error> {
    let r: Grok = serde_json::from_slice(line)?;
    let Some(p) = r.params else {
        return Ok(vec![]);
    };
    let Some(u) = p.update else {
        return Ok(vec![]);
    };
    if u.session_update.as_deref() != Some("turn_completed") {
        return Ok(vec![]);
    }
    let usage = u.usage.ok_or_else(bad)?;
    let meta = p.meta.ok_or_else(bad)?;
    let at = meta
        .at
        .filter(|n| n.is_finite() && *n > 0. && *n < 253_402_300_799_000.)
        .map(|n| n as i64)
        .or_else(|| {
            r.timestamp
                .filter(|n| n.is_finite() && *n > 0. && *n < 253_402_300_799.)
                .map(|n| (n * 1000.) as i64)
        })
        .ok_or_else(bad)?;
    let id = valid(meta.event_id).ok_or_else(bad)?;
    let session = valid(p.session_id)
        .map(|s| hash(&s))
        .unwrap_or_else(|| state.session.clone());
    let mut result = vec![];
    if usage.model_usage.is_empty() {
        if let Some(e) = event(
            "grok",
            &format!("{session}:{id}"),
            &session,
            at,
            None,
            grok_tokens(usage.totals)?,
        )? {
            result.push(e);
        }
    } else {
        if usage.model_usage.len() > 100 {
            return Err(bad());
        }
        for (model, counts) in usage.model_usage {
            if let Some(e) = event(
                "grok",
                &format!("{session}:{id}:{model}"),
                &session,
                at,
                Some(model),
                grok_tokens(counts)?,
            )? {
                result.push(e);
            }
        }
    }
    Ok(result)
}
#[derive(Deserialize)]
struct Kimi {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<KimiMessage>,
    #[serde(default)]
    timestamp: Option<f64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<KimiNewUsage>,
    #[serde(rename = "usageScope", default)]
    scope: Option<String>,
    #[serde(default)]
    time: Option<i64>,
}
#[derive(Deserialize)]
struct KimiMessage {
    #[serde(rename = "type")]
    kind: Option<String>,
    payload: Option<KimiPayload>,
}
#[derive(Deserialize)]
struct KimiPayload {
    token_usage: Option<KimiUsage>,
    message_id: Option<String>,
}
#[derive(Deserialize)]
struct KimiUsage {
    input_other: u64,
    output: u64,
    #[serde(default)]
    input_cache_creation: u64,
    #[serde(default)]
    input_cache_read: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KimiNewUsage {
    input_other: u64,
    output: u64,
    #[serde(default)]
    input_cache_creation: u64,
    #[serde(default)]
    input_cache_read: u64,
}
pub(super) fn kimi(line: &[u8], state: &mut State) -> Result<Vec<Event>, serde_json::Error> {
    state.ordinal = state.ordinal.saturating_add(1);
    let r: Kimi = serde_json::from_slice(line)?;
    let (at, id, model, tokens) = if r.kind.as_deref() == Some("usage.record") {
        if r.scope.as_deref() != Some("turn") {
            return Ok(vec![]);
        }
        let u = r.usage.ok_or_else(bad)?;
        let at = r.time.ok_or_else(bad)?;
        (
            at,
            format!("{}:{}", state.session, state.ordinal),
            r.model,
            Tokens {
                input: u.input_other,
                cached: u.input_cache_read,
                write: u.input_cache_creation,
                output: u.output,
                ..Default::default()
            },
        )
    } else {
        let Some(m) = r.message else {
            return Ok(vec![]);
        };
        if m.kind.as_deref() != Some("StatusUpdate") {
            return Ok(vec![]);
        }
        let Some(p) = m.payload else {
            return Ok(vec![]);
        };
        let Some(u) = p.token_usage else {
            return Ok(vec![]);
        };
        let at = r
            .timestamp
            .filter(|n| n.is_finite() && *n > 0. && *n < 253_402_300_799.)
            .ok_or_else(bad)?;
        let id = valid(p.message_id).unwrap_or_else(|| state.ordinal.to_string());
        (
            (at * 1000.) as i64,
            format!("{}:{id}", state.session),
            r.model,
            Tokens {
                input: u.input_other,
                cached: u.input_cache_read,
                write: u.input_cache_creation,
                output: u.output,
                ..Default::default()
            },
        )
    };
    Ok(event("kimi", &id, &state.session, at, model, tokens)?
        .into_iter()
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn grok_model_breakdowns_replace_totals_and_cached_reasoning_are_not_added() {
        let v = json!({"params":{"sessionId":"s","_meta":{"eventId":"turn-1","agentTimestampMs":1893456000000i64},"update":{"sessionUpdate":"turn_completed","usage":{"inputTokens":200,"outputTokens":50,"modelUsage":{"grok-a":{"inputTokens":100,"outputTokens":20,"cachedReadTokens":30,"reasoningTokens":5},"grok-b":{"inputTokens":100,"outputTokens":30,"cacheCreationTokens":10}}}}}});
        let rows = grok(&serde_json::to_vec(&v).unwrap(), &mut State::default()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().map(|r| r.tokens.total()).sum::<u64>(), 250);
        assert_ne!(rows[0].id, rows[1].id);
    }
    #[test]
    fn grok_missing_timestamp_or_event_identity_cannot_fabricate_history() {
        let v = json!({"params":{"update":{"sessionUpdate":"turn_completed","usage":{"inputTokens":20,"outputTokens":5}}}});
        assert!(grok(&serde_json::to_vec(&v).unwrap(), &mut State::default()).is_err());
    }
    #[test]
    fn kimi_turn_records_exclude_session_rollups_and_do_not_collapse_equal_turns() {
        let v = json!({"type":"usage.record","usageScope":"turn","model":"synthetic-kimi","time":1893456000000i64,"usage":{"inputOther":100,"output":10,"inputCacheRead":30,"inputCacheCreation":5}});
        let bytes = serde_json::to_vec(&v).unwrap();
        let mut state = State {
            session: "session-fixture".into(),
            ..Default::default()
        };
        let a = kimi(&bytes, &mut state).unwrap().remove(0);
        let b = kimi(&bytes, &mut state).unwrap().remove(0);
        assert_eq!(a.tokens.total(), 145);
        assert_ne!(a.id, b.id);
        let mut rollup = v;
        rollup["usageScope"] = json!("session");
        assert!(kimi(&serde_json::to_vec(&rollup).unwrap(), &mut state)
            .unwrap()
            .is_empty());
    }
}
