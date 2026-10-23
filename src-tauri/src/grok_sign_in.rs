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
fn owner_arguments(recover: bool, device: bool) -> &'static [&'static str] {
    if recover {
        &["models"]
    } else if device {
        &["login", "--device-auth"]
    } else {
        &["login", "--oauth"]
    }
}
struct NativeHost {
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    child: Mutex<Option<crate::login_process::LoginProcess>>,
    owner: Mutex<Option<tokio::sync::OwnedMutexGuard<()>>>,
    recover: bool,
    device: bool,
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
        if !self.launched.load(Ordering::SeqCst) {
            return Err(Fault::new(
                FaultKind::NotSignedIn,
                "An official CLI sign-in/recovery command was explicitly requested.",
            ));
        }
        {
            let mut owned = self.child.lock().unwrap();
            if let Some(child) = owned.as_mut() {
                match child.try_wait() {
                    Ok(Some(true)) => {
                        *owned = None;
                        self.owner.lock().unwrap().take();
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
            .map_err(|mut f|{if self.completed.load(Ordering::SeqCst)&&matches!(f.kind,FaultKind::NotSignedIn|FaultKind::NeedsRelogin){f.kind=FaultKind::PermissionDenied;f.message="The official Grok command finished without a usable saved OAuth session. Try device-code sign-in. OpenToken did not start another login automatically.".into();}f})
            .map(|m| format!("{m}:{}", self.completed.load(Ordering::SeqCst)))
    }
    async fn connect(&self) -> Result<(), Fault> {
        self.engine
            .connect_grok_personal(true)
            .await
            .map(|_| ())
            .map_err(|e| {
                let mut f=e.downcast_ref::<Fault>().cloned().unwrap_or_else(||Fault::new(FaultKind::Unknown,e.to_string()));
                if self.completed.load(Ordering::SeqCst)&&matches!(f.kind,FaultKind::NeedsRelogin|FaultKind::NotSignedIn|FaultKind::IdentityChanged){f.kind=FaultKind::PermissionDenied;f.message=if self.recover{"The official Grok model check did not restore an eligible OAuth session. Choose Sign in with Grok explicitly. No new login was launched automatically."}else{"Grok sign-in finished, but its saved session did not pass identity/quota verification. The browser step alone is not a completed connection. Try device-code sign-in; no second login was started automatically."}.into();}f
            })
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        let lease = self.engine.grok_owner.clone().try_lock_owned().map_err(|_| Fault::new(FaultKind::Unreadable, "An approved Grok renewal is already running. Wait for it to finish, then try sign-in again."))?;
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
            owner_arguments(self.recover, self.device),
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
        *self.owner.lock().unwrap() = Some(lease);
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
    device: bool,
) -> Result<Status, String> {
    if recover && device {
        return Err("Choose either session recovery or device-code sign-in.".into());
    }
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
            owner: Mutex::new(None),
            recover,
            device,
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
                } else if device {
                    "In the Grok terminal, find the device code. Enter that code on the official page Grok opens, then approve sign-in. Leave the terminal open; no browser return code needs to be pasted into OpenToken."
                } else {
                    "Leave the Grok terminal open. If the browser does not return automatically, copy the FULL return URL into the terminal at ‘Paste the URL here if it doesn’t connect’ and press Enter. A code alone is not enough. OpenToken never asks for that code."
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
        assert_eq!(super::owner_arguments(false, false), ["login", "--oauth"]);
        assert_eq!(
            super::owner_arguments(false, true),
            ["login", "--device-auth"]
        );
        assert_eq!(super::owner_arguments(true, false), ["models"]);
    }
}
