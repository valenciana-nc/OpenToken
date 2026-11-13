use anyhow::Result;
use std::path::Path;

pub fn find_sign_in_cli(
    provider: opentoken_core::cli_profiles::CliProvider,
) -> Option<std::path::PathBuf> {
    use opentoken_core::cli_profiles::CliProvider;
    if provider == CliProvider::Codex {
        return opentoken_core::codex::find_cli();
    }
    let name = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    let mut paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(name))
        .collect();
    paths.push(
        opentoken_core::storage::Storage::system()
            .user_home
            .join(".local/bin")
            .join(name),
    );
    paths.into_iter().find(|p| {
        p.is_absolute()
            && std::fs::symlink_metadata(p)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    })
}
/// A separate, default-No native consent boundary for Meta's credential-issuing endpoint.
pub fn confirm_antigravity_cli_profile(window: &tauri::WebviewWindow) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::{
            core::w,
            Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{
                    MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNOCANCEL,
                },
            },
        };
        let hwnd = window.hwnd()?;
        let answer = unsafe {
            MessageBoxW(Some(HWND(hwnd.0)),w!("Monitor the installed agy CLI's CURRENT PROFILE, not a verified human account. Switching CLI accounts changes the observed profile; OpenToken will not attribute local history or a selected account to these readings.\n\nBackground refresh runs only agy --print /usage --output-format json until disconnected. This is a CLI-handled quota command, not a model prompt. The official CLI may renew its own saved login. No borrowed OAuth registration, credential copy, installer or hidden login fallback is used.\n\nOld CLI-profile reports are not reused after restart or a failed read. Provider-reported meters can still be incomplete/default observations, not independent subscription verification. A request or owner-side credential update already made cannot be undone by cancellation.\n\nAllow CLI-profile monitoring?"),w!("OpenToken · Antigravity CLI profile"),MB_YESNOCANCEL|MB_DEFBUTTON2|MB_ICONWARNING)
        };
        Ok(answer == IDYES)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(false)
    }
}
pub fn confirm_grok_cli_renewal(window: &tauri::WebviewWindow) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::{
            core::w,
            Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{
                    MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNOCANCEL,
                },
            },
        };
        let hwnd = window.hwnd()?;
        let answer = unsafe {
            MessageBoxW(Some(HWND(hwnd.0)), w!("Allow the installed official Grok CLI to maintain the CURRENT saved Grok account when xAI rejects its access token?\n\nOpenToken will run only grok --version and grok models in the saved profile, at most once per 15 minutes and only after an authentication rejection. The CLI may renew or replace its own saved credentials and may request authentication in its own browser. This is not read-only access. No model prompt or extra grok login command is sent.\n\nIdentity and quota are checked again before anything is shown. A different CLI account is never silently substituted. Provider pauses remain enforced; unknown CLI versions/custom authentication fail closed.\n\nPermission continues after restart until this monitoring connection is disconnected or replaced using Use existing Grok. Backups do not restore it. Cancelling cannot undo an owner-side credential update already completed.\n\nAllow Grok CLI session renewal?"), w!("OpenToken · Grok CLI renewal"), MB_YESNOCANCEL | MB_DEFBUTTON2 | MB_ICONWARNING)
        };
        Ok(answer == IDYES)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(false)
    }
}
pub fn confirm_muse_key_endpoint(window: &tauri::WebviewWindow) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::{
            core::w,
            Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{
                    MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNOCANCEL,
                },
            },
        };
        let hwnd = window.hwnd()?;
        let answer = unsafe {
            MessageBoxW(Some(HWND(hwnd.0)),w!("This is NOT read-only. Meta's subscription endpoint may issue or rotate a Muse CLI inference key. Replacement effects are not documented and could affect running Muse sessions.\n\nBackground monitoring will repeat this endpoint until you disconnect. OpenToken discards returned inference keys and never writes Muse's credential file. No prompt or payment operation is requested.\n\nAllow this endpoint for this connection?"),w!("OpenToken · Muse quota access"),MB_YESNOCANCEL|MB_DEFBUTTON2|MB_ICONWARNING)
        };
        Ok(answer == IDYES)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(false)
    }
}
pub fn open_log(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("notepad.exe")
            .arg(path)
            .spawn()?;
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
    Ok(())
}
pub fn open_cursor_sign_in() -> Result<()> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(std::path::PathBuf::from(local).join("Programs/Cursor"));
        }
        for key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(key) {
                roots.push(std::path::PathBuf::from(root).join("Cursor"));
            }
        }
        for root in roots {
            let exe = root.join("Cursor.exe");
            if exe.is_absolute()
                && std::fs::symlink_metadata(&exe)
                    .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            {
                std::process::Command::new(exe).spawn().map_err(|_| {
                    anyhow::anyhow!("Windows could not open Cursor. Open it manually and sign in.")
                })?;
                return Ok(());
            }
        }
    }
    anyhow::bail!("Cursor was not found in a standard install location. Open your Cursor app manually to sign in, or install it from cursor.com/downloads. Nothing was installed automatically.")
}
pub fn antigravity_client_executable(
    storage: &opentoken_core::storage::Storage,
    client: opentoken_core::connections::AntigravityClient,
) -> Option<std::path::PathBuf> {
    use opentoken_core::connections::AntigravityClient;
    let mut local = vec![storage.user_home.join("AppData/Local")];
    let mut program = vec![];
    if storage.user_home == opentoken_core::storage::Storage::system().user_home {
        if let Some(p) = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            local.push(p);
        }
        for name in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = std::env::var_os(name)
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                program.push(p);
            }
        }
    }
    let paths: Vec<_> = if client == AntigravityClient::Cli {
        local
            .into_iter()
            .map(|p| p.join("agy/bin/agy.exe"))
            .collect()
    } else {
        let name = if client == AntigravityClient::Ide {
            "Antigravity IDE"
        } else {
            "Antigravity"
        };
        local
            .into_iter()
            .map(|p| p.join("Programs"))
            .chain(program)
            .map(|p| p.join(name).join(format!("{name}.exe")))
            .collect()
    };
    paths.into_iter().find(|p| {
        p.is_absolute()
            && p.ancestors().all(|a| {
                std::fs::symlink_metadata(a).is_ok_and(|m| {
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        if m.file_attributes() & 0x400 != 0 {
                            return false;
                        }
                    }
                    !m.file_type().is_symlink() && if a == p { m.is_file() } else { m.is_dir() }
                })
            })
    })
}
pub fn provider_page(provider: &str, docs: bool) -> Result<&'static str> {
    Ok(match (provider,docs) {
        ("codex",true) => "https://developers.openai.com/codex/cli/reference/",
        ("codex",false) => "https://developers.openai.com/codex/cli/",
        ("claude",true) => "https://code.claude.com/docs/en/authentication",
        ("claude",false) => "https://code.claude.com/docs/en/setup",
        ("cursor_personal",true) => "https://cursor.com/docs",
        ("cursor_personal",false) => "https://cursor.com/downloads",
        ("cursor",true) => "https://cursor.com/docs/account/teams/admin-api",
        ("cursor",false) => "https://cursor.com/dashboard/api",
        ("grok",false) => "https://grok.com/build",
        ("grok",true) => "https://docs.x.ai/docs/grok-code",
        ("github_cli",false) => "https://cli.github.com/",
        ("github_cli",true) => "https://cli.github.com/manual/gh_auth_login",
        ("copilot",true) => "https://docs.github.com/en/rest/billing/usage#get-billing-premium-request-usage-report-for-a-user",
        ("deepseek",false) => "https://platform.deepseek.com/api_keys",
        ("deepseek",true) => "https://api-docs.deepseek.com/api/get-user-balance",
        ("zai",false) => "https://z.ai/manage-apikey/apikey-list",
        ("zai",true) => "https://docs.z.ai/devpack/overview",
        ("zai_cn",false) => "https://bigmodel.cn/usercenter/proj-mgmt/apikeys",
        ("zai_cn",true) => "https://docs.bigmodel.cn/",
        ("kimi",false) => "https://www.kimi.com/code/console",
        ("kimi",true) => "https://www.kimi.com/code/docs/en/",
        ("kimi_international",false) => "https://www.kimi.ai/code/console",
        ("kimi_international",true) => "https://www.kimi.ai/code/docs/en/",
        ("opencode",false) => "https://opencode.ai/console",
        ("opencode",true) => "https://opencode.ai/docs/go/",
        ("copilot",false) => "https://github.com/settings/personal-access-tokens/new",
        _ => anyhow::bail!("No supported provider page was selected."),
    })
}
pub fn open_provider_page(provider: &str, docs: bool) -> Result<()> {
    let url = provider_page(provider, docs)?;
    #[cfg(windows)]
    {
        use windows::{
            core::{w, PCWSTR},
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };
        let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        if unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        }
        .0 as isize
            <= 32
        {
            anyhow::bail!("Windows could not open the provider page in your browser.");
        }
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        anyhow::bail!("Opening provider pages requires Windows.");
    }
    #[allow(unreachable_code)]
    Ok(())
}
pub fn open_tray_settings() -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:taskbar")
            .spawn()?;
    }
    Ok(())
}

