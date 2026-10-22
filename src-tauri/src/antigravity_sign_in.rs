//! Explicit owner-client recovery, separate from read-only existing-source discovery.
use opentoken_core::{
    connections::AntigravityClient,
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
struct NativeHost {
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    client: AntigravityClient,
    child: Mutex<Option<crate::login_process::LoginProcess>>,
    opened: AtomicBool,
    completed: AtomicBool,
}
#[async_trait::async_trait]
impl Host for NativeHost {
    async fn cooldown_seconds(&self) -> u64 {
        self.engine
            .verification_wait_secs(opentoken_core::model::Provider::Antigravity)
            .await
    }
    async fn probe(&self) -> Result<String, Fault> {
        if !self.opened.load(Ordering::SeqCst) {
            return Err(Fault::new(
                FaultKind::NotSignedIn,
                "Official client recovery was explicitly requested.",
            ));
        }
        {
            let mut owned = self.child.lock().unwrap();
            if let Some(child) = owned.as_mut() {
                match child.try_wait(){
            Ok(Some(true))=>{*owned=None;self.completed.store(true,Ordering::SeqCst);},
            Ok(Some(false))=>return Err(Fault::new(FaultKind::Unknown,"The official agy model-list check did not complete. Recover sign-in in the official client, then try again. No inference or hidden OAuth fallback was requested.")),
            Ok(None)=>return Err(Fault::new(FaultKind::NotSignedIn,"Finish the official agy client recovery window.")),
            Err(_)=>return Err(Fault::new(FaultKind::Unknown,"The owned agy recovery process could not be checked.")),
        }
            }
        }
        let marker = self
            .engine
            .antigravity_recovery_marker(self.client, true)
            .await?;
        Ok(format!(
            "{marker}:{}",
            self.completed.load(Ordering::SeqCst)
        ))
    }
    async fn connect(&self) -> Result<(), Fault> {
        self.engine.connect_antigravity_client(self.client,true).await.map(|_|()).map_err(|e|{
            let mut f=e.downcast_ref::<Fault>().cloned().unwrap_or_else(||Fault::new(FaultKind::Unknown,e.to_string()));
            if self.client==AntigravityClient::Cli&&self.completed.load(Ordering::SeqCst)&&f.kind==FaultKind::NeedsRelogin{f.kind=FaultKind::PermissionDenied;f.message="The official agy model check did not restore quota access. Finish sign-in in the official client; no alternative grant or Google registration was used.".into();}f
        })
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        let exe=crate::platform::antigravity_client_executable(&self.storage,self.client).ok_or_else(||Fault::new(FaultKind::ToolMissing,"The selected official Antigravity client was not found in a standard installation. Nothing was installed automatically."))?;
        if self.client == AntigravityClient::Cli {
            let home = self.storage.user_home.clone();
            let child = crate::login_process::LoginProcess::spawn_args(
                &exe,
                &["models"],
                "USERPROFILE",
                &home,
            )
            .map_err(|_| {
                Fault::new(
                    FaultKind::ToolMissing,
                    "The official agy recovery window could not open.",
                )
            })?;
            *self.child.lock().unwrap() = Some(child);
        } else {
            std::process::Command::new(exe).spawn().map_err(|_| {
                Fault::new(
                    FaultKind::ToolMissing,
                    "Windows could not open the selected Antigravity app. Nothing was installed.",
                )
            })?;
            // The vendor app remains open on cancel/disconnect; only its normal owner manages it.
        }
        self.opened.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn publish(&self, status: Status) {
        let _ = self.app.emit("antigravity-login-changed", status);
    }
}
pub fn start(
    flow: Arc<Flow>,
    engine: Engine,
    storage: Storage,
    app: tauri::AppHandle,
    client: AntigravityClient,
    consent: bool,
) -> Result<Status, String> {
    if !consent {
        return Err("Choose official Antigravity recovery explicitly. No client, login or quota request was started.".into());
    }
    if let Some(run) = flow.begin_named("Antigravity") {
        let status = flow.status();
        let _ = app.emit("antigravity-login-changed", status.clone());
        let message = if client == AntigravityClient::Cli {
            "The official agy models command may renew its own login or open its recovery UI. No model prompt is sent. We verify quota only after the owned command succeeds."
        } else {
            "Finish recovery in the selected official Antigravity app. Its own code manages authentication; OpenToken will verify that client's identity and quota automatically."
        };
        let host = Arc::new(NativeHost {
            engine,
            storage,
            app,
            client,
            child: Mutex::new(None),
            opened: AtomicBool::new(false),
            completed: AtomicBool::new(false),
        });
        tauri::async_runtime::spawn(cursor_login::run(
            flow,
            run,
            host,
            Options {
                name: "Antigravity",
                waiting_message: message,
                waiting_timeout: Duration::from_secs(600),
                ..Default::default()
            },
        ));
        return Ok(status);
    }
    Ok(flow.status())
}
