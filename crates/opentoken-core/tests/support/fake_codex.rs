//! A deterministic, real stdio child used by transport regression tests. Not shipped by the app.
use fs2::FileExt;
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::{self, BufRead, Write},
    path::PathBuf,
};
fn main() {
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").expect("isolated home"));
    if std::env::args().nth(1).as_deref() == Some("login") {
        std::fs::write(home.join("login-started"), "synthetic-only").unwrap();
        use std::io::IsTerminal;
        std::fs::write(home.join("login-console.json"),serde_json::to_vec(&json!({"stdin":io::stdin().is_terminal(),"stdout":io::stdout().is_terminal(),"stderr":io::stderr().is_terminal()})).unwrap()).unwrap();
        let polluted = [
            "CODEX_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "OPENAI_API_KEY",
            "CODEX_SQLITE_HOME",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
        ]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
        std::fs::write(home.join("login-env-safe"), (!polluted).to_string()).unwrap();
        for _ in 0..150 {
            if home.join("complete-sign-in").is_file() {
                let id = home.file_name().unwrap().to_string_lossy();
                std::fs::write(home.join("account.json"), serde_json::to_vec(&json!({"workspace":format!("signin-{id}"),"email":"new-signin@example.test","percent":22})).unwrap()).unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::process::exit(1);
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("child.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    std::fs::write(home.join("started"), "true").unwrap();
    if home.join("stall").exists() {
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    let fixture: Value = std::fs::read(home.join("account.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(
            || json!({"workspace":"fixture","email":"fixture@example.test","percent":25}),
        );
    let polluted = [
        "CODEX_API_KEY",
        "CODEX_ACCESS_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_SQLITE_HOME",
    ]
    .iter()
    .any(|k| std::env::var_os(k).is_some());
    for line in io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let Some(id) = v["id"].as_u64() else {
            continue;
        };
        let result = match v["method"].as_str().unwrap() {
            "initialize" => json!({}),
            "account/read" => {
                let workspace = if home.join("switched").exists() {
                    "wrong"
                } else {
                    fixture["workspace"].as_str().unwrap()
                };
                json!({"account":{"type":"chatgpt","email":fixture["email"],"accountId":workspace,"planType":"plus"}})
            }
            "account/rateLimits/read" => {
                let mut reads = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(home.join("provider-reads"))
                    .unwrap();
                writeln!(reads, "read").unwrap();
                if home.join("switch").exists() {
                    std::fs::write(home.join("switched"), "true").unwrap();
                }
                json!({"rateLimits":{"primary":{"usedPercent":if polluted{99}else{fixture["percent"].as_u64().unwrap_or(25)},"windowDurationMins":300,"resetsAt":fixture["sessionReset"]},"secondary":{"usedPercent":67,"windowDurationMins":10080,"resetsAt":fixture["weeklyReset"]},"credits":{"balance":fixture["credits"]}},"rateLimitResetCredits":{"availableCount":fixture["resets"]}})
            }
            _ => json!({}),
        };
        println!("{}", json!({"method":"notification/ignore","params":{}}));
        println!("{}", json!({"id":id,"result":result}));
        io::stdout().flush().unwrap();
    }
}
