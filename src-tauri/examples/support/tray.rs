//! Test-only replay of Explorer's callbacks into a synthetic child's own registered tray HWND.
//! No Explorer injection, cross-process memory access, registry edits or clicks on other icons.
use windows::{
    core::BOOL,
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
};
struct Targets {
    pid: u32,
    main: bool,
    trays: Vec<HWND>,
}
unsafe extern "system" fn find(window: HWND, data: LPARAM) -> BOOL {
    let targets = &mut *(data.0 as *mut Targets);
    let mut pid = 0;
    GetWindowThreadProcessId(window, Some(&mut pid));
    if pid == targets.pid {
        let mut class = [0u16; 64];
        let len = GetClassNameW(window, &mut class);
        if String::from_utf16_lossy(&class[..len as usize]) == "tray_icon_app" {
            targets.trays.push(window);
        }
        let mut title = [0u16; 128];
        let len = GetWindowTextW(window, &mut title);
        targets.main |= String::from_utf16_lossy(&title[..len as usize]) == "OpenToken";
    }
    BOOL(1)
}
pub unsafe fn notify(pid: u32, index: usize, kind: usize) -> bool {
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    let message = match kind {
        1 => WM_LBUTTONDOWN,
        2 => WM_LBUTTONUP,
        3 => WM_LBUTTONDBLCLK,
        4 => 0, // Test-only loss of a selected synthetic icon registration.
        _ => return false,
    };
    let mut targets = Targets {
        pid,
        main: false,
        trays: Vec::new(),
    };
    let _ = EnumWindows(
        Some(find),
        LPARAM(std::ptr::from_mut(&mut targets) as isize),
    );
    targets.trays.sort_by_key(|window| window.0 as usize);
    let Some(&window) = targets.trays.get(index).filter(|_| targets.main) else {
        return false;
    };
    if kind == 4 {
        use windows::Win32::UI::Shell::{
            Shell_NotifyIconGetRect, Shell_NotifyIconW, NIM_DELETE, NOTIFYICONDATAW,
            NOTIFYICONIDENTIFIER,
        };
        // Standard shell APIs, exact synthetic child's own HWND; no Explorer
        // restart, injection, global broadcast, registry edit or remote memory.
        for id in 1..=512 {
            let identity = NOTIFYICONIDENTIFIER {
                cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
                hWnd: window,
                uID: id,
                ..Default::default()
            };
            if Shell_NotifyIconGetRect(&identity).is_ok() {
                let data = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: window,
                    uID: id,
                    ..Default::default()
                };
                return Shell_NotifyIconW(NIM_DELETE, &data).as_bool();
            }
        }
        return false;
    }
    // Pinned tray-icon implementation's WM_USER_TRAYICON. Event source queries its own Shell rect.
    SendMessageW(
        window,
        6002,
        Some(WPARAM(0)),
        Some(LPARAM(message as isize)),
    );
    true
}
