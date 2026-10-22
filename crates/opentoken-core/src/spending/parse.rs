use crate::model::hash;
#[path = "clients.rs"]
mod clients;
#[path = "muse.rs"]
mod muse;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct State {
    pub session: String,
    pub model: String,
    pub tier: String,
    pub turn: String,
    pub ordinal: u64,
    pub last_total: String,
    pub last_direct: Option<(i64, Tokens)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Tokens {
    pub input: u64,
    pub cached: u64,
    pub write: u64,
    pub write_hour: u64,
    pub output: u64,
}
impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.cached + self.write + self.write_hour + self.output
    }
}
pub(super) struct Event {
    pub id: String,
    pub session: String,
    pub model: String,
    pub at: i64,
    pub kind: &'static str,
    pub tier: String,
    pub tokens: Tokens,
}
// Deserialize only metadata and counters. Prompt, tool, assistant-content and instruction fields are ignored.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Record {
    timestamp: Option<String>,
    r#type: String,
    payload: Payload,
    message: Message,
    request_id: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Payload {
    r#type: String,
    id: Option<String>,
    session_id: Option<String>,
    thread_id: Option<String>,
    response_id: Option<String>,
    turn_id: Option<String>,
    model: Option<String>,
    service_tier: Option<String>,
    usage: Counts,
    thread_token_usage: Counts,
    info: Option<Info>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Info {
    total_token_usage: Counts,
    last_token_usage: Counts,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Counts,
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Counts {
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    cache_write_input_tokens: u64,
    cache_read_input_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_creation: CacheCreation,
    service_tier: Option<String>,
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct CacheCreation {
    ephemeral_1h_input_tokens: u64,
    ephemeral_5m_input_tokens: u64,
}
fn valid(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
}
fn openai(c: &Counts) -> Tokens {
    let cached = c.cached_input_tokens.min(c.input_tokens);
    let write = c.cache_write_input_tokens.min(c.input_tokens - cached);
    Tokens {
        input: c.input_tokens - cached - write,
        cached,
        write,
        output: c.output_tokens,
        ..Default::default()
    }
}
pub(super) fn many(
    line: &[u8],
    provider: &str,
    state: &mut State,
) -> Result<Vec<Event>, serde_json::Error> {
    match provider {
        "grok" => clients::grok(line, state),
        "kimi" => clients::kimi(line, state),
        _ => Ok(parse(line, provider, state)?.into_iter().collect()),
    }
}
pub(super) fn parse(
    line: &[u8],
    provider: &str,
    state: &mut State,
) -> Result<Option<Event>, serde_json::Error> {
    if provider == "muse" {
        return muse::parse(line, state);
    }
    let r: Record = serde_json::from_slice(line)?;
    if provider == "codex" {
        if r.r#type == "session_meta" {
            if let Some(id) = valid(r.payload.id) {
                state.session = hash(&id);
            }
            return Ok(None);
        }
        if r.r#type == "turn_context" {
            if let Some(model) = valid(r.payload.model) {
                state.model = model.to_lowercase();
            }
            state.tier = valid(r.payload.service_tier).unwrap_or_default();
            state.turn = valid(r.payload.turn_id).unwrap_or_default();
            return Ok(None);
        }
    }
    let Some(at) = r
        .timestamp
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|t| t.timestamp_millis())
    else {
        return Ok(None);
    };
    let model = if provider == "claude" {
        valid(r.message.model).unwrap_or_else(|| "unknown".into())
    } else {
        state.model.clone()
    };
    let model = if model.is_empty() {
        "unknown".into()
    } else {
        model
    };
    let (id, session, kind, tokens, tier) = match (provider, r.r#type.as_str()) {
        ("codex", "token_usage_record") => {
            let p = r.payload;
            let session = valid(p.session_id.or(p.thread_id))
                .map(|s| hash(&s))
                .unwrap_or_else(|| state.session.clone());
            let id = valid(p.response_id).unwrap_or_else(|| {
                format!(
                    "{}:{}",
                    session,
                    serde_json::to_string(&p.thread_token_usage).unwrap_or_default()
                )
            });
            let tokens = openai(&p.usage);
            state.last_direct = Some((at, tokens.clone()));
            (
                hash(&format!("codex:direct:{id}")),
                session,
                "direct",
                tokens,
                state.tier.clone(),
            )
        }
        ("codex", "event_msg") if r.payload.r#type == "token_count" => {
            let Some(info) = r.payload.info else {
                return Ok(None);
            };
            let total = hash(&serde_json::to_string(&info.total_token_usage).unwrap_or_default());
            if total == state.last_total {
                return Ok(None);
            }
            state.last_total = total.clone();
            let tokens = openai(&info.last_token_usage);
            if state
                .last_direct
                .as_ref()
                .is_some_and(|(time, t)| at.abs_diff(*time) <= 2000 && t == &tokens)
            {
                return Ok(None);
            }
            let scope = if state.turn.is_empty() {
                &state.session
            } else {
                &state.turn
            };
            let id = hash(&format!("codex:fallback:{scope}:{at}:{total}"));
            (
                id,
                state.session.clone(),
                "fallback",
                tokens,
                state.tier.clone(),
            )
        }
        ("claude", "assistant") => {
            let Some(id) = valid(r.message.id.or(r.request_id)) else {
                return Ok(None);
            };
            let c = r.message.usage;
            let write_hour = c
                .cache_creation
                .ephemeral_1h_input_tokens
                .min(c.cache_creation_input_tokens);
            let write = c
                .cache_creation_input_tokens
                .saturating_sub(write_hour)
                .max(c.cache_creation.ephemeral_5m_input_tokens);
            (
                hash(&format!("claude:{id}")),
                state.session.clone(),
                "claude",
                Tokens {
                    input: c.input_tokens,
                    cached: c.cache_read_input_tokens,
                    output: c.output_tokens,
                    write,
                    write_hour,
                },
                c.service_tier.unwrap_or_default(),
            )
        }
        _ => return Ok(None),
    };
    if [
        tokens.input,
        tokens.cached,
        tokens.write,
        tokens.write_hour,
        tokens.output,
    ]
    .iter()
    .any(|v| *v > 1_000_000_000_000)
        || tokens.total() == 0
    {
        return Ok(None);
    }
    Ok(Some(Event {
        id,
        session,
        model: model.to_lowercase(),
        at,
        kind,
        tier,
        tokens,
    }))
}
pub(super) fn relevant(line: &[u8], provider: &str) -> bool {
    if matches!(provider, "grok" | "kimi") {
        return true;
    }
    let prefix = &line[..line.len().min(1024)];
    let s = String::from_utf8_lossy(prefix);
    if provider == "muse" {
        return s.contains("\"runtime.session\"");
    }
    if provider == "claude" {
        s.contains("\"assistant\"")
    } else {
        [
            "\"session_meta\"",
            "\"turn_context\"",
            "\"token_usage_record\"",
            "\"event_msg\"",
        ]
        .iter()
        .any(|t| s.contains(t))
    }
}