/// Notification icons follow the Windows taskbar, not the app's selected theme.
pub fn light_taskbar() -> bool {
    #[cfg(windows)]
    {
        use windows::{core::w, Win32::System::Registry::*};
        let mut value = 1u32;
        let mut size = 4u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(std::ptr::from_mut(&mut value).cast()),
                Some(&mut size),
            )
        };
        result.is_err() || value != 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Classify real native focus; DOM focus changes alone must not dismiss the flyout.
pub fn foreground(window: &tauri::WebviewWindow) -> crate::flyout::Foreground {
    #[cfg(windows)]
    if let Ok(main) = window.hwnd() {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, GetForegroundWindow, GetWindow, GetWindowThreadProcessId, GA_ROOT,
            GW_OWNER,
        };
        let main = windows::Win32::Foundation::HWND(main.0);
        // SAFETY: read-only HWND queries; owner traversal is bounded and no handles are owned.
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground == main || GetAncestor(foreground, GA_ROOT) == main {
                return crate::flyout::Foreground::Main;
            }
            let mut process = 0;
            GetWindowThreadProcessId(foreground, Some(&mut process));
            if process == std::process::id() {
                return crate::flyout::Foreground::OwnedPopup;
            }
            let mut owner = foreground;
            for _ in 0..16 {
                let Ok(next) = GetWindow(owner, GW_OWNER) else {
                    break;
                };
                if next == main {
                    return crate::flyout::Foreground::OwnedPopup;
                }
                if next == owner || next.0.is_null() {
                    break;
                }
                owner = next;
            }
        }
    }
    let _ = window;
    crate::flyout::Foreground::Other
}

