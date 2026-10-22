//! A single bounded transient retry for read-only GitHub/Grok session observations.
//! This stays inside the caller's existing deadline/read cycle; authentication failures,
//! permission errors, malformed data, redirects and provider pauses are never retried here.
use super::*;

impl Service {
    pub(super) async fn session_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        provider: Provider,
        stage: &'static str,
    ) -> Result<T, Fault> {
        let mut request = request.build().map_err(|_| invalid())?;
        if request.method() != reqwest::Method::GET {
            return Err(invalid());
        }
        for attempt in 0..2 {
            let retry_request = (attempt == 0).then(|| request.try_clone()).flatten();
            let response = self.http.execute(request).await;
            let (result, transient) = match response {
                Ok(response) => {
                    let status = response.status();
                    let transient = (status.is_success()
                        || matches!(status.as_u16(), 408 | 500 | 502 | 503 | 504))
                        && !response.headers().contains_key("retry-after");
                    (self.decode_json_response(response, false).await, transient)
                }
                Err(error) => (
                    Err(Fault::offline()),
                    error.is_connect() || error.is_timeout() || error.is_body(),
                ),
            };
            match result {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => return Err(invalid()),
                Err(fault) => {
                    if transient && fault.kind == FaultKind::Offline {
                        if let Some(next) = retry_request {
                            // A retry is still part of this budgeted observation, not another
                            // scheduled cycle or a new credential/login operation.
                            tokio::time::sleep(Duration::from_millis(250)).await;
                            request = next;
                            continue;
                        }
                    }
                    tracing::warn!(?provider, stage, kind=?fault.kind, "Session usage stage failed");
                    return Err(Fault {
                        message: format!("{stage}: {}", fault.message),
                        ..fault
                    });
                }
            }
        }
        unreachable!("the second attempt always returns")
    }
}
