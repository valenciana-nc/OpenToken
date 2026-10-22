//! Read-only loopback API. It reads published snapshots and never triggers provider calls.
use crate::{
    engine::Engine,
    model::{Provider, Snapshot},
};
use axum::{
    extract::{Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tokio::sync::watch;

async fn guard(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    if headers.contains_key(header::ORIGIN)
        || headers.contains_key("sec-fetch-site")
        || !matches!(host, "127.0.0.1" | "localhost")
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"native_loopback_clients_only"})),
        )
            .into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}
fn state_response(snapshot: Snapshot, headers: &HeaderMap) -> Response {
    let tag = format!("\"{}\"", snapshot.revision);
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(&tag)
    {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    let mut response = Json(snapshot).into_response();
    response
        .headers_mut()
        .insert(header::ETAG, tag.parse().unwrap());
    response
}
async fn usage(State(engine): State<Engine>, headers: HeaderMap) -> Response {
    state_response(engine.state.borrow().clone(), &headers)
}
async fn accounts(State(engine): State<Engine>) -> Response {
    let snapshot = engine.state.borrow().clone();
    Json(json!({"scannedAt":snapshot.scanned_at,"accounts":snapshot.accounts.iter().map(|a|json!({"id":a.account.id,"provider":a.account.provider,"label":a.account.label,"email":a.account.email,"status":a.status,"folderCount":a.account.sources.len()})).collect::<Vec<_>>()})).into_response()
}
async fn filtered(State(engine): State<Engine>, Path(id): Path<String>) -> Response {
    let mut s = engine.state.borrow().clone();
    s.accounts.retain(|a| {
        a.account.id == id
            || match id.as_str() {
                "codex" => a.account.provider == Provider::Codex,
                "claude" => a.account.provider == Provider::Claude,
                "cursor" => a.account.provider == Provider::Cursor,
                "copilot" => a.account.provider == Provider::Copilot,
                "zai" => a.account.provider == Provider::Zai,
                "deepseek" => a.account.provider == Provider::Deepseek,
                "kimi" => a.account.provider == Provider::Kimi,
                "opencode" => a.account.provider == Provider::Opencode,
                "grok" => a.account.provider == Provider::Grok,
                "antigravity" => a.account.provider == Provider::Antigravity,
                "muse" => a.account.provider == Provider::Muse,
                "devin" => a.account.provider == Provider::Devin,
                _ => false,
            }
    });
    if s.accounts.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(s).into_response()
}
pub fn router(engine: Engine) -> Router {
    Router::new()
        .route("/v1/state", get(usage))
        .route("/v1/usage", get(usage))
        .route("/v1/usage/{id}", get(filtered))
        .route("/v1/accounts", get(accounts))
        .route(
            "/health",
            get(|| async { Json(json!({"app":"OpenToken","version":env!("CARGO_PKG_VERSION")})) }),
        )
        .layer(middleware::from_fn(guard))
        .with_state(engine)
}
pub async fn serve(engine: Engine, mut stop: watch::Receiver<bool>) -> anyhow::Result<()> {
    let port = engine.settings.borrow().api_port;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    axum::serve(listener, router(engine))
        .with_graceful_shutdown(async move {
            let _ = stop.changed().await;
        })
        .await?;
    Ok(())
}
