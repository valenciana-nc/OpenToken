//! Dismissal policy, independent of WebView/DOM blur events.
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foreground {
    Main,
    OwnedPopup,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Keep,
    Wait,
    Dismiss,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub pinned: bool,
    pub visible: bool,
    pub focused: bool,
    pub modal: bool,
    pub tray_pressed: bool,
    pub foreground: Foreground,
}
pub fn decide(c: Context) -> Decision {
    if c.pinned || !c.visible || c.focused || c.modal {
        return Decision::Keep;
    }
    if c.tray_pressed {
        return Decision::Wait;
    }
    match c.foreground {
        Foreground::Main => Decision::Keep,
        Foreground::OwnedPopup => Decision::Wait,
        Foreground::Other => Decision::Dismiss,
    }
}
struct TrayPress {
    icon: String,
    presented: bool,
    at: Instant,
}
const TRAY_LEASE: Duration = Duration::from_secs(2);
#[derive(Default)]
pub struct State {
    epoch: AtomicU64,
    modals: AtomicUsize,
    tray: Mutex<Option<TrayPress>>,
}
impl State {
    pub fn cancel(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }
    pub fn current(&self, epoch: u64) -> bool {
        self.epoch.load(Ordering::Relaxed) == epoch
    }
    pub fn modal_count(&self) -> usize {
        self.modals.load(Ordering::Relaxed)
    }
    pub fn tray_pressed(&self) -> bool {
        self.tray
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|press| press.at.elapsed() < TRAY_LEASE)
    }
    /// Both a normal down and Windows' second-press DoubleClick begin one gesture.
    pub fn tray_down(&self, icon: &str, presented: bool) {
        self.cancel();
        *self.tray.lock().unwrap() = Some(TrayPress {
            icon: icon.into(),
            presented,
            at: Instant::now(),
        });
    }
    /// Never reinterpret an unmatched/repeated/expired mouse-up as a new open request.
    pub fn tray_up(&self, icon: &str) -> Option<bool> {
        // A matched action cancels dismissal via show/hide. An ignored release must
        // leave the existing blur timer alive after ending the mouse-down lease.
        self.tray
            .lock()
            .unwrap()
            .take()
            .filter(|press| press.icon == icon && press.at.elapsed() < TRAY_LEASE)
            .map(|press| press.presented)
    }
    pub fn hold(self: &Arc<Self>) -> Hold {
        self.cancel();
        self.modals.fetch_add(1, Ordering::Relaxed);
        Hold(self.clone())
    }
}
pub struct Hold(Arc<State>);
impl Drop for Hold {
    fn drop(&mut self) {
        self.0.modals.fetch_sub(1, Ordering::Relaxed);
        self.0.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn outside() -> Context {
        Context {
            pinned: false,
            visible: true,
            focused: false,
            modal: false,
            tray_pressed: false,
            foreground: Foreground::Other,
        }
    }
    #[test]
    fn unpinned_external_focus_dismisses() {
        assert_eq!(decide(outside()), Decision::Dismiss);
    }
    #[test]
    fn pinned_never_auto_dismisses() {
        assert_eq!(
            decide(Context {
                pinned: true,
                ..outside()
            }),
            Decision::Keep
        );
    }
    #[test]
    fn hidden_and_focused_windows_stay_untouched() {
        assert_eq!(
            decide(Context {
                visible: false,
                ..outside()
            }),
            Decision::Keep
        );
        assert_eq!(
            decide(Context {
                focused: true,
                ..outside()
            }),
            Decision::Keep
        );
        assert_eq!(
            decide(Context {
                foreground: Foreground::Main,
                ..outside()
            }),
            Decision::Keep
        );
    }
    #[test]
    fn owned_dialogs_and_tray_gestures_do_not_collapse_the_parent() {
        assert_eq!(
            decide(Context {
                modal: true,
                ..outside()
            }),
            Decision::Keep
        );
        assert_eq!(
            decide(Context {
                tray_pressed: true,
                ..outside()
            }),
            Decision::Wait
        );
        assert_eq!(
            decide(Context {
                foreground: Foreground::OwnedPopup,
                ..outside()
            }),
            Decision::Wait
        );
    }
    #[test]
    fn refocus_and_explicit_show_invalidate_old_dismissals() {
        let s = State::default();
        let old = s.cancel();
        let new = s.cancel();
        assert!(!s.current(old));
        assert!(s.current(new));
    }
    #[test]
    fn dialog_holds_release_on_every_exit_path() {
        let s = Arc::new(State::default());
        {
            let _hold = s.hold();
            assert_eq!(s.modal_count(), 1);
        }
        assert_eq!(s.modal_count(), 0);
    }
    #[test]
    fn a_lost_mouse_up_does_not_block_dismissal_forever() {
        let s = State::default();
        *s.tray.lock().unwrap() = Some(TrayPress {
            icon: "a".into(),
            presented: true,
            at: Instant::now() - Duration::from_secs(3),
        });
        assert!(!s.tray_pressed());
    }
    #[test]
    fn a_bare_release_cannot_reopen_an_auto_dismissed_panel() {
        let s = State::default();
        assert_eq!(s.tray_up("a"), None);
    }
    #[test]
    fn a_release_from_another_badge_does_not_toggle_the_shared_panel() {
        let s = State::default();
        s.tray_down("a", true);
        assert_eq!(s.tray_up("b"), None);
        assert!(!s.tray_pressed());
        s.tray_down("b", false);
        assert_eq!(s.tray_up("b"), Some(false));
    }
    #[test]
    fn ignored_release_does_not_cancel_the_pending_blur_check() {
        let s = State::default();
        s.tray_down("a", true);
        let pending_blur = s.cancel();
        assert_eq!(s.tray_up("b"), None);
        assert!(s.current(pending_blur));
        assert!(!s.tray_pressed());
        assert_eq!(decide(outside()), Decision::Dismiss);
    }
    #[test]
    fn a_duplicate_release_never_performs_a_second_toggle() {
        let s = State::default();
        s.tray_down("a", false);
        assert_eq!(s.tray_up("a"), Some(false));
        assert_eq!(s.tray_up("a"), None);
    }
    #[test]
    fn expired_press_does_not_later_open_the_window() {
        let s = State::default();
        *s.tray.lock().unwrap() = Some(TrayPress {
            icon: "a".into(),
            presented: false,
            at: Instant::now() - Duration::from_secs(3),
        });
        assert_eq!(s.tray_up("a"), None);
    }
    #[test]
    fn double_click_second_press_latches_a_close_even_after_blur_started() {
        let s = State::default();
        s.tray_down("a", false);
        assert_eq!(s.tray_up("a"), Some(false)); // first click opens
        s.tray_down("a", true); // DoubleClick is a second down; closing still looks open
        assert_eq!(s.tray_up("a"), Some(true)); // does not reopen when focus loss wins
    }
    #[test]
    fn tray_click_uses_visibility_at_mouse_down() {
        let s = State::default();
        s.tray_down("a", true);
        assert!(s.tray_pressed());
        assert_eq!(s.tray_up("a"), Some(true));
        assert!(!s.tray_pressed());
        assert_eq!(s.tray_up("a"), None);
    }
}
