//! Native-smoke helper only. Not bundled or run by OpenToken; no credentials or startup access.
#[cfg(windows)]
#[path = "support/capture.rs"]
mod capture;
#[cfg(windows)]
#[path = "support/tray.rs"]
mod tray;
#[cfg(not(windows))]
fn main() {}
#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::{
        io::{self, BufRead, Write},
        mem::size_of,
    };
    use windows::{
        core::{w, BOOL},
        Win32::{
            Foundation::*,
            Graphics::Gdi::{
                CreateSolidBrush, DeleteObject, FillRect, GetStockObject, InvalidateRect,
                UpdateWindow, HBRUSH, HDC, HGDIOBJ, WHITE_BRUSH,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
    };
    struct Search {
        process: u32,
        window: HWND,
    }
    unsafe extern "system" fn find_main(window: HWND, data: LPARAM) -> BOOL {
        let search = &mut *(data.0 as *mut Search);
        let mut process = 0;
        GetWindowThreadProcessId(window, Some(&mut process));
        if process == search.process && IsWindowVisible(window).as_bool() {
            let mut title = [0u16; 128];
            let n = GetWindowTextW(window, &mut title);
            if String::from_utf16_lossy(&title[..n as usize]) == "OpenToken" {
                search.window = window;
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    static BACKDROP_COLOR: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0xffffff);
    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_ERASEBKGND {
            let mut rect = RECT::default();
            let brush = CreateSolidBrush(COLORREF(
                BACKDROP_COLOR.load(std::sync::atomic::Ordering::Relaxed),
            ));
            if !brush.0.is_null() {
                if GetClientRect(window, &mut rect).is_ok() {
                    FillRect(HDC(wparam.0 as *mut _), &rect, brush);
                }
                let _ = DeleteObject(HGDIOBJ(brush.0));
                return LRESULT(1);
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
    unsafe fn belongs_to(mut window: HWND, expected: u32) -> bool {
        for _ in 0..16 {
            let mut process = 0;
            GetWindowThreadProcessId(window, Some(&mut process));
            if process == expected && expected != 0 {
                return true;
            }
            let Ok(owner) = GetWindow(window, GW_OWNER) else {
                break;
            };
            if owner == window || owner.0.is_null() {
                break;
            }
            window = owner;
        }
        false
    }
    fn emit(value: &str) {
        println!("{value}");
        let _ = io::stdout().flush();
    }
    unsafe fn click(window: HWND, dashboard: bool) -> bool {
        let topmost = GetWindowLongPtrW(window, GWL_EXSTYLE) & WS_EX_TOPMOST.0 as isize != 0;
        let _ = SetWindowPos(
            window,
            Some(if !dashboard || topmost {
                HWND_TOPMOST
            } else {
                HWND_TOP
            }),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = SetForegroundWindow(window);
        let mut rect = RECT::default();
        if GetWindowRect(window, &mut rect).is_err() || !IsWindowVisible(window).as_bool() {
            return false;
        }
        let x = if dashboard {
            rect.left + (rect.right - rect.left) * 13 / 100
        } else {
            (rect.left + rect.right) / 2
        };
        let y = if dashboard {
            rect.top + (rect.bottom - rect.top) / 10
        } else {
            (rect.top + rect.bottom) / 2
        };
        let hit = GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT);
        if hit != window {
            eprintln!("Fixture click obscured: target={window:?}, hit={hit:?}, point=({x},{y}), style={:#x}", GetWindowLongPtrW(window, GWL_EXSTYLE));
            return false;
        }
        if SetCursorPos(x, y).is_err() {
            return false;
        }
        let inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        ..Default::default()
                    },
                },
            },
        ];
        SendInput(&inputs, size_of::<INPUT>() as i32) == 2
    }
    // SAFETY: this process owns its top-level window. Input is limited to that test window,
    // the visible synthetic OpenToken process explicitly named by PID, and Escape for its picker.
    unsafe {
        let args: Vec<_> = std::env::args().collect();
        if args.get(1).is_some_and(|s| s == "--capture") {
            let mut search = Search {
                process: args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
                window: HWND::default(),
            };
            let _ = EnumWindows(
                Some(find_main),
                LPARAM(std::ptr::from_mut(&mut search) as isize),
            );
            if search.window.0.is_null() || args.get(3).is_none() {
                return Err(windows::core::Error::from_hresult(E_FAIL));
            }
            if let Err(error) = capture::save(search.window, std::path::Path::new(&args[3])) {
                eprintln!("Fixture capture failed: {error}");
                return Err(windows::core::Error::from_hresult(E_FAIL));
            }
            return Ok(());
        }
        let mut original = POINT::default();
        let restore = GetCursorPos(&mut original).is_ok();
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("OpenTokenFocusTarget"),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            w!("OpenTokenFocusTarget"),
            w!("OpenToken focus test"),
            WS_OVERLAPPEDWINDOW,
            80,
            80,
            280,
            120,
            None,
            None,
            Some(instance),
            None,
        )?;
        let handle = window.0 as usize;
        std::thread::spawn(move || {
            let target = HWND(handle as *mut _);
            for line in io::stdin().lock().lines().map_while(Result::ok) {
                let mut words = line.split_whitespace();
                let command = words.next().unwrap_or("");
                if let Some(kind) = match command {
                    "traydown" => Some(1),
                    "trayup" => Some(2),
                    "traydouble" => Some(3),
                    "traydrop" => Some(4),
                    _ => None,
                } {
                    let pid = words
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    let index = words
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    let _ = PostMessageW(
                        Some(target),
                        WM_APP + 7,
                        WPARAM(pid),
                        LPARAM((kind << 16) | (index & 0xffff) as isize),
                    );
                    continue;
                }
                let (message, data) = match command {
                    "focus" => (WM_APP + 1, 0),
                    "color" => (
                        WM_APP + 9,
                        words
                            .next()
                            .and_then(|s| usize::from_str_radix(s, 16).ok())
                            .unwrap_or(0xffffff)
                            & 0xffffff,
                    ),
                    "hide" => (WM_APP + 2, 0),
                    "park" => (WM_APP + 10, 0),
                    "main" => (
                        WM_APP + 3,
                        words
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0),
                    ),
                    "cliyes" => (
                        WM_APP + 8,
                        words
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0),
                    ),
                    "escape" => (
                        WM_APP + 4,
                        words
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0),
                    ),
                    "backdrop" => (
                        WM_APP + 6,
                        words
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0),
                    ),
                    "allow" => (
                        WM_APP + 5,
                        words
                            .next()
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0),
                    ),
                    _ => (WM_CLOSE, 0),
                };
                let _ = PostMessageW(Some(target), message, WPARAM(data), LPARAM(0));
                if message == WM_CLOSE {
                    return;
                }
            }
            let _ = PostMessageW(Some(target), WM_CLOSE, WPARAM(0), LPARAM(0));
        });
        emit("ready");
        let mut message = MSG::default();
        loop {
            if GetMessageW(&mut message, None, 0, 0).0 <= 0 {
                break;
            }
            match message.message {
                m if m == WM_APP + 1 => {
                    let _ = SetWindowPos(
                        window,
                        None,
                        80,
                        80,
                        280,
                        120,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let ok = click(window, false);
                    emit(if ok { "focused" } else { "failed" });
                }
                m if m == WM_APP + 7 => {
                    let packed = message.lParam.0 as usize;
                    emit(
                        if tray::notify(message.wParam.0 as u32, packed & 0xffff, packed >> 16) {
                            "trayed"
                        } else {
                            "failed"
                        },
                    );
                }
                m if m == WM_APP + 6 => {
                    let mut search = Search {
                        process: message.wParam.0 as u32,
                        window: HWND::default(),
                    };
                    let _ = EnumWindows(
                        Some(find_main),
                        LPARAM(std::ptr::from_mut(&mut search) as isize),
                    );
                    let mut rect = RECT::default();
                    let ok = !search.window.0.is_null()
                        && GetWindowRect(search.window, &mut rect).is_ok()
                        && SetWindowPos(
                            window,
                            Some(HWND_TOPMOST),
                            rect.left - 40,
                            rect.top - 40,
                            rect.right - rect.left + 80,
                            rect.bottom - rect.top + 80,
                            SWP_SHOWWINDOW | SWP_NOACTIVATE,
                        )
                        .is_ok();
                    if ok {
                        // Keep our opaque fixture above unrelated apps but below the pinned synthetic panel.
                        let _ = SetWindowPos(
                            search.window,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                    emit(if ok { "backdrop" } else { "failed" });
                }
                m if m == WM_APP + 10 => {
                    let mut rect = RECT::default();
                    let ok = GetWindowRect(window, &mut rect).is_ok()
                        && SetCursorPos(rect.left + 20, rect.top + 20).is_ok();
                    emit(if ok { "parked" } else { "failed" });
                }
                m if m == WM_APP + 9 => {
                    BACKDROP_COLOR.store(
                        message.wParam.0 as u32,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    let _ = InvalidateRect(Some(window), None, true);
                    let _ = UpdateWindow(window);
                    emit("painted");
                }
                m if m == WM_APP + 2 => {
                    let _ = ShowWindow(window, SW_HIDE);
                    emit("hidden");
                }
                m if m == WM_APP + 3 => {
                    let mut search = Search {
                        process: message.wParam.0 as u32,
                        window: HWND::default(),
                    };
                    let _ = EnumWindows(
                        Some(find_main),
                        LPARAM(std::ptr::from_mut(&mut search) as isize),
                    );
                    emit(
                        if !search.window.0.is_null() && click(search.window, true) {
                            "main"
                        } else {
                            "failed"
                        },
                    );
                }
                m if m == WM_APP + 8 => {
                    // Test-only consent driver. Never approve a production/portable executable.
                    use windows::Win32::System::Threading::{
                        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                        PROCESS_QUERY_LIMITED_INFORMATION,
                    };
                    let pid = message.wParam.0 as u32;
                    let foreground = GetForegroundWindow();
                    let mut safe = false;
                    if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    {
                        let mut buf = vec![0u16; 32768];
                        let mut len = buf.len() as u32;
                        safe = QueryFullProcessImageNameW(
                            process,
                            PROCESS_NAME_WIN32,
                            windows::core::PWSTR(buf.as_mut_ptr()),
                            &mut len,
                        )
                        .is_ok()
                            && String::from_utf16_lossy(&buf[..len as usize])
                                .to_ascii_lowercase()
                                .ends_with("\\target\\release\\opentoken.exe");
                        let _ = CloseHandle(process);
                    }
                    let mut title = [0u16; 128];
                    let n = GetWindowTextW(foreground, &mut title);
                    safe &= belongs_to(foreground, pid)
                        && String::from_utf16_lossy(&title[..n as usize])
                            == "OpenToken · Antigravity CLI profile";
                    safe &= (SendMessageW(foreground, 0x0400, Some(WPARAM(0)), Some(LPARAM(0))).0
                        as u16)
                        == 7;
                    if safe {
                        if let Ok(button) = GetDlgItem(Some(foreground), 6) {
                            SendMessageW(button, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
                            emit("cli-confirmed");
                        } else {
                            emit("failed");
                        }
                    } else {
                        emit("failed");
                    }
                }
                m if m == WM_APP + 4 => {
                    if !belongs_to(GetForegroundWindow(), message.wParam.0 as u32) {
                        emit("failed");
                        continue;
                    }
                    let inputs = [
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_ESCAPE,
                                    ..Default::default()
                                },
                            },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_ESCAPE,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    ..Default::default()
                                },
                            },
                        },
                    ];
                    emit(if SendInput(&inputs, size_of::<INPUT>() as i32) == 2 {
                        "escaped"
                    } else {
                        "failed"
                    });
                }
                m if m == WM_APP + 5 => {
                    emit(
                        if AllowSetForegroundWindow(message.wParam.0 as u32).is_ok() {
                            "allowed"
                        } else {
                            "denied"
                        },
                    );
                }
                WM_CLOSE => break,
                _ => {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        let _ = DestroyWindow(window);
        if restore {
            let _ = SetCursorPos(original.x, original.y);
        }
    }
    Ok(())
}
