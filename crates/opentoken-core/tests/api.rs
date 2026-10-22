use opentoken_core::{api, engine::Engine, model::Snapshot, storage::Settings};
use tokio::sync::{mpsc, watch};
#[tokio::test]
async fn loopback_api_is_read_only_and_rejects_browser_and_rebinding() {
    let (commands, _rx) = mpsc::channel(4);
    let (_state, state) = watch::channel(Snapshot::default());
    let (_settings, settings) = watch::channel(Settings::default());
    let e = Engine::from_receivers(commands, state, settings);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, api::router(e)).await.unwrap() });
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/v1/state");
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let etag = response.headers()["etag"].clone();
    let value: serde_json::Value = response.json().await.unwrap();
    assert!(value["accounts"].as_array().unwrap().is_empty());
    assert!(!value.to_string().contains("access_token"));
    assert_eq!(
        client
            .get(&url)
            .header("If-None-Match", etag)
            .send()
            .await
            .unwrap()
            .status(),
        304
    );
    assert_eq!(
        client
            .get(&url)
            .header("Origin", "https://malicious.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(&url)
            .header("Host", "rebind.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(client.post(&url).send().await.unwrap().status(), 405);
    server.abort();
}
