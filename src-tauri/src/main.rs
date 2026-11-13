#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod antigravity_sign_in;
mod cli_sign_in;
mod credentials;
mod cursor_sign_in;
mod flyout;
mod github_sign_in;
mod grok_sign_in;
mod icon;
mod logging;
mod login_process;
mod motion;
mod placement;
mod platform;
mod runtime_health;
mod tray_health;
use opentoken_core::{
    engine::Engine,
    model::{Snapshot, Status},
    storage::{Settings, Storage},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

use tauri_plugin_dialog::DialogExt;

struct Slot {
    icon: TrayIcon,
    key: (icon::Glyph, bool),
    health: tray_health::Health,
    tooltip: String,
}
struct AppState {
    engine: Engine,
    storage: Storage,
    trays: Mutex<HashMap<String, Slot>>,
    no_startup: bool,
    flyout: Arc<flyout::State>,
    motion: motion::State,
    cursor_login: Arc<opentoken_core::cursor_login::Flow>,
    cli_logins: cli_sign_in::Flows,
    github_login: Arc<opentoken_core::cursor_login::Flow>,
    grok_login: Arc<opentoken_core::cursor_login::Flow>,
    antigravity_login: Arc<opentoken_core::cursor_login::Flow>,
}
#[tauri::command]
fn get_state(state: tauri::State<'_, Arc<AppState>>) -> Snapshot {
    state.engine.state.borrow().clone()
}
#[tauri::command]
async fn set_history_source(
    provider: opentoken_core::model::Provider,
    enabled: bool,
    consent: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::monitoring::Outcome, String> {
    state
        .engine
        .set_history_source(provider, enabled, consent)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_sources(state: tauri::State<'_, Arc<AppState>>) -> opentoken_core::monitoring::Inventory {
    state.engine.sources.borrow().clone()
}
#[tauri::command]
async fn connect_existing_usage(
    provider: opentoken_core::model::Provider,
    consent: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::monitoring::Outcome, String> {
    state
        .engine
        .connect_existing(provider, consent)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn connect_antigravity_cli_usage(
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::monitoring::Outcome, String> {
    if !consent {
        return Err(
            "Approve Antigravity CLI-profile monitoring explicitly. No CLI was started.".into(),
        );
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Dashboard window is unavailable.")?;
    let hold = state.flyout.hold();
    let allowed = tauri::async_runtime::spawn_blocking(move || {
        platform::confirm_antigravity_cli_profile(&window)
    })
    .await;
    drop(hold);
    defer_dismiss(&app);
    if !allowed
        .map_err(|_| "CLI-profile confirmation stopped.")?
        .map_err(|_| "CLI-profile confirmation was unavailable.")?
    {
        return Err("CLI-profile monitoring cancelled. No CLI was started.".into());
    }
    state
        .engine
        .connect_antigravity_cli_usage(true)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn connect_muse_subscription(
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::monitoring::Outcome, String> {
    if !consent {
        return Err("Approve the Muse key endpoint explicitly. No request was made.".into());
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Dashboard window is unavailable.")?;
    let hold = state.flyout.hold();
    let allowed =
        tauri::async_runtime::spawn_blocking(move || platform::confirm_muse_key_endpoint(&window))
            .await;
    drop(hold);
    defer_dismiss(&app);
    if !allowed
        .map_err(|_| "Muse endpoint confirmation stopped.")?
        .map_err(|_| "Muse endpoint confirmation was unavailable.")?
    {
        return Err("Muse key endpoint access cancelled. No request was made.".into());
    }
    state
        .engine
        .connect_muse_subscription(true)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn cancel_existing_usage(
    provider: opentoken_core::model::Provider,
    state: tauri::State<'_, Arc<AppState>>,
) {
    state.engine.cancel_existing(provider);
}
#[tauri::command]
fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Settings {
    state.engine.settings.borrow().clone()
}
#[tauri::command]
async fn refresh(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    state.engine.refresh().await;
    Ok(())
}
#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    settings: Settings,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    settings.validate().map_err(|e| e.to_string())?;
    let previous = state.engine.settings.borrow().clone();
    let window = app.get_webview_window("main");
    let old_top = window
        .as_ref()
        .and_then(|w| w.is_always_on_top().ok())
        .unwrap_or(previous.dashboard_pinned);
    if let Some(w) = &window {
        w.set_always_on_top(settings.dashboard_pinned)
            .map_err(|e| e.to_string())?;
    }
    if !state.no_startup && settings.launch_at_startup != previous.launch_at_startup {
        let enabled = settings.launch_at_startup;
        let startup =
            tauri::async_runtime::spawn_blocking(move || platform::configure_startup(enabled))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| format!("Windows could not update startup: {e}")));
        if let Err(e) = startup {
            if let Some(w) = &window {
                let _ = w.set_always_on_top(old_top);
            }
            return Err(e);
        }
    }
    if let Err(e) = state.engine.save(settings).await {
        if let Some(w) = &window {
            let _ = w.set_always_on_top(old_top);
        }
        if !state.no_startup {
            let _ = platform::configure_startup(previous.launch_at_startup);
        }
        return Err(e.to_string());
    }
    let shared = state.inner().clone();
    let handle = app.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _activity = runtime_health::activity(runtime_health::Stage::Preferences);
        let snapshot = shared.engine.state.borrow().clone();
        update_trays(&handle, &snapshot, &shared);
        if let Some(window) = handle.get_webview_window("main") {
            let theme = shared.engine.settings.borrow().theme.clone();
            platform::apply_window_style(&window, &theme);
            if shared.motion.requested_visible() && window.is_visible().unwrap_or(false) {
                present(&handle, None, false);
            }
        }
        defer_dismiss(&handle);
        let _ = tx.send(());
    })
    .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "Dashboard preferences could not be applied.".to_string())?;
    Ok(state.engine.settings.borrow().clone())
}
#[tauri::command]
async fn connect_provider(
    input: opentoken_core::connections::ConnectInput,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    state.engine.connect(input).await.map_err(|e| e.to_string())
}
#[tauri::command]
fn get_cursor_login_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    state.cursor_login.status()
}
#[tauri::command]
fn start_cursor_sign_in(
    app: tauri::AppHandle,
    consent: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::cursor_login::Status, String> {
    cursor_sign_in::start(
        state.cursor_login.clone(),
        state.engine.clone(),
        app,
        consent,
    )
}
#[tauri::command]
fn cancel_cursor_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    let status = state.cursor_login.cancel();
    let _ = app.emit("cursor-login-changed", &status);
    status
}
#[tauri::command]
async fn connect_cursor_personal(
    consent: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    state
        .engine
        .connect_cursor_personal(consent)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn open_cursor_sign_in() -> Result<(), String> {
    platform::open_cursor_sign_in().map_err(|e| e.to_string())
}
#[tauri::command]
async fn disconnect_provider(
    app: tauri::AppHandle,
    id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    let personal =
        state.engine.settings.borrow().connections.iter().any(|c| {
            c.id == id && c.mode == opentoken_core::connections::ConnectionMode::CursorApp
        });
    let github =
        state.engine.settings.borrow().connections.iter().any(|c| {
            c.id == id && c.mode == opentoken_core::connections::ConnectionMode::GithubCli
        });
    let grok = state.engine.settings.borrow().connections.iter().any(|c| {
        c.id == id
            && matches!(
                c.mode,
                opentoken_core::connections::ConnectionMode::GrokCli
                    | opentoken_core::connections::ConnectionMode::GrokCliRenewal
            )
    });
    let antigravity = state
        .engine
        .settings
        .borrow()
        .connections
        .iter()
        .any(|c| c.id == id && c.provider == opentoken_core::model::Provider::Antigravity);
    if antigravity && state.antigravity_login.status().phase.active() {
        return Err("Finish or cancel Antigravity recovery before disconnecting.".into());
    }
    if grok && state.grok_login.status().phase.active() {
        return Err("Finish or cancel Grok sign-in before disconnecting.".into());
    }
    if github && state.github_login.status().phase.active() {
        return Err("Finish or cancel GitHub sign-in before disconnecting.".into());
    }
    let settings = state
        .engine
        .disconnect(id)
        .await
        .map_err(|e| e.to_string())?;
    if antigravity {
        let status = state.antigravity_login.reset();
        let _ = app.emit("antigravity-login-changed", status);
    }
    if grok {
        let status = state.grok_login.reset();
        let _ = app.emit("grok-login-changed", status);
    }
    if github {
        let status = state.github_login.reset();
        let _ = app.emit("github-login-changed", status);
    }
    if personal {
        let status = state.cursor_login.reset();
        let _ = app.emit("cursor-login-changed", status);
    }
    Ok(settings)
}
#[tauri::command]
fn open_provider_page(provider: String, docs: bool) -> Result<(), String> {
    platform::open_provider_page(&provider, docs).map_err(|e| e.to_string())
}
#[tauri::command]
async fn save_account_order(
    order: Vec<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .engine
        .save_account_order(order)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn set_dashboard_pinned(
    app: tauri::AppHandle,
    pinned: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Dashboard window is unavailable.")?;
    state.flyout.cancel();
    let previous = window.is_always_on_top().map_err(|e| e.to_string())?;
    window
        .set_always_on_top(pinned)
        .map_err(|e| e.to_string())?;
    if let Err(error) = state.engine.save_dashboard_pin(pinned).await {
        let _ = window.set_always_on_top(previous);
        defer_dismiss(&app);
        return Err(error.to_string());
    }
    defer_dismiss(&app);
    Ok(())
}
#[tauri::command]
async fn get_spending(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::spending::Spending, String> {
    let mut rx = state.engine.tracking.state.clone();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            {
                let snapshot = rx.borrow_and_update();
                if let Some(error) = &snapshot.error {
                    return Err(error.clone());
                }
                if let Some(data) = &snapshot.data {
                    return Ok(data.clone());
                }
            }
            rx.changed()
                .await
                .map_err(|_| "Local history worker is unavailable.".to_string())?;
        }
    })
    .await
    .map_err(|_| "Local history is still initializing.".to_string())?
}
#[tauri::command]
fn get_tracking(state: tauri::State<'_, Arc<AppState>>) -> opentoken_core::tracking::TrackingState {
    state.engine.tracking.state.borrow().clone()
}
fn defer_dismiss(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let shared = state.inner().clone();
    let epoch = shared.flyout.cancel();
    if shared.engine.settings.borrow().dashboard_pinned || shared.flyout.modal_count() > 0 {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let handle = app.clone();
            let state = shared.clone();
            if app
                .run_on_main_thread(move || {
                    let decision = if !state.flyout.current(epoch) {
                        flyout::Decision::Keep
                    } else if let Some(w) = handle.get_webview_window("main") {
                        flyout::decide(flyout::Context {
                            pinned: state.engine.settings.borrow().dashboard_pinned
                                || w.is_always_on_top().unwrap_or(true),
                            visible: w.is_visible().unwrap_or(false),
                            focused: w.is_focused().unwrap_or(false),
                            modal: state.flyout.modal_count() > 0,
                            tray_pressed: state.flyout.tray_pressed(),
                            foreground: platform::foreground(&w),
                        })
                    } else {
                        flyout::Decision::Keep
                    };
                    if decision == flyout::Decision::Dismiss {
                        hide_dashboard(handle);
                    }
                    let _ = tx.send(decision);
                })
                .is_err()
            {
                break;
            }
            if rx.await != Ok(flyout::Decision::Wait) {
                break;
            }
        }
    });
}
#[tauri::command]
async fn choose_login_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Dashboard window is unavailable.")?;
    let hold = state.flyout.hold();
    let start = state
        .engine
        .settings
        .borrow()
        .codex_homes
        .first()
        .map(|p| opentoken_core::storage::normalize(p, &state.storage.user_home))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| state.storage.user_home.clone());
    let picker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        picker_app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Choose an isolated CLI sign-in folder")
            .set_directory(start)
            .blocking_pick_folder()
    })
    .await;
    drop(hold);
    defer_dismiss(&app);
    result
        .map_err(|_| "Folder selection stopped unexpectedly.".to_string())?
        .map(|p| {
            p.into_path()
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|_| "Choose a local folder.".to_string())
        })
        .transpose()
}
fn visibility(app: &tauri::AppHandle, visible: bool) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.engine.tracking.set_visible(visible);
    }
    let _ = app.emit("dashboard-visibility", visible);
}
#[tauri::command]
fn hide_dashboard(app: tauri::AppHandle) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.flyout.cancel();
    }
    motion::hide(&app);
}
#[tauri::command]
fn show_dashboard(app: tauri::AppHandle) {
    show(&app, None);
}
#[tauri::command]
fn dashboard_ready(app: tauri::AppHandle) {
    motion::ready(&app);
}
#[tauri::command]
fn import_quotatray(state: tauri::State<'_, Arc<AppState>>) -> Result<Settings, String> {
    state.storage.import_quotatray().map_err(|e| e.to_string())
}
#[tauri::command]
fn get_grok_login_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    state.grok_login.status()
}
#[tauri::command]
fn start_grok_sign_in(
    consent: bool,
    recover: Option<bool>,
    device_auth: Option<bool>,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::cursor_login::Status, String> {
    grok_sign_in::start(
        state.grok_login.clone(),
        state.engine.clone(),
        state.storage.clone(),
        app,
        consent,
        recover.unwrap_or(false),
        device_auth.unwrap_or(false),
    )
}
#[tauri::command]
async fn connect_grok_cli_renewal(
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::monitoring::Outcome, String> {
    if !consent {
        return Err("Approve repeated Grok CLI renewal explicitly.".into());
    }
    if state.grok_login.status().phase.active() {
        return Err("Finish or cancel Grok sign-in before enabling CLI renewal.".into());
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Dashboard window is unavailable.")?;
    let hold = state.flyout.hold();
    let allowed =
        tauri::async_runtime::spawn_blocking(move || platform::confirm_grok_cli_renewal(&window))
            .await;
    drop(hold);
    defer_dismiss(&app);
    if !allowed
        .map_err(|_| "Grok renewal review stopped.")?
        .map_err(|_| "Grok renewal review is unavailable.")?
    {
        return Err(
            "Grok CLI renewal cancelled. No command was run and no permission was saved.".into(),
        );
    }
    if state.grok_login.status().phase.active() {
        return Err("Grok sign-in is active; no renewal was started.".into());
    }
    state
        .engine
        .connect_grok_cli_renewal(true)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn cancel_grok_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    let status = state.grok_login.cancel();
    let _ = app.emit("grok-login-changed", status.clone());
    status
}
#[tauri::command]
fn get_antigravity_login_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    state.antigravity_login.status()
}
#[tauri::command]
fn start_antigravity_sign_in(
    client: opentoken_core::connections::AntigravityClient,
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::cursor_login::Status, String> {
    antigravity_sign_in::start(
        state.antigravity_login.clone(),
        state.engine.clone(),
        state.storage.clone(),
        app,
        client,
        consent,
    )
}
#[tauri::command]
fn cancel_antigravity_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    let status = state.antigravity_login.cancel();
    let _ = app.emit("antigravity-login-changed", status.clone());
    status
}
#[tauri::command]
fn get_github_login_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    state.github_login.status()
}
#[tauri::command]
fn start_github_sign_in(
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::cursor_login::Status, String> {
    github_sign_in::start(
        state.github_login.clone(),
        state.engine.clone(),
        state.storage.clone(),
        app,
        consent,
    )
}
#[tauri::command]
fn cancel_github_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    let status = state.github_login.cancel();
    let _ = app.emit("github-login-changed", status.clone());
    status
}
#[tauri::command]
fn get_cli_login_status(
    provider: opentoken_core::cli_profiles::CliProvider,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    state.cli_logins.get(provider).status()
}
#[tauri::command]
fn start_cli_sign_in(
    provider: opentoken_core::cli_profiles::CliProvider,
    consent: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<opentoken_core::cursor_login::Status, String> {
    cli_sign_in::start(
        state.cli_logins.get(provider),
        state.engine.clone(),
        state.storage.clone(),
        app,
        provider,
        consent,
    )
}
#[tauri::command]
fn cancel_cli_sign_in(
    provider: opentoken_core::cli_profiles::CliProvider,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> opentoken_core::cursor_login::Status {
    let status = state.cli_logins.get(provider).cancel();
    cli_sign_in::publish(&app, provider, status.clone());
    status
}
#[tauri::command]
async fn disconnect_cli_account(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Settings, String> {
    let provider = state
        .engine
        .state
        .borrow()
        .accounts
        .iter()
        .find(|a| a.account.id == id)
        .map(|a| a.account.provider)
        .ok_or("This account is no longer monitored.")?;
    let provider = match provider {
        opentoken_core::model::Provider::Codex => opentoken_core::cli_profiles::CliProvider::Codex,
        opentoken_core::model::Provider::Claude => {
            opentoken_core::cli_profiles::CliProvider::Claude
        }
        _ => return Err("Choose a CLI account.".into()),
    };
    if state.cli_logins.get(provider).status().phase.active() {
        return Err("Finish or cancel this provider's sign-in before disconnecting.".into());
    }
    let next = state
        .engine
        .disconnect_cli(id)
        .await
        .map_err(|e| e.to_string())?;
    let status = state.cli_logins.get(provider).reset();
    cli_sign_in::publish(&app, provider, status);
    cli_sign_in::changed(&app, provider, next.clone());
    Ok(next)
}
#[tauri::command]
fn open_log(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    platform::open_log(&state.storage.dir.join("logs/opentoken.log")).map_err(|e| e.to_string())
}
#[tauri::command]
fn open_tray_settings() -> Result<(), String> {
    platform::open_tray_settings().map_err(|e| e.to_string())
}
#[tauri::command]
fn get_desktop_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> serde_json::Value {
    // Native getters/COM can dispatch nested Windows messages. Never hold the tray
    // mutex while calling them: a nested reconciliation needs that same mutex.
    let trays = {
        let slots = state.trays.lock().unwrap();
        serde_json::json!({
            "accountTrayIcons":slots.keys().filter(|id|id.as_str()!="opentoken" && !id.ends_with(":provider")).count(),
            "providerTrayIcons":slots.keys().filter(|id|id.ends_with(":provider")).count(),
            "nativeTrayIcons":slots.len(),
            "unconfirmedTrayIcons":slots.values().filter(|s|s.health.unconfirmed).count(),
            "trayRepairs":slots.values().map(|s|s.health.repairs).sum::<u64>(),
            "trayRemaining":slots.iter().filter_map(|(id,slot)| match slot.key.0 { icon::Glyph::Badge{primary,..} | icon::Glyph::Usage{primary,..} => Some((id.clone(),primary)), _=>None }).collect::<HashMap<_,_>>(),
        })
    };
    let window = app.get_webview_window("main");
    let mut result = serde_json::json!({
        "material":window.as_ref().map(platform::material_status),
        "visible":window.as_ref().and_then(|w|w.is_visible().ok()),
        "dashboardPinned":state.engine.settings.borrow().dashboard_pinned,
        "alwaysOnTop":window.as_ref().and_then(|w|w.is_always_on_top().ok()),
        "focused":window.as_ref().and_then(|w|w.is_focused().ok()),
        "modalInteractions":state.flyout.modal_count(),
        "trayGesture":state.flyout.tray_pressed(),
        "motion":state.motion.status(),
        "viewport":window.as_ref().and_then(platform::viewport_status),
        "placeholderRegistered":app.tray_by_id("opentoken").is_some(),
        "position":window.as_ref().and_then(|w|w.outer_position().ok()),
        "size":window.as_ref().and_then(|w|w.outer_size().ok()),
        "workArea":window.and_then(|w|w.primary_monitor().ok().flatten()).map(|m|*m.work_area()),
    });
    result
        .as_object_mut()
        .unwrap()
        .extend(trays.as_object().unwrap().clone());
    result
}
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn show(app: &tauri::AppHandle, anchor: Option<tauri::PhysicalPosition<f64>>) {
    present(app, anchor, true);
}
fn present(app: &tauri::AppHandle, anchor: Option<tauri::PhysicalPosition<f64>>, activate: bool) {
    if activate {
        if let Some(state) = app.try_state::<Arc<AppState>>() {
            state.flyout.cancel();
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let mode = app
            .try_state::<Arc<AppState>>()
            .map(|s| s.engine.settings.borrow().flyout_position.clone())
            .unwrap_or_else(|| "bottom_right".into());
        let mut destination = window
            .outer_position()
            .map(|p| (p.x, p.y))
            .unwrap_or((0, 0));
        let mut offset = (0, 0);
        let monitor = anchor
            .and_then(|p| window.monitor_from_point(p.x, p.y).ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        if let Some(monitor) = monitor {
            let work = monitor.work_area();
            let size = app
                .try_state::<Arc<AppState>>()
                .and_then(|s| s.motion.settled_size())
                .or_else(|| window.outer_size().ok())
                .unwrap_or(tauri::PhysicalSize::new(454, 760));
            let margin = (10.0 * monitor.scale_factor()).round() as i32;
            let (x, y) = placement::position(
                placement::WorkArea {
                    x: work.position.x,
                    y: work.position.y,
                    width: work.size.width,
                    height: work.size.height,
                },
                (size.width, size.height),
                &mode,
                anchor.map(|p| (p.x as i32, p.y as i32)),
                margin,
            );
            destination = (x, y);
            offset = motion::offset(
                placement::WorkArea {
                    x: work.position.x,
                    y: work.position.y,
                    width: work.size.width,
                    height: work.size.height,
                },
                (size.width, size.height),
                destination,
                monitor.scale_factor(),
                mode == "tray" && anchor.is_some_and(|p| p.y < y as f64),
            );
        }
        if activate {
            motion::show(app, destination, offset);
        } else {
            motion::reposition(app, destination, offset);
        }
    }
}
// Await the UI acknowledgement before taking another watch value. Slow shell calls
// cannot build an unbounded queue of obsolete snapshots, and the callback reads latest state.
async fn reconcile_trays(app: &tauri::AppHandle, state: &Arc<AppState>) -> bool {
    let handle = app.clone();
    let shared = state.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    if app
        .run_on_main_thread(move || {
            let snapshot = shared.engine.state.borrow().clone();
            update_trays(&handle, &snapshot, &shared);
            let _ = tx.send(());
        })
        .is_err()
    {
        return false;
    }
    rx.await.is_ok()
}
/// Where a tray reading came from, as shown in its tooltip.
fn provenance(source: opentoken_core::model::ReportSource) -> &'static str {
    use opentoken_core::model::ReportSource as S;
    match source {
        S::Provider => "Verified · ",
        S::CliLog => "CLI log · ",
        S::CursorDashboard => "Cursor app · Unofficial dashboard · ",
        S::CopilotInternal => "GitHub CLI · Unofficial quota · ",
        S::GrokCli | S::GrokCliRenewal => "Grok CLI · Private credit API · ",
        S::DevinCli => "Devin CLI · Unofficial quota · ",
        S::DevinApp => "Devin app · Unofficial quota · ",
        S::AntigravityCliUsage => {
            "Official agy /usage · Current profile · Identity not provided · "
        }
        S::AntigravityCli => "Antigravity CLI · Daily Cloud Code quota · ",
        S::AntigravityStored => "Antigravity IDE · Stored session · Daily quota · ",
        S::AntigravityLocal => "Antigravity app · Owned local quota · ",
        S::MuseCli => "Muse CLI · Last observation · ",
        S::MuseDevice => "Muse · Authorized key-issuing quota endpoint · ",
    }
}
fn update_trays(app: &tauri::AppHandle, snapshot: &Snapshot, state: &AppState) {
    let _activity = runtime_health::activity(runtime_health::Stage::Trays);
    let settings = state.engine.settings.borrow().clone();
    let mut desired: Vec<_> = snapshot
        .accounts
        .iter()
        .flat_map(|s| {
            let q = s.report.as_ref().and_then(|r| {
                if !matches!(
                    s.account.provider,
                    opentoken_core::model::Provider::Codex
                        | opentoken_core::model::Provider::Claude
                ) {
                    return icon::cursor_quota(r);
                }
                r.quotas
                    .iter()
                    .find(|q| q.id == settings.tray_metric)
                    .or_else(|| {
                        r.quotas.iter().find(|q| {
                            q.label
                                == if settings.tray_metric == "weekly" {
                                    "Weekly"
                                } else {
                                    "Session"
                                }
                        })
                    })
                    .or_else(|| r.quotas.iter().find(|q| q.percent.is_some()))
            });
            let percent = q.and_then(|q| q.percent).map(icon::remaining);
            let error = s.status == Status::Faulted;
            let stale = s.status == Status::Stale;
            let weekly = s
                .report
                .as_ref()
                .and_then(|r| r.quotas.iter().find(|q| q.label == "Weekly"))
                .and_then(|q| q.percent)
                .map(icon::remaining);
            let windows = s
                .report
                .as_ref()
                .map(|r| {
                    r.quotas
                        .iter()
                        .filter_map(|q| {
                            q.percent
                                .map(|p| format!(" · {}: {}% left", q.label, icon::remaining(p)))
                        })
                        .collect::<String>()
                })
                .unwrap_or_default();
            let provider = s.account.provider.name();
            let provenance = match &s.report {
                None => "Unavailable · ",
                Some(_) if stale => "Cached · ",
                Some(r) => provenance(r.source),
            };
            let primary_label = if s.account.provider == opentoken_core::model::Provider::Cursor
                && s.report.as_ref().is_some_and(|r| {
                    r.source == opentoken_core::model::ReportSource::CursorDashboard
                }) {
                q.and_then(|q| percent.map(|left| format!("{}: {}% left · ", q.label, left)))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let tooltip = format!(
                "OpenToken · {provider} · {primary_label}{provenance}{}{}{}{}",
                s.account.email.as_deref().unwrap_or(&s.account.label),
                if windows.is_empty() {
                    " · Percent-left not reported".to_string()
                } else {
                    windows
                },
                if stale { " (cached)" } else { "" },
                s.fault
                    .as_ref()
                    .map(|f| format!(" · {}", f.message))
                    .unwrap_or_default()
            );
            if settings.tray_style == "badge" {
                return vec![(
                    s.account.id.clone(),
                    icon::Glyph::Badge {
                        primary: percent,
                        error,
                        stale,
                    },
                    tooltip,
                )];
            }
            vec![
                (
                    format!("{}:provider", s.account.id),
                    icon::Glyph::Provider(s.account.provider),
                    tooltip.clone(),
                ),
                (
                    s.account.id.clone(),
                    icon::Glyph::Usage {
                        primary: percent,
                        secondary: weekly,
                        dual: settings.tray_metric == "auto",
                        error,
                        stale,
                    },
                    tooltip,
                ),
            ]
        })
        .collect();
    if desired.is_empty() {
        desired.push((
            "opentoken".into(),
            icon::Glyph::App,
            "OpenToken · Click to connect your accounts".into(),
        ));
    }
    // A synchronous Explorer/COM call can pump messages and reenter this function.
    // The outer pass already owns reconciliation; never wait on ourselves.
    let Ok(mut slots) = state.trays.try_lock() else {
        return;
    };
    let removed: Vec<_> = slots
        .keys()
        .filter(|id| !desired.iter().any(|d| &d.0 == *id))
        .cloned()
        .collect();
    for id in removed {
        if let Some(slot) = slots.remove(&id) {
            let _ = slot.icon.set_visible(false);
        }
        // Tauri owns a resource-table reference too: dropping our handle alone leaks the icon.
        let _ = app.remove_tray_by_id(&id);
    }
    let light_taskbar = platform::light_taskbar();
    for (id, glyph, tooltip) in desired {
        let key = (glyph, light_taskbar);
        let tooltip: String = tooltip.chars().take(120).collect();
        if let Some(slot) = slots.get_mut(&id) {
            // NIM_MODIFY fails when Explorer has lost the registration. An owned
            // cached TrayIcon alone does NOT establish presence in the shell.
            if !slot.health.should_probe(
                std::time::Instant::now(),
                slot.tooltip != tooltip || slot.key != key,
            ) {
                continue;
            }
            let mut confirmed = slot.icon.set_tooltip(Some(&tooltip)).is_ok();
            if !confirmed && slot.health.failed(std::time::Instant::now()) {
                if tray_health::restore(&slot.icon) && slot.icon.set_tooltip(Some(&tooltip)).is_ok()
                {
                    confirmed = true;
                    slot.health.confirmed(true);
                    tracing::info!("Restored an OpenToken tray registration");
                } else {
                    tracing::warn!("Windows tray registration unavailable; bounded retry pending");
                }
            } else if confirmed {
                slot.health.confirmed(false);
            }
            if confirmed {
                slot.tooltip.clone_from(&tooltip);
            }
            if confirmed
                && slot.key != key
                && slot
                    .icon
                    .set_icon(Some(icon::render(glyph, light_taskbar)))
                    .is_ok()
            {
                slot.key = key; // Never acknowledge a failed art update as applied.
            }
            continue;
        }
        let build = |guid: Option<u128>| -> tauri::Result<TrayIcon> {
            let menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "show", "Show usage", true, None::<&str>)?,
                    &MenuItem::with_id(app, "refresh", "Refresh now", true, None::<&str>)?,
                    &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
                    &MenuItem::with_id(app, "log", "Open log", true, None::<&str>)?,
                    &MenuItem::with_id(app, "quit", "Exit OpenToken", true, None::<&str>)?,
                ],
            )?;
            let builder = TrayIconBuilder::with_id(&id);
            let builder = match guid {
                Some(guid) => builder.guid(guid),
                None => builder,
            };
            builder
                .icon(icon::render(glyph, light_taskbar))
                .tooltip(&tooltip)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    let Some(state) = app.try_state::<Arc<AppState>>() else {
                        return;
                    };
                    if app.get_webview_window("main").is_none() {
                        return;
                    }
                    let id: &str = tray.id().as_ref();
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Down,
                            ..
                        }
                        | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } => {
                            // Windows substitutes DoubleClick for the second mouse-down.
                            // Blur may already have started closing: that panel still counts as open.
                            state
                                .flyout
                                .tray_down(id, state.motion.presented_for_tray());
                            defer_dismiss(app);
                        }
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            position,
                            ..
                        } => {
                            let Some(was_presented) = state.flyout.tray_up(id) else {
                                return;
                            };
                            if state.flyout.modal_count() > 0 {
                                return;
                            }
                            if was_presented {
                                hide_dashboard(app.clone());
                            } else {
                                show(app, Some(position));
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)
        };
        let guid = std::env::current_exe()
            .ok()
            .map(|exe| tray_health::guid(&exe, &id));
        let built = build(guid).and_then(|icon| {
            // While Explorer is still starting, every registration fails; that is the
            // TaskbarCreated retry path's job, not evidence the GUID was refused.
            if guid.is_none()
                || icon.set_tooltip(Some(&tooltip)).is_ok()
                || !tray_health::shell_ready()
            {
                return Ok(icon);
            }
            // Explorer refuses a GUID it has bound to another binary path. A stable
            // identity is worth nothing if the icon is missing, so fall back to the
            // per-run identity rather than leaving the account without a tray icon.
            tracing::warn!("Windows refused a stable tray identity; using a per-run one");
            drop(icon);
            let _ = app.remove_tray_by_id(&id);
            build(None)
        });
        match built {
            Ok(icon) => {
                let confirmed = icon.set_tooltip(Some(&tooltip)).is_ok();
                let mut health = tray_health::Health::default();
                health.unconfirmed = !confirmed;
                health.should_probe(std::time::Instant::now(), true);
                slots.insert(
                    id,
                    Slot {
                        icon,
                        key,
                        health,
                        tooltip,
                    },
                );
            }
            Err(_) => tracing::warn!("Windows could not register a usage tray icon"),
        }
    }
}
fn main() {
    logging::install_panic_hook();
    if let Err(e) = run() {
        logging::record_failure(&format!("OpenToken could not start: {e}"));
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|s| s == "--remove-startup")
        && !args.iter().any(|s| s == "--diagnose-existing")
    {
        return platform::configure_startup(false);
    }
    let mut storage = Storage::system();
    // Explicit isolated fixtures get an isolated discovery home as well as app data.
    if args.iter().any(|s| s == "--no-startup")
        && std::env::var_os("OPENTOKEN_DATA_DIR").is_some()
        && storage.dir.is_absolute()
    {
        if let Some(home) = std::env::var_os("OPENTOKEN_SOURCE_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            storage.user_home = home;
        }
    }
    // Deliberate native diagnosis runs before logging, settings, WebView, startup or
    // single-instance initialization. No production connection/settings are written.
    if let Some(i) = args.iter().position(|s| s == "--diagnose-existing") {
        let provider = args.get(i + 1).and_then(|s| {
            serde_json::from_value::<opentoken_core::model::Provider>(serde_json::Value::String(
                s.clone(),
            ))
            .ok()
        });
        let consent = args.iter().any(|s| s == "--allow-existing-source-read");
        if let Some(provider) = provider {
            if args.iter().any(|s| s == "--official-cli-usage") {
                let runtime = tokio::runtime::Runtime::new()?;
                let result = runtime.block_on(async {
                    let service = opentoken_core::connections::Service::new(
                        storage,
                        Arc::new(opentoken_core::connections::UnavailableStore),
                    );
                    if provider != opentoken_core::model::Provider::Antigravity {
                        return Err(opentoken_core::model::Fault::new(
                            opentoken_core::model::FaultKind::PermissionDenied,
                            "This diagnostic supports only Antigravity.",
                        ));
                    }
                    service
                        .inspect_antigravity_cli_usage(
                            consent,
                            args.iter().any(|s| s == "--allow-official-client-recovery"),
                        )
                        .await
                });
                let output = match result {
                    Ok(probe) => {
                        serde_json::json!({"ok":true,"source":"official_cli_usage","probe":probe,"persisted":false})
                    }
                    Err(fault) => {
                        serde_json::json!({"ok":false,"source":"official_cli_usage","fault":fault,"persisted":false})
                    }
                };
                println!("{}", serde_json::to_string(&output)?);
                return Ok(());
            }
            if args.iter().any(|s| s == "--inspect-cli-namespace") {
                let service = opentoken_core::connections::Service::new(
                    storage,
                    Arc::new(opentoken_core::connections::UnavailableStore),
                );
                let result = if provider == opentoken_core::model::Provider::Antigravity {
                    service.inspect_antigravity_cli_namespace(consent)
                } else {
                    Err(opentoken_core::model::Fault::new(
                        opentoken_core::model::FaultKind::PermissionDenied,
                        "This diagnostic supports only Antigravity.",
                    ))
                };
                let output = match result {
                    Ok(present) => {
                        serde_json::json!({"ok":true,"namespace":"gemini:antigravity-cli","present":present,"persisted":false,"networkRequests":0})
                    }
                    Err(fault) => {
                        serde_json::json!({"ok":false,"fault":fault,"persisted":false,"networkRequests":0})
                    }
                };
                println!("{}", serde_json::to_string(&output)?);
                return Ok(());
            }
            let runtime = tokio::runtime::Runtime::new()?;
            let result = runtime.block_on(async {
                let service = opentoken_core::connections::Service::new(
                    storage,
                    Arc::new(opentoken_core::connections::UnavailableStore),
                );
                service.inspect_existing_source(provider, consent).await
            });
            let output = match result {
                Ok(report) => serde_json::json!({"ok":true,"probe":report,"persisted":false}),
                Err(fault) => {
                    serde_json::json!({"ok":false,"provider":provider,"fault":fault,"persisted":false})
                }
            };
            println!("{}", serde_json::to_string(&output)?);
        } else {
            println!("{{\"ok\":false,\"error\":\"Select a supported provider and explicitly allow existing-source reading.\",\"persisted\":false}}");
        }
        return Ok(());
    }
    std::fs::create_dir_all(storage.dir.join("logs"))?;
    logging::set_failure_log(storage.dir.join("logs/failures.log"));
    let log = logging::RotatingLog::new(storage.dir.join("logs/opentoken.log"))?;
    let (writer, _guard) = tracing_appender::non_blocking(log);
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(writer)
        .init();
    let settings = storage.load_settings()?;
    if args.iter().any(|s| s == "--doctor") {
        println!(
            "OpenToken {}\nData: {}\nCodex CLI: {}\nAPI: 127.0.0.1:{}\nStartup requested: {}",
            env!("CARGO_PKG_VERSION"),
            storage.dir.display(),
            opentoken_core::codex::find_cli()
                .map(|p| p.display().to_string())
                .unwrap_or("not found".into()),
            settings.api_port,
            settings.launch_at_startup
        );
        return Ok(());
    }
    let no_startup = args.iter().any(|s| s == "--no-startup") || cfg!(debug_assertions);
    let background = args.iter().any(|s| s == "--background");
    let mut context = tauri::generate_context!();
    // Isolated --no-startup fixtures must never forward quit/show into the live
    // user instance. Keep the normal application's single-instance identity.
    if args.iter().any(|s| s == "--no-startup")
        && std::env::var_os("OPENTOKEN_DATA_DIR").is_some()
        && storage.dir.is_absolute()
    {
        context.config_mut().identifier = format!(
            "{}.isolated.{}",
            context.config().identifier,
            opentoken_core::model::hash(&opentoken_core::storage::path_key(&storage.dir))
        );
    }
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_single_instance::init(|app,args,_cwd|{
        if args.iter().any(|s|s=="quit"){app.exit(0);return;}
        if args.iter().any(|s|s=="refresh"){let state=app.state::<Arc<AppState>>();let engine=state.engine.clone();tauri::async_runtime::spawn(async move{engine.refresh().await;});return;}
        if args.iter().any(|s|s=="settings"){let _=app.emit("navigate","settings");}show(app,None);
    })).invoke_handler(tauri::generate_handler![get_state,get_settings,get_sources,connect_existing_usage,connect_antigravity_cli_usage,connect_muse_subscription,cancel_existing_usage,set_history_source,refresh,save_settings,connect_provider,get_cursor_login_status,start_cursor_sign_in,cancel_cursor_sign_in,connect_cursor_personal,open_cursor_sign_in,disconnect_provider,open_provider_page,save_account_order,set_dashboard_pinned,get_spending,get_tracking,hide_dashboard,show_dashboard,dashboard_ready,choose_login_folder,import_quotatray,get_cli_login_status,start_cli_sign_in,cancel_cli_sign_in,disconnect_cli_account,get_github_login_status,start_github_sign_in,cancel_github_sign_in,get_grok_login_status,start_grok_sign_in,cancel_grok_sign_in,connect_grok_cli_renewal,get_antigravity_login_status,start_antigravity_sign_in,cancel_antigravity_sign_in,open_log,open_tray_settings,get_desktop_status,quit])
    .on_menu_event(|app,event|match event.id().as_ref(){
        "show"=>show(app,None),"settings"=>{let _=app.emit("navigate","settings");show(app,None);},
        "refresh"=>{let e=app.state::<Arc<AppState>>().engine.clone();tauri::async_runtime::spawn(async move{e.refresh().await;});},
        "log"=>{let _=platform::open_log(&app.state::<Arc<AppState>>().storage.dir.join("logs/opentoken.log"));},"quit"=>app.exit(0),_=>{}
    }).on_window_event(|window,event|{if window.label()!="main"{return;}match event{tauri::WindowEvent::CloseRequested{api,..}=>{api.prevent_close();hide_dashboard(window.app_handle().clone());},tauri::WindowEvent::Focused(true)=>{if let Some(state)=window.app_handle().try_state::<Arc<AppState>>(){state.flyout.cancel();}},tauri::WindowEvent::Focused(false)=>defer_dismiss(window.app_handle()),_=>{}}})
    .setup(move|app|{
        if args.iter().any(|s|s=="quit") {app.handle().exit(0);return Ok(());}
        if let Some(window)=app.get_webview_window("main"){platform::apply_window_style(&window,&settings.theme);window.set_always_on_top(settings.dashboard_pinned)?;}
        // Start inside Tauri's Tokio runtime; no nested runtime or UI-thread network work.
        let engine=tauri::async_runtime::block_on(async{Engine::start_with_credentials(storage.clone(),settings.clone(),Arc::new(credentials::Vault::new(&storage)))});
        let state=Arc::new(AppState{engine:engine.clone(),storage,trays:Mutex::new(HashMap::new()),no_startup,flyout:Arc::new(flyout::State::default()),motion:motion::State::default(),cursor_login:Arc::new(opentoken_core::cursor_login::Flow::default()),cli_logins:Default::default(),github_login:Default::default(),grok_login:Default::default(),antigravity_login:Default::default()});app.manage(state.clone());
        tracing::info!(pid=std::process::id(), background, "Desktop starting");
        let monitor=runtime_health::Monitor::default();monitor.start(app.handle().clone(),state.flyout.clone());app.manage(monitor);
        update_trays(app.handle(),&Snapshot::default(),&state);
        let handle=app.handle().clone();let observer=state.clone();let mut rx=engine.state.clone();
        tauri::async_runtime::spawn(async move{loop{let snapshot=rx.borrow_and_update().clone();let _=handle.emit("state-changed",&snapshot);if !reconcile_trays(&handle,&observer).await || rx.changed().await.is_err(){break;}}});
        let tray_handle=app.handle().clone();let tray_state=state.clone();
        tauri::async_runtime::spawn(async move {
            let mut tick=tokio::time::interval(std::time::Duration::from_secs(15));tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);tick.tick().await;
            loop {tick.tick().await;if !reconcile_trays(&tray_handle,&tray_state).await{break;}}
        });
        let source_handle=app.handle().clone();let mut source_rx=engine.sources.clone();
        tauri::async_runtime::spawn(async move{loop{let value=source_rx.borrow_and_update().clone();let _=source_handle.emit("sources-changed",&value);if source_rx.changed().await.is_err(){break;}}});
        let spending_handle=app.handle().clone();let mut spending_rx=engine.tracking.state.clone();
        tauri::async_runtime::spawn(async move{loop{let snapshot=spending_rx.borrow_and_update().clone();let _=spending_handle.emit("spending-changed",&snapshot);if spending_rx.changed().await.is_err(){break;}}});
        let api_engine=engine.clone();let connections_handle=app.handle().clone();
        tauri::async_runtime::spawn(async move{
            let mut settings_rx=api_engine.settings.clone();
            let mut running:Option<(tokio::sync::watch::Sender<bool>,tauri::async_runtime::JoinHandle<()>)>=None;
            let mut active=None;let mut last_connections=None;
            loop {
                let s=settings_rx.borrow_and_update().clone();
                if last_connections.as_ref()!=Some(&s.connections) {let _=connections_handle.emit("connections-changed",&s.connections);last_connections=Some(s.connections.clone());}
                let desired=(s.api_enabled,s.api_port);
                if active!=Some(desired) {
                    if let Some((stop,task))=running.take(){let _=stop.send(true);let _=task.await;}
                    let _=api_engine.commands.send(opentoken_core::engine::Command::ApiStatus(None)).await;
                    if s.api_enabled {let (stop,rx)=tokio::sync::watch::channel(false);let e=api_engine.clone();let task=tauri::async_runtime::spawn(async move{
                        if opentoken_core::api::serve(e.clone(),rx).await.is_err(){tracing::warn!("Local API could not bind; another app may own the port.");let _=e.commands.send(opentoken_core::engine::Command::ApiStatus(Some("Local API unavailable. Check whether another app is using the configured port.".into()))).await;}
                    });running=Some((stop,task));}active=Some(desired);
                }
                if settings_rx.changed().await.is_err(){break;}
            }
        });
        if !background{show(app.handle(),None);}
        if !no_startup&&settings.launch_at_startup {tauri::async_runtime::spawn_blocking(||{if platform::configure_startup(true).is_err(){tracing::warn!("Windows startup registration failed");}});}
        Ok(())
    }).build(context)?.run(|app,event|if let tauri::RunEvent::ExitRequested{code,..}=event{
        if let Some(monitor)=app.try_state::<runtime_health::Monitor>(){monitor.stop();}
        tracing::info!(?code,"Desktop exit requested");
        if let Some(state)=app.try_state::<Arc<AppState>>() {state.cursor_login.reset();state.cli_logins.reset();state.github_login.reset();state.grok_login.reset();state.antigravity_login.reset();let engine=state.engine.clone();tauri::async_runtime::block_on(async{if tokio::time::timeout(std::time::Duration::from_secs(10),engine.shutdown()).await.is_err(){logging::record_failure("Intentional exit: engine shutdown exceeded 10 seconds; allowing exit without a recovery restart");}});}
    });
    tracing::info!("Desktop stopped");
    Ok(())
}
