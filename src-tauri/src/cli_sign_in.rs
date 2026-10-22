//! Official CLI authentication, isolated per account and coordinated natively.
use opentoken_core::{
    cli_profiles::{CliProvider, Profile},
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
#[derive(Default)]
pub struct Flows {
    codex: Arc<Flow>,
    claude: Arc<Flow>,
}
impl Flows {
    pub fn get(&self, provider: CliProvider) -> Arc<Flow> {
        match provider {
            CliProvider::Codex => self.codex.clone(),
            CliProvider::Claude => self.claude.clone(),
        }
    }
    pub fn reset(&self) {
        self.codex.reset();
        self.claude.reset();
    }
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    provider: CliProvider,
    status: Status,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Changed {
    provider: CliProvider,
    settings: opentoken_core::storage::Settings,
}
pub fn publish(app: &tauri::AppHandle, provider: CliProvider, status: Status) {
    let _ = app.emit("cli-login-changed", Progress { provider, status });
}
pub fn changed(
    app: &tauri::AppHandle,
    provider: CliProvider,
    settings: opentoken_core::storage::Settings,
) {
    let _ = app.emit("cli-accounts-changed", Changed { provider, settings });
}
struct NativeHost {
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    profile: Profile,
    child: Mutex<Option<crate::login_process::LoginProcess>>,
    finished: Mutex<bool>,
    flow: Arc<Flow>,
    run: u64,
}
impl Drop for NativeHost {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            if let Some(mut child) = child.take() {
                child.stop();
            }
        }
    }
}
#[async_trait::async_trait]
impl Host for NativeHost {
    async fn probe(&self) -> Result<String, Fault> {
        let profile = self.profile.clone();
        let storage = self.storage.clone();
        let marker = tauri::async_runtime::spawn_blocking(move || profile.local_marker(&storage))
            .await
            .map_err(|_| Fault::new(FaultKind::Unreadable, "The local sign-in check stopped."))?
            .map_err(|_| {
                Fault::new(
                    FaultKind::Unreadable,
                    "The local CLI sign-in could not be checked safely.",
                )
            })?;
        let mut child = self.child.lock().unwrap();
        if let Some(process) = child.as_mut() {
            match process.try_wait() {
                Ok(Some(exit)) => {
                    *child = None;
                    if !exit {
                        return Err(Fault::new(FaultKind::Unknown,"Sign-in closed before it completed. Try again; existing accounts were not changed."));
                    }
                    *self.finished.lock().unwrap() = true;
                }
                Ok(None) => {
                    return Err(Fault::new(
                        FaultKind::NotSignedIn,
                        "Finish signing in in the official CLI window.",
                    ))
                }
                Err(_) => {
                    return Err(Fault::new(
                        FaultKind::Unknown,
                        "The sign-in window could not be checked. Try again.",
                    ))
                }
            }
        }
        if *self.finished.lock().unwrap() {
            Ok(format!("complete:{}", marker.unwrap_or_default()))
        } else {
            marker.ok_or_else(|| Fault::new(FaultKind::NotSignedIn, "No completed sign-in yet."))
        }
    }
    async fn connect(&self) -> Result<(), Fault> {
        let next = self
            .engine
            .connect_cli(self.profile.clone())
            .await
            .map_err(|e| {
                e.downcast_ref::<Fault>()
                    .cloned()
                    .unwrap_or_else(|| Fault::new(FaultKind::Unknown, e.to_string()))
            })?;
        changed(&self.app, self.profile.provider, next);
        Ok(())
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        if !self.flow.current(self.run) {
            return Ok(());
        }
        self.profile.validate(&self.storage).map_err(|_| {
            Fault::new(
                FaultKind::Unreadable,
                "The sign-in location is unavailable.",
            )
        })?;
        let executable=crate::platform::find_sign_in_cli(self.profile.provider).ok_or_else(||Fault::new(FaultKind::ToolMissing,format!("{} CLI was not found. Install it from the official site, then try again. Nothing was installed automatically.",self.profile.provider.name())))?;
        let process = crate::login_process::LoginProcess::spawn(
            &executable,
            self.profile.provider,
            &self.profile.home,
        )
        .map_err(|_| {
            Fault::new(
                FaultKind::ToolMissing,
                "The official sign-in window could not be opened. Check your CLI installation.",
            )
        })?;
        *self.child.lock().unwrap() = Some(process);
        *self.finished.lock().unwrap() = false;
        Ok(())
    }
    fn publish(&self, status: Status) {
        publish(&self.app, self.profile.provider, status);
    }
}
pub fn start(
    flow: Arc<Flow>,
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    provider: CliProvider,
    consent: bool,
) -> Result<Status, String> {
    if !consent {
        return Err("Start sign-in explicitly to connect this account.".into());
    }
    if let Some(run) = flow.begin_named(provider.name()) {
        let status = flow.status();
        publish(&app, provider, status.clone());
        tauri::async_runtime::spawn(async move {
            let settings = engine.settings.borrow().clone();
            let saved = storage.clone();
            let profile = tauri::async_runtime::spawn_blocking(move || {
                Profile::prepare(&saved, provider, &settings)
            })
            .await;
            if !flow.current(run) {
                return;
            }
            let profile = match profile {
                Ok(Ok(profile)) => profile,
                _ => {
                    // A preparation failure must not leave an endless signing-in indicator.
                    let host = PreparationHost { app, provider };
                    cursor_login::failure(&flow,&host,run,Fault::new(FaultKind::Unreadable,"A safe sign-in location could not be prepared. Existing accounts were left untouched."));
                    return;
                }
            };
            let host = Arc::new(NativeHost {
                engine,
                storage,
                app,
                profile,
                child: Mutex::new(None),
                finished: Mutex::new(false),
                flow: flow.clone(),
                run,
            });
            let options = Options {
                name: provider.name(),
                waiting_message:
                    "Finish in the official sign-in window. We’ll connect automatically.",
                waiting_timeout: Duration::from_secs(600),
                ..Default::default()
            };
            cursor_login::run(flow, run, host, options).await;
        });
        return Ok(status);
    }
    Ok(flow.status())
}
struct PreparationHost {
    app: tauri::AppHandle,
    provider: CliProvider,
}
#[async_trait::async_trait]
impl Host for PreparationHost {
    async fn probe(&self) -> Result<String, Fault> {
        unreachable!()
    }
    async fn connect(&self) -> Result<(), Fault> {
        unreachable!()
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        unreachable!()
    }
    fn publish(&self, status: Status) {
        publish(&self.app, self.provider, status);
    }
}