/// Capture on the UI thread; the watchdog must never ask Tauri for a synchronous getter.
pub fn current_thread_id() -> u32 {
    #[cfg(windows)]
    {
        unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}
/// Cross-thread read-only native query, including when a menu's nested loop defers Tao tasks.
pub fn native_interaction_active(thread: u32) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            GetGUIThreadInfo(thread, &mut info).is_ok()
                && (info.flags
                    & (GUI_INMENUMODE | GUI_INMOVESIZE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE))
                    .0
                    != 0
        }
    }
    #[cfg(not(windows))]
    {
        let _ = thread;
        false
    }
}

/// Read Windows Accessibility > Animation effects for every transition; never change it.
pub fn animations_enabled() -> bool {
    #[cfg(windows)]
    {
        use windows::{core::BOOL, Win32::UI::WindowsAndMessaging::*};
        let mut enabled = BOOL(0);
        // Fail closed: avoid motion if the accessibility preference cannot be queried.
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(std::ptr::from_mut(&mut enabled).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok()
                && enabled.as_bool()
                && GetSystemMetrics(SM_REMOTESESSION) == 0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
/// Called ONLY by the short-lived animation worker, never the UI thread.
/// DwmFlush waits for the compositor presentation boundary without changing global timer policy.
pub fn wait_for_compositor() -> bool {
    #[cfg(windows)]
    {
        unsafe { windows::Win32::Graphics::Dwm::DwmFlush().is_ok() }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
/// Temporarily compose the entire HWND at uniform alpha, including the glass and WebView.
/// Unlike legacy AnimateWindow, this does not require WebView2 to support WM_PRINTCLIENT.
pub fn begin_opacity(window: &tauri::WebviewWindow) -> Option<bool> {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let layered = style & WS_EX_LAYERED.0 as isize != 0;
            if !layered {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            }
            if SetLayeredWindowAttributes(
                hwnd,
                windows::Win32::Foundation::COLORREF(0),
                255,
                LWA_ALPHA,
            )
            .is_ok()
            {
                return Some(layered);
            }
            if !layered {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
            }
        }
    }
    let _ = window;
    None
}
pub fn set_opacity(window: &tauri::WebviewWindow, alpha: u8) {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
        unsafe {
            // A simultaneous pin/theme change can rebuild extended styles. Preserve all its bits.
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if style & WS_EX_LAYERED.0 as isize == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            }
            let _ = SetLayeredWindowAttributes(
                hwnd,
                windows::Win32::Foundation::COLORREF(0),
                alpha,
                LWA_ALPHA,
            );
        }
    }
    let _ = (window, alpha);
}
pub fn restore_opacity(window: &tauri::WebviewWindow, was_layered: bool) {
    set_opacity(window, 255);
    #[cfg(windows)]
    if !was_layered {
        if let Ok(hwnd) = window.hwnd() {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_LAYERED.0 as isize));
            }
        }
    }
    let _ = was_layered;
}

