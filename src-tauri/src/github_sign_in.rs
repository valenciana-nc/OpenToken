//! Delegate sign-in to GitHub's installed CLI; never borrow an OAuth client ID.
use opentoken_core::{
    connections::{find_github_cli, github_config_home},
    cursor_login::{self, Flow, Host, Options, Status},
    engine::Engine,
    model::{Fault, FaultKind},
    storage::Storage,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Emitter;
struct NativeHost {
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    child: Mutex<Option<crate::login_process::LoginProcess>>,
}
impl Drop for NativeHost {
    fn drop(&mut self) {
        if let Ok(Some(child)) = self.child.get_mut().map(Option::as_mut) {
            child.stop();
        }
    }
}
#[async_trait::async_trait]
impl Host for NativeHost {
    async fn probe(&self) -> Result<String, Fault> {
        {
            let mut owned = self.child.lock().unwrap();
            if let Some(child) = owned.as_mut() {
                match child.try_wait() {
                    Ok(Some(true)) => {
                        *owned = None;
                    }
                    Ok(Some(false)) => {
                        return Err(Fault::new(
                            FaultKind::Unknown,
                            "GitHub sign-in closed before completion. Try again.",
                        ))
                    }
                    Ok(None) => {
                        return Err(Fault::new(
                            FaultKind::NotSignedIn,
                            "Finish the official GitHub CLI sign-in window.",
                        ))
                    }
                    Err(_) => {
                        return Err(Fault::new(
                            FaultKind::Unreadable,
                            "The GitHub sign-in window could not be checked.",
                        ))
                    }
                }
            }
        }
        self.engine.github_sign_in_marker(true).await
    }
    async fn connect(&self) -> Result<(), Fault> {
        self.engine
            .connect_github_personal(true)
            .await
            .map(|_| ())
            .map_err(|e| {
                e.downcast_ref::<Fault>()
                    .cloned()
                    .unwrap_or_else(|| Fault::new(FaultKind::Unknown, e.to_string()))
            })
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        let exe=find_github_cli().ok_or_else(||Fault::new(FaultKind::ToolMissing,"Install the official GitHub CLI, then try again. Nothing was installed automatically."))?;
        let home = github_config_home(&self.storage);
        std::fs::create_dir_all(&home).map_err(|_| {
            Fault::new(
                FaultKind::Unreadable,
                "The GitHub CLI configuration location is unavailable.",
            )
        })?;
        let process = crate::login_process::LoginProcess::spawn_args(
            &exe,
            &[
                "auth",
                "login",
                "--web",
                "--hostname",
                "github.com",
                "--git-protocol",
                "https",
            ],
            "GH_CONFIG_DIR",
            &home,
        )
        .map_err(|_| {
            Fault::new(
                FaultKind::ToolMissing,
                "The GitHub CLI sign-in window could not open.",
            )
        })?;
        *self.child.lock().unwrap() = Some(process);
        Ok(())
    }
    fn publish(&self, status: Status) {
        let _ = self.app.emit("github-login-changed", status);
    }
}
pub fn start(
    flow: Arc<Flow>,
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    consent: bool,
) -> Result<Status, String> {
    if !consent {
        return Err("Start Copilot sign-in explicitly to reuse the GitHub CLI login.".into());
    }
    if let Some(run) = flow.begin_named("Copilot") {
        let status = flow.status();
        let _ = app.emit("github-login-changed", status.clone());
        let host = Arc::new(NativeHost {
            engine,
            storage,
            app,
            child: Mutex::new(None),
        });
        tauri::async_runtime::spawn(cursor_login::run(flow,run,host,Options{name:"Copilot",waiting_message:"Finish GitHub CLI sign-in in its window/browser. GitHub CLI requests its standard permissions; OpenToken reads only identity and Copilot usage.",waiting_timeout:Duration::from_secs(600),poll_interval:Duration::from_secs(2),..Default::default()}));
        return Ok(status);
    }
    Ok(flow.status())
}
