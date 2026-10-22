use opentoken_core::{
    connections::{find_grok_cli, grok_config_home},
    cursor_login::{self, Flow, Host, Options, Status},
    engine::Engine,
    model::{Fault, FaultKind},
    storage::Storage,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::Emitter;
fn owner_arguments(recover: bool) -> &'static [&'static str] {
    if recover {
        &["models"]
    } else {
        &["login", "--oauth"]
    }
}
struct NativeHost {
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    child: Mutex<Option<crate::login_process::LoginProcess>>,
    recover: bool,
    completed: AtomicBool,
    launched: AtomicBool,
}
#[async_trait::async_trait]
impl Host for NativeHost {
    async fn cooldown_seconds(&self) -> u64 {
        self.engine
            .verification_wait_secs(opentoken_core::model::Provider::Grok)
            .await
    }
    async fn probe(&self) -> Result<String, Fault> {
        if self.recover && !self.launched.load(Ordering::SeqCst) {
            return Err(Fault::new(
                FaultKind::NotSignedIn,
                "Official CLI recovery was explicitly requested.",
            ));
        }
        {
            let mut owned = self.child.lock().unwrap();
            if let Some(child) = owned.as_mut() {
                match child.try_wait() {
                    Ok(Some(true)) => {
                        *owned = None;
                        self.completed.store(true, Ordering::SeqCst);
                    }
                    Ok(Some(false)) => {
                        return Err(Fault::new(
                            FaultKind::Unknown,
                            "Grok sign-in closed before completion. Try again.",
                        ))
                    }
                    Ok(None) => {
                        return Err(Fault::new(
                            FaultKind::NotSignedIn,
                            "Finish the official Grok sign-in window.",
                        ))
                    }
                    Err(_) => {
                        return Err(Fault::new(
                            FaultKind::Unreadable,
                            "Grok's sign-in window could not be checked.",
                        ))
                    }
                }
            }
        }
        self.engine
            .grok_sign_in_marker(true)
            .await
            .map_err(|mut f|{if self.recover&&self.completed.load(Ordering::SeqCst)&&matches!(f.kind,FaultKind::NotSignedIn|FaultKind::NeedsRelogin){f.kind=FaultKind::PermissionDenied;f.message="The official Grok models check finished without an eligible saved OAuth session. Choose Sign in with Grok explicitly; no login fallback was started.".into();}f})
            .map(|m| format!("{m}:{}", self.completed.load(Ordering::SeqCst)))
    }
    async fn connect(&self) -> Result<(), Fault> {
        self.engine
            .connect_grok_personal(true)
            .await
            .map(|_| ())
            .map_err(|e| {
                let mut f=e.downcast_ref::<Fault>().cloned().unwrap_or_else(||Fault::new(FaultKind::Unknown,e.to_string()));
                if self.recover&&self.completed.load(Ordering::SeqCst)&&f.kind==FaultKind::NeedsRelogin{f.kind=FaultKind::PermissionDenied;f.message="The official Grok model check did not restore an eligible OAuth session. Choose Sign in with Grok to use its own explicit --oauth flow. No new login was launched automatically.".into();}f
            })
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        let exe=find_grok_cli().ok_or_else(||Fault::new(FaultKind::ToolMissing,"Install the official Grok CLI, then try again. Nothing was installed automatically."))?;
        let home = grok_config_home(&self.storage);
        if !home.is_absolute() {
            return Err(Fault::new(
                FaultKind::Unreadable,
                "GROK_HOME must be absolute. No other profile was selected.",
            ));
        }
        std::fs::create_dir_all(&home)
            .map_err(|_| Fault::new(FaultKind::Unreadable, "The Grok CLI home is unavailable."))?;
        let child = crate::login_process::LoginProcess::spawn_args(
            &exe,
            owner_arguments(self.recover),
            "GROK_HOME",
            &home,
        )
        .map_err(|_| {
            Fault::new(
                FaultKind::ToolMissing,
                "Grok's sign-in window could not open.",
            )
        })?;
        *self.child.lock().unwrap() = Some(child);
        self.launched.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn publish(&self, status: Status) {
        let _ = self.app.emit("grok-login-changed", status);
    }
}
pub fn start(
    flow: Arc<Flow>,
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    consent: bool,
    recover: bool,
) -> Result<Status, String> {
    if !consent {
        return Err(
            "Start Grok sign-in explicitly to allow read-only CLI-session monitoring.".into(),
        );
    }
    if let Some(run) = flow.begin_named("Grok") {
        let status = flow.status();
        let _ = app.emit("grok-login-changed", status.clone());
        let host = Arc::new(NativeHost {
            engine,
            storage,
            app,
            child: Mutex::new(None),
            recover,
            completed: AtomicBool::new(false),
            launched: AtomicBool::new(false),
        });
        tauri::async_runtime::spawn(cursor_login::run(
            flow,
            run,
            host,
            Options {
                name: "Grok",
                waiting_message: if recover {
                    "The official Grok models command may renew its own saved access. Finish its window if needed; no inference prompt or automatic new OAuth fallback is requested."
                } else {
                    "Finish the official Grok OAuth window/browser. We’ll verify identity and usage automatically."
                },
                waiting_timeout: Duration::from_secs(600),
                ..Default::default()
            },
        ));
        return Ok(status);
    }
    Ok(flow.status())
}
#[cfg(test)]
mod tests {
    #[test]
    fn owner_commands_match_oauth_scope_and_recovery_has_no_prompt() {
        assert_eq!(super::owner_arguments(false), ["login", "--oauth"]);
        assert_eq!(super::owner_arguments(true), ["models"]);
    }
}