#[cfg(windows)]
const REVEAL_SUBCLASS: usize = 0x4f54524c;
#[cfg(windows)]
fn reveal_ex_style(style: u32, preserve_opacity: bool) -> u32 {
    if preserve_opacity {
        style | windows::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED.0
    } else {
        style
    }
}
#[cfg(windows)]
unsafe extern "system" fn reveal_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::{
        Foundation::LRESULT,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    // Keep WebView2's full-size viewport while the actual acrylic host rolls up/down.
    // Otherwise Wry's WM_SIZE handler would reflow the contents on every animation frame.
    if message == WM_SIZE && wparam.0 != SIZE_MINIMIZED as usize {
        return LRESULT(0);
    }
    if message == WM_STYLECHANGING && data & 1 != 0 && wparam.0 as i32 == GWL_EXSTYLE.0 {
        let result = DefSubclassProc(hwnd, message, wparam, lparam);
        // Tao rebuilds EXSTYLE on show/pin changes and otherwise removes WS_EX_LAYERED,
        // making an alpha-zero window flash opaque until a later frame reapplies alpha.
        // Keep only that one bit while the existing native opacity lease is active.
        // SAFETY: Windows supplies this mutable STYLESTRUCT for this owned-window message.
        if let Some(style) = (lparam.0 as *mut STYLESTRUCT).as_mut() {
            style.styleNew = reveal_ex_style(style.styleNew, true);
        }
        return result;
    }
    if message == WM_NCDESTROY {
        let _ = RemoveWindowSubclass(hwnd, Some(reveal_proc), REVEAL_SUBCLASS);
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}
pub fn begin_reveal(window: &tauri::WebviewWindow, preserve_opacity: bool) -> bool {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        return unsafe {
            windows::Win32::UI::Shell::SetWindowSubclass(
                windows::Win32::Foundation::HWND(hwnd.0),
                Some(reveal_proc),
                REVEAL_SUBCLASS,
                usize::from(preserve_opacity),
            )
            .as_bool()
        };
    }
    let _ = (window, preserve_opacity);
    false
}
/// Move/resize the REAL HWND: DWM's glass and shadow must follow, not leave a static backdrop.
/// Its dock-side edge remains inside the work area; the WebView viewport never resizes.
pub fn set_reveal(
    window: &tauri::WebviewWindow,
    pose: crate::motion::Frame,
    size: (u32, u32),
    active: bool,
) {
    #[cfg(windows)]
    if active {
        if let Ok(hwnd) = window.hwnd() {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let (x, y, width, height) = crate::motion::host_geometry(pose, size);
            unsafe {
                let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSENDCHANGING,
                );
            }
            let _ = position_viewport(window, pose.slide.min(0));
            return;
        }
    }
    let _ = (size, active);
    let _ = window.set_position(tauri::PhysicalPosition::new(pose.x, pose.y));
}
pub fn clear_reveal(window: &tauri::WebviewWindow) {
    // The caller restores the full host dimensions BEFORE removing this short-lived hook.
    #[cfg(windows)]
    {
        let _ = position_viewport(window, 0);
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                use windows::Win32::{
                    Foundation::*,
                    UI::{Shell::RemoveWindowSubclass, WindowsAndMessaging::*},
                };
                let hwnd = HWND(hwnd.0);
                let _ = RemoveWindowSubclass(hwnd, Some(reveal_proc), REVEAL_SUBCLASS);
                // Resynchronize normal Wry/Tao sizing once, only after the full host is restored.
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect).is_ok() {
                    let kind = if IsIconic(hwnd).as_bool() {
                        SIZE_MINIMIZED
                    } else {
                        SIZE_RESTORED
                    };
                    SendMessageW(
                        hwnd,
                        WM_SIZE,
                        Some(WPARAM(kind as usize)),
                        Some(LPARAM(
                            ((rect.right as u32 & 0xffff) | ((rect.bottom as u32 & 0xffff) << 16))
                                as isize,
                        )),
                    );
                }
            }
        }
    }
    let _ = window;
}

