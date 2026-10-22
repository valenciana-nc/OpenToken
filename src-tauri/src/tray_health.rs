//! Cached handles are not proof of Explorer registration. Bounded retries only on shell failure.
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Health {
    pub unconfirmed: bool,
    pub repairs: u64,
    failures: u32,
    retry_at: Option<Instant>,
}
impl Health {
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
    fn ordinary_confirmation_is_not_a_repair_or_periodic_recreation() {
        let mut health = Health::default();
        for _ in 0..100 {
            health.confirmed(false);
        }
        assert_eq!(health.repairs, 0);
        assert!(!health.unconfirmed);
    }
}
