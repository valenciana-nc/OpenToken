//! Synthetic transport executable for isolated tests, never a real provider login.
use serde_json::{json, Value};
use std::{
    io::{BufRead, IsTerminal, Write},
    path::PathBuf,
};
fn muse() {
    let auth = std::env::var_os("MUSE_AUTH_PATH")
        .map(PathBuf::from)
        .expect("isolated auth path");
    assert!(auth.is_absolute() && auth.to_string_lossy().contains("fixture"));
    std::fs::write(
        auth.parent().unwrap().join("fixture-msp-pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    for line in std::io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let method = v["method"].as_str().unwrap_or("");
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(auth.parent().unwrap().join("fixture-msp-methods.log"))
            .unwrap();
        writeln!(log, "{method}").unwrap();
        let result = match method {
            "initialize" => {
                assert_eq!(v["params"]["clientInfo"]["name"], "opentoken");
                json!({"serverInfo":{"name":"synthetic-muse","version":"1.4.10"},"schema":{"version":1},"museHome":auth.parent().unwrap()})
            }
            "initialized" => continue,
            "usage/read" => {
                if auth.parent().unwrap().join("fixture-wait").exists() {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                if auth.parent().unwrap().join("fixture-empty").exists() {
                    json!({})
                } else {
                    let now = chrono::Utc::now().timestamp_millis();
                    let old = auth.parent().unwrap().join("fixture-expired").exists();
                    json!({"usage":{"observedAtMs":now-1000,"tier":"High · fixture","window":{"usedPercent":25,"resetsAtMs":if old{now-1000}else{now+3600000},"windowDurationMins":300},"weekly":{"usedPercent":40,"resetsAtMs":if old{now-1000}else{now+86400000}}}})
                }
            }
            _ => std::process::exit(23),
        };
        println!("{}", json!({"jsonrpc":"2.0","id":v["id"],"result":result}));
        std::io::stdout().flush().unwrap();
    }
}
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--version"] || args == ["--print", "/usage", "--output-format", "json"] {
        let home = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        assert!(home.is_absolute() && home.to_string_lossy().contains("fixture"));
        assert_eq!(
            std::env::var("AGY_CLI_DISABLE_AUTO_UPDATE").unwrap(),
            "true"
        );
        assert!(std::env::var_os("GEMINI_API_KEY").is_none());
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(home.join("fixture-cli-invocations"))
            .unwrap();
        writeln!(log, "{}", args.join(" ")).unwrap();
        if args == ["--version"] {
            println!(
                "{}",
                if home.join("fixture-cli-old-version").exists() {
                    "1.2.1"
                } else {
                    "1.3.5"
                }
            );
            return;
        }
        if home.join("fixture-cli-refused").exists() {
            eprintln!("Authentication required. fixture-sensitive-do-not-export");
            std::process::exit(1);
        }
        if home.join("fixture-cli-invalid").exists() {
            println!("fixture invalid quota envelope");
            return;
        }
        println!(
            "{}",
            json!({"status":"SUCCESS","num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0},"command":{"name":"usage","data":{"groups":[{"buckets":[{"id":"gemini-5h","remaining_fraction":0.75},{"id":"3p-weekly","remaining_fraction":0.6}]}]}}})
        );
        return;
    }
    if args.first().is_some_and(|s| s == "models") {
        let home = PathBuf::from(std::env::var_os("GROK_HOME").expect("isolated Grok home"));
        assert!(home.is_absolute() && home.to_string_lossy().contains("opentoken-smoke-"));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("fixture-models-started"),json!({"pid":std::process::id(),"stdin":std::io::stdin().is_terminal(),"stdout":std::io::stdout().is_terminal(),"stderr":std::io::stderr().is_terminal()}).to_string()).unwrap();
        while !home.join("fixture-models-finish").exists() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        return;
    }
    if args.is_empty()
        && std::env::current_exe()
            .unwrap()
            .file_name()
            .is_some_and(|n| n == "Antigravity.exe")
    {
        let home =
            PathBuf::from(std::env::var_os("OPENTOKEN_SOURCE_HOME").expect("isolated source home"));
        assert!(home.is_absolute() && home.to_string_lossy().contains("opentoken-smoke-"));
        let exe = std::env::current_exe()
            .unwrap()
            .with_file_name("language_server_windows_x64.exe");
        let mut command = std::process::Command::new(exe);
        command
            .args([
                "--app_data_dir",
                "antigravity",
                "--override_ide_name",
                "antigravity",
                "--csrf_token",
                "synthetic-recovery-csrf",
                "--fixture-output",
            ])
            .arg(
                home.parent()
                    .unwrap()
                    .join("fixture-antigravity-recovery.json"),
            )
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command.spawn().unwrap().wait().unwrap();
        return;
    }
    if args.first().is_some_and(|s| s == "serve") {
        muse();
        return;
    }
    let pos = args
        .iter()
        .position(|s| s == "--fixture-output")
        .expect("synthetic fixture output required");
    let out = PathBuf::from(args.get(pos + 1).unwrap());
    assert!(out.is_absolute() && out.to_string_lossy().contains("fixture"));
    let csrf = args
        .iter()
        .position(|s| s == "--csrf_token")
        .and_then(|i| args.get(i + 1))
        .unwrap()
        .clone();
    let state = std::sync::Arc::new((csrf, out.parent().unwrap().to_owned()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    std::fs::write(
        &out,
        json!({"pid":std::process::id(),"port":port}).to_string(),
    )
    .unwrap();
    let app=axum::Router::new().route("/{*path}",axum::routing::post(|axum::extract::State(state):axum::extract::State<std::sync::Arc<(String,PathBuf)>>,axum::extract::Path(path):axum::extract::Path<String>,headers:axum::http::HeaderMap,axum::Json(body):axum::Json<Value>|async move{
        if headers.get("x-codeium-csrf-token").and_then(|v|v.to_str().ok())!=Some(state.0.as_str()){return (axum::http::StatusCode::FORBIDDEN,axum::Json(json!({})));}
        if path.ends_with("RetrieveUserQuotaSummary"){assert_eq!(body,json!({"forceRefresh":true}),"summary must use its own protobuf schema, not GetUserStatus metadata");}else if path.ends_with("GetUserStatus"){assert_eq!(body["metadata"]["extensionName"],"opentoken");}
        let switched=state.1.join("fixture-switch").exists();
        let value=if path.ends_with("GetUserStatus"){json!({"userStatus":{"email":if switched{"changed@fixture.test"}else{"antigravity@fixture.test"},"userTier":{"name":"Pro · fixture"}}})}
        else if path.ends_with("RetrieveUserQuotaSummary"){json!({"response":{"groups":[{"displayName":"Gemini","buckets":[{"bucketId":"gemini-5h","remainingFraction":0.75},{"bucketId":"gemini-weekly","remainingFraction":0.5}]},{"displayName":"Third-party","buckets":[{"bucketId":"3p-5h","remainingFraction":0.9},{"bucketId":"3p-weekly","remainingFraction":0.6}]}]}})}
        else{return (axum::http::StatusCode::NOT_FOUND,axum::Json(json!({})));};
        (axum::http::StatusCode::OK,axum::Json(value))
    })).with_state(state);
    let stop = out.parent().unwrap().join("fixture-antigravity-stop");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !stop.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
}