#[cfg(windows)]
fn position_viewport(window: &tauri::WebviewWindow, slide: i32) -> bool {
    // Do NOT use Webview::set_position for a main/non-child Wry WebView: that API moves
    // the top-level HWND and resets the controller to (0,0). Move its native surface instead.
    let (tx, rx) = std::sync::mpsc::channel();
    if window
        .with_webview(move |native| unsafe {
            let controller = native.controller();
            let mut bounds = Default::default();
            let ok = controller.Bounds(&mut bounds).is_ok() && {
                let height = bounds.bottom - bounds.top;
                if bounds.top == slide {
                    true
                } else {
                    bounds.top = slide;
                    bounds.bottom = slide.saturating_add(height);
                    controller.SetBounds(bounds).is_ok()
                }
            };
            let _ = tx.send(ok);
        })
        .is_err()
    {
        return false;
    }
    // Motion always runs on the UI thread, so with_webview has already executed synchronously.
    rx.try_recv().unwrap_or(false)
}
pub fn viewport_status(window: &tauri::WebviewWindow) -> Option<serde_json::Value> {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        window.with_webview(move |native| unsafe {
            let mut r = Default::default();
            let value = native.controller().Bounds(&mut r).ok().map(|_| serde_json::json!({"position":{"x":r.left,"y":r.top},"size":{"width":r.right-r.left,"height":r.bottom-r.top}}));
            let _ = tx.send(value);
        }).ok()?;
        rx.try_recv().ok().flatten()
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

pub fn apply_window_style(window: &tauri::WebviewWindow, theme: &str) {
    let theme = match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    let _ = window.set_theme(theme);
    #[cfg(windows)]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};
        let dark = theme.unwrap_or_else(|| window.theme().unwrap_or(tauri::Theme::Light))
            == tauri::Theme::Dark;
        let color = if dark {
            Color(28, 31, 36, 210)
        } else {
            Color(242, 248, 253, 210)
        };
        let _ = window.set_effects(
            EffectsBuilder::new()
                .effect(Effect::Acrylic)
                .color(color)
                .build(),
        );
    }
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::Graphics::Dwm::*;
        let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
        // Native Windows 11 corner clipping/shadow, not a screenshot border.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::from_ref(&DWMWCP_ROUND).cast(),
                std::mem::size_of_val(&DWMWCP_ROUND) as u32,
            )
        };
    }
}

