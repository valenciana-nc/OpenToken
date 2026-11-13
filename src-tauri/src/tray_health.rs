//! Cached handles are not proof of Explorer registration. Bounded retries only on shell failure.
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Health {
    pub unconfirmed: bool,
    pub repairs: u64,
    failures: u32,
    retry_at: Option<Instant>,
    probed_at: Option<Instant>,
}
impl Health {
    /// Snapshot bursts must not repeatedly make synchronous calls into Explorer.
    pub fn should_probe(&mut self, now: Instant, changed: bool) -> bool {
        if self.unconfirmed && self.retry_at.is_some_and(|at| now < at) {
            return false;
        }
        if !changed
            && self
                .probed_at
                .is_some_and(|at| now.duration_since(at) < Duration::from_secs(15))
        {
            return false;
        }
        self.probed_at = Some(now);
        true
    }
    pub fn failed(&mut self, now: Instant) -> bool {
        self.unconfirmed = true;
        if self.retry_at.is_some_and(|at| now < at) {
            return false;
        }
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(now + Duration::from_secs((5u64 << self.failures.min(4)).min(60)));
        true
    }
    pub fn confirmed(&mut self, repaired: bool) {
        self.unconfirmed = false;
        self.failures = 0;
        self.retry_at = None;
        if repaired {
            self.repairs = self.repairs.saturating_add(1);
        }
    }
}
/// Stable Windows identity for one tray slot.
///
/// Without a GUID, Explorer identifies an icon by (HWND, uID) and tray-icon hands
/// out uIDs from a process-wide counter, so every launch (and every remove/recreate)
/// looks like a brand-new icon: the user's "show in taskbar" choice is lost and the
/// icon drops into the overflow. Windows binds an unsigned binary's GUID to its path,
/// so the path is part of the hash: a dev build and an installed build never collide.
pub fn guid(exe: &std::path::Path, slot: &str) -> u128 {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013B;
    let path = exe.to_string_lossy().to_lowercase();
    let mut hash = OFFSET;
    for byte in path.bytes().chain([0]).chain(slot.bytes()) {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    // RFC 9562 version 8 (custom) + variant bits so it is a well-formed GUID.
    (hash & !(0xF << 76) & !(0x3 << 62)) | (0x8 << 76) | (0x2 << 62)
}
/// True when Explorer's taskbar (the notification area's host) exists.
pub fn shell_ready() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::{core::w, Win32::UI::WindowsAndMessaging::FindWindowW};
        return FindWindowW(w!("Shell_TrayWnd"), None).is_ok_and(|h| !h.is_invalid());
    }
    #[allow(unreachable_code)]
    true
}
pub fn restore(icon: &tauri::tray::TrayIcon) -> bool {
    #[cfg(windows)]
    {
        // Reuse the library's registration path and the SAME icon identity/HWND.
        // Message is sent only to this app-owned window, never broadcast to Explorer.
        use windows::{
            core::w,
            Win32::{
                Foundation::{HWND, LPARAM, WPARAM},
                UI::WindowsAndMessaging::{RegisterWindowMessageW, SendMessageW},
            },
        };
        let handle = icon.with_inner_tray_icon(|inner| inner.window_handle() as usize);
        if let Ok(handle) = handle {
            if handle != 0 {
                unsafe {
                    let message = RegisterWindowMessageW(w!("TaskbarCreated"));
                    if message != 0 {
                        SendMessageW(
                            HWND(handle as *mut _),
                            message,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        );
                        return true;
                    }
                }
            }
        }
    }
    let _ = icon;
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_are_bounded_and_success_resets_the_lease() {
        let mut health = Health::default();
        let now = Instant::now();
        assert!(health.failed(now));
        assert!(health.unconfirmed);
        assert!(!health.failed(now + Duration::from_secs(1)));
        assert!(health.failed(now + Duration::from_secs(60)));
        health.confirmed(true);
        assert!(!health.unconfirmed);
        assert_eq!(health.repairs, 1);
        assert!(health.failed(now + Duration::from_secs(61)));
    }
    #[test]
    fn snapshot_bursts_do_not_spam_explorer_and_failed_probes_obey_backoff() {
        let now = Instant::now();
        let mut health = Health::default();
        assert!(health.should_probe(now, false));
        for _ in 0..100 {
            assert!(!health.should_probe(now + Duration::from_secs(1), false));
        }
        assert!(health.should_probe(now + Duration::from_secs(2), true));
        assert!(health.failed(now + Duration::from_secs(2)));
        assert!(!health.should_probe(now + Duration::from_secs(3), true));
        assert!(health.should_probe(now + Duration::from_secs(17), false));
        health.confirmed(true);
        assert!(!health.should_probe(now + Duration::from_secs(18), false));
        assert!(health.should_probe(now + Duration::from_secs(32), false));
    }
    #[test]
    fn tray_guids_are_stable_per_slot_and_per_binary() {
        let exe = std::path::Path::new(r"C:\Users\me\OpenToken\OpenToken.exe");
        let a = guid(exe, "codex");
        assert_eq!(a, guid(exe, "codex"));
        assert_eq!(
            a,
            guid(
                std::path::Path::new(r"c:\users\ME\opentoken\opentoken.exe"),
                "codex"
            )
        );
        assert_ne!(a, guid(exe, "codex:provider"));
        assert_ne!(
            a,
            guid(std::path::Path::new(r"C:\other\OpenToken.exe"), "codex")
        );
        assert_eq!((a >> 76) & 0xF, 0x8);
        assert_eq!((a >> 62) & 0x3, 0x2);
    }
    #[test]
    fn ordinary_confirmation_is_not_a_repair_or_periodic_recreation() {
        let mut health = Health::default();
        for _ in 0..100 {
            health.confirmed(false);
        }
        assert_eq!(health.repairs, 0);
        assert!(!health.unconfirmed);
    }
}
