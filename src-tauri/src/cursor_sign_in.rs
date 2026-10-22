//! Thin Tauri host for the tested native sign-in coordinator.
use opentoken_core::{
    cursor_login::{self, Flow, Host, Status},
    engine::Engine,
    model::{Fault, FaultKind},
};
use std::sync::Arc;
use tauri::Emitter;
struct NativeHost {
    engine: Engine,
    app: tauri::AppHandle,
}
#[async_trait::async_trait]
impl Host for NativeHost {
    async fn probe(&self) -> Result<String, Fault> {
        self.engine.cursor_sign_in_marker(true).await
    }
    async fn connect(&self) -> Result<(), Fault> {
        self.engine
            .connect_cursor_personal(true)
            .await
            .map(|_| ())
            .map_err(|e| {
                e.downcast_ref::<Fault>()
                    .cloned()
                    .unwrap_or_else(|| Fault::new(FaultKind::Unknown, e.to_string()))
            })
    }
    async fn open_cursor(&self) -> Result<(), Fault> {
        tauri::async_runtime::spawn_blocking(crate::platform::open_cursor_sign_in)
            .await
            .map_err(|_| {
                Fault::new(
                    FaultKind::ToolMissing,
                    "Cursor could not be opened. Open it manually to sign in.",
                )
            })?
            .map_err(|error| Fault::new(FaultKind::ToolMissing, error.to_string()))
    }
    fn publish(&self, status: Status) {
        let _ = self.app.emit("cursor-login-changed", status);
    }
}
pub fn start(
    flow: Arc<Flow>,
    engine: Engine,
    app: tauri::AppHandle,
    consent: bool,
) -> Result<Status, String> {
    if !consent {
        return Err(
            "Sign in with Cursor requests read-only access; start it explicitly to continue."
                .into(),
        );
    }
    if let Some(run) = flow.begin() {
        let status = flow.status();
        let _ = app.emit("cursor-login-changed", status.clone());
        tauri::async_runtime::spawn(cursor_login::run(
            flow,
            run,
            Arc::new(NativeHost { engine, app }),
            Default::default(),
        ));
        return Ok(status);
    }
    Ok(flow.status())
}