pub fn material_status(window: &tauri::WebviewWindow) -> serde_json::Value {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::Graphics::Dwm::*;
        let hwnd = windows::Win32::Foundation::HWND(hwnd.0);
        let read = |attribute| {
            let mut value = 0u32;
            unsafe {
                DwmGetWindowAttribute(hwnd, attribute, std::ptr::from_mut(&mut value).cast(), 4)
            }
            .ok()
            .map(|_| value)
        };
        use windows::Win32::UI::WindowsAndMessaging::*;
        let layered =
            unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } & WS_EX_LAYERED.0 as isize != 0;
        let mut alpha = 255u8;
        let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
        let opacity =
            unsafe { GetLayeredWindowAttributes(hwnd, None, Some(&mut alpha), Some(&mut flags)) }
                .ok()
                .map(|_| alpha);
        let reveal = unsafe {
            windows::Win32::UI::Shell::GetWindowSubclass(
                hwnd,
                Some(reveal_proc),
                REVEAL_SUBCLASS,
                None,
            )
            .as_bool()
        };
        return serde_json::json!({"systemBackdrop":read(DWMWA_SYSTEMBACKDROP_TYPE),"cornerPreference":read(DWMWA_WINDOW_CORNER_PREFERENCE),"layered":layered,"opacity":opacity,"hostReveal":reveal});
    }
    let _ = window;
    serde_json::Value::Null
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// One interactive Scheduled Task; no supervisor, repeat trigger, Run key, or PowerShell.
pub fn configure_startup(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::{
            core::BSTR,
            Win32::System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                    COINIT_MULTITHREADED,
                },
                TaskScheduler::*,
                Variant::VARIANT,
            },
        };
        // All COM interfaces stay on this fresh thread and are dropped before CoUninitialize.
        let executable = std::env::current_exe()?;
        let user = format!(
            "{}\\{}",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME")?
        );
        std::thread::spawn(move||->Result<()>{unsafe {
            CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;
            struct ComGuard;impl Drop for ComGuard {fn drop(&mut self){unsafe{CoUninitialize();}}}let _guard=ComGuard;
            let service:ITaskService=CoCreateInstance(&TaskScheduler,None,CLSCTX_INPROC_SERVER)?;
            service.Connect(&VARIANT::default(),&VARIANT::default(),&VARIANT::default(),&VARIANT::default())?;
            let folder=service.GetFolder(&BSTR::from("\\"))?;
            if !enabled {if let Err(e)=folder.DeleteTask(&BSTR::from("OpenToken"),0){if e.code().0 as u32!=0x80070002{return Err(e.into());}}return Ok(());}
            let task=format!(r#"<?xml version="1.0" encoding="UTF-16"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><RegistrationInfo><Description>OpenToken AI usage monitor. Restarts on failure; intentional Exit stays closed.</Description></RegistrationInfo><Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers><Principals><Principal id="User"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><StartWhenAvailable>true</StartWhenAvailable><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>10</Count></RestartOnFailure></Settings><Actions Context="User"><Exec><Command>{exe}</Command><Arguments>--background</Arguments><WorkingDirectory>{dir}</WorkingDirectory></Exec></Actions></Task>"#,user=xml(&user),exe=xml(&executable.to_string_lossy()),dir=xml(&executable.parent().unwrap_or(Path::new(".")).to_string_lossy()));
            folder.RegisterTask(&BSTR::from("OpenToken"),&BSTR::from(task),TASK_CREATE_OR_UPDATE.0,&VARIANT::from(user.as_str()),&VARIANT::default(),TASK_LOGON_INTERACTIVE_TOKEN,&VARIANT::default())?;
            Ok(())
        }}).join().map_err(|_|anyhow::anyhow!("Startup configuration thread failed"))??;
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        anyhow::bail!("Scheduled startup is only available on Windows.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn reveal_preserves_alpha_without_changing_pin_or_focus_style() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let flags = WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0 | WS_EX_NOACTIVATE.0;
        assert_eq!(reveal_ex_style(flags, true), flags | WS_EX_LAYERED.0);
        assert_eq!(
            reveal_ex_style(flags | WS_EX_LAYERED.0, true),
            flags | WS_EX_LAYERED.0
        );
        assert_eq!(
            reveal_ex_style(flags & !WS_EX_TOPMOST.0, true) & WS_EX_TOPMOST.0,
            0
        );
    }
    #[cfg(windows)]
    #[test]
    fn absent_opacity_lease_never_forces_a_layered_window() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        assert_eq!(
            reveal_ex_style(WS_EX_TOOLWINDOW.0, false),
            WS_EX_TOOLWINDOW.0
        );
    }
    #[test]
    fn provider_pages_are_fixed_https_allowlisted_destinations() {
        for id in ["cursor", "cursor_personal", "copilot"] {
            for docs in [true, false] {
                assert!(provider_page(id, docs).unwrap().starts_with("https://"));
            }
        }
        for id in [
            "https://malicious.test/",
            "cursor/../../",
            "file:///secret",
            "muse",
            "",
        ] {
            assert!(provider_page(id, false).is_err());
        }
    }
    #[test]
    fn escapes_xml_not_shell() {
        assert_eq!(xml("a&<\"'"), "a&amp;&lt;&quot;&apos;");
    }
}
