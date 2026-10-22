//! Original typed Muse runtime-session reader. No prompt/content deserialization.
use super::{valid, Event, State, Tokens};
use crate::model::hash;
use serde::Deserialize;
#[derive(Deserialize)]
struct Record {
    schema_version: u64,
    record_type: String,
    payload_type: String,
    payload_schema_version: u64,
    id: String,
    recorded_at: i64,
    payload: Payload,
}
#[derive(Deserialize)]
struct Payload {
    event: EventData,
}
#[derive(Deserialize)]
struct EventData {
    kind: String,
    #[serde(default)]
    model: Option<Model>,
    #[serde(default)]
    usage: Option<Usage>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Model {
    Text(String),
    Descriptor { model_id: String },
}
#[derive(Deserialize, Default)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cached_tokens: Option<u64>,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}
fn bad() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Unsupported or inconsistent Muse usage counters.",
    ))
}
pub(super) fn parse(line: &[u8], state: &State) -> Result<Option<Event>, serde_json::Error> {
    let r: Record = serde_json::from_slice(line)?;
    if r.schema_version != 1
        || r.record_type != "event"
        || r.payload_type != "runtime.session"
        || r.payload_schema_version != 1
    {
        return Err(bad());
    }
    let e = r.payload.event;
    if !matches!(
        e.kind.as_str(),
        "model_completed" | "automated_review_completed"
    ) {
        return Ok(None);
    }
    let u = e.usage.ok_or_else(bad)?;
    let Some(id) = valid(Some(r.id)) else {
        return Err(bad());
    };
    if r.recorded_at <= 0
        || r.recorded_at > 253_402_300_799_999_999
        || [
            u.input_tokens,
            u.output_tokens,
            u.cache_write_tokens,
            u.reasoning_tokens,
        ]
        .iter()
        .any(|v| *v > 1_000_000_000_000)
    {
        return Err(bad());
    }
    let mut cache = None;
    for value in [u.cache_read_tokens, u.cached_input_tokens, u.cached_tokens]
        .into_iter()
        .flatten()
    {
        if value > u.input_tokens || cache.is_some_and(|c| c != value) {
            return Err(bad());
        }
        cache = Some(value);
    }
    let cached = cache.unwrap_or(0);
    if u.cache_write_tokens > u.input_tokens - cached
        || u.reasoning_tokens > u.output_tokens
        || u.total_tokens
            .is_some_and(|v| v != u.input_tokens + u.output_tokens)
    {
        return Err(bad());
    }
    let tokens = Tokens {
        input: u.input_tokens - cached - u.cache_write_tokens,
        cached,
        write: u.cache_write_tokens,
        output: u.output_tokens,
        ..Default::default()
    };
    if tokens.total() == 0 {
        return Ok(None);
    }
    let model = valid(e.model.map(|m| match m {
        Model::Text(s) => s,
        Model::Descriptor { model_id } => model_id,
    }))
    .unwrap_or_else(|| "unknown".into());
    Ok(Some(Event {
        id: hash(&format!("muse:{id}")),
        session: state.session.clone(),
        model: model.to_lowercase(),
        at: r.recorded_at / 1000,
        kind: "muse",
        tier: String::new(),
        tokens,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record() -> serde_json::Value {
        json!({"schema_version":1,"record_type":"event","payload_type":"runtime.session","payload_schema_version":1,"id":"synthetic-muse-event","recorded_at":1893456000000000i64,"payload":{"event":{"kind":"model_completed","model":"synthetic-muse","usage":{"input_tokens":100,"output_tokens":25,"cached_tokens":40,"cache_read_tokens":40,"reasoning_tokens":10,"total_tokens":125},"prompt":"THIS MUST NOT BE INDEXED"}}})
    }
    #[test]
    fn cache_and_reasoning_are_subsets_not_extra_tokens() {
        let event = parse(&serde_json::to_vec(&record()).unwrap(), &State::default())
            .unwrap()
            .unwrap();
        assert_eq!(event.tokens.total(), 125);
        assert_eq!(event.tokens.input, 60);
        assert_eq!(event.at, 1893456000000);
    }
    #[test]
    fn review_descriptors_and_stable_event_ids_support_deduplication() {
        let mut v = record();
        v["payload"]["event"]["kind"] = json!("automated_review_completed");
        v["payload"]["event"]["model"] = json!({"model_id":"MUSE-REVIEW"});
        let bytes = serde_json::to_vec(&v).unwrap();
        let a = parse(&bytes, &State::default()).unwrap().unwrap();
        let b = parse(
            &bytes,
            &State {
                session: "other-import-path".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.model, "muse-review");
    }
    #[test]
    fn schema_drift_inconsistent_counts_and_aliases_are_not_silent_zero() {
        for field in ["schema", "total", "alias", "negative"] {
            let mut v = record();
            match field {
                "schema" => v["schema_version"] = json!(2),
                "total" => v["payload"]["event"]["usage"]["total_tokens"] = json!(999),
                "alias" => v["payload"]["event"]["usage"]["cached_tokens"] = json!(41),
                _ => v["payload"]["event"]["usage"]["input_tokens"] = json!(-1),
            }
            assert!(parse(&serde_json::to_vec(&v).unwrap(), &State::default()).is_err());
        }
    }
}
