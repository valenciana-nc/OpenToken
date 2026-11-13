//! Native HWND translation/opacity: the real acrylic surface and its shadow move together.
use crate::{platform, AppState};
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, PhysicalPosition};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Hidden,
    Waiting,
    Opening,
    Open,
    Closing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub alpha: u8,
    /// Signed full-panel displacement inside the native window's moving reveal mask.
    pub slide: i32,
}
#[derive(Clone, Copy)]
struct Plan {
    epoch: u64,
    opening: bool,
    from: Frame,
    to: Frame,
    rest: (i32, i32),
    duration: u64,
    started: Instant,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub sequence: u64,
    pub ready: bool,
    pub requested_visible: bool,
    pub duration_ms: u64,
    pub alpha: u8,
    pub slide: i32,
    pub frames: u64,
    pub animation_enabled: bool,
    pub pacing: &'static str,
    pub frame_times_ms: Vec<f64>,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            phase: Phase::Hidden,
            sequence: 0,
            ready: false,
            requested_visible: false,
            duration_ms: 0,
            alpha: 255,
            slide: 0,
            frames: 0,
            animation_enabled: false,
            pacing: "idle",
            frame_times_ms: Vec::new(),
        }
    }
}
#[derive(Default)]
struct Inner {
    status: Status,
    rest: (i32, i32),
    offset: (i32, i32),
    layer: Option<bool>,
    host_size: Option<(u32, u32)>,
    viewport_height: i32,
    reveal: bool,
    last_frame: Option<Frame>,
}
#[derive(Default)]
pub struct State(Mutex<Inner>);
impl State {
    pub fn status(&self) -> Status {
        self.0.lock().unwrap().status.clone()
    }
    pub fn requested_visible(&self) -> bool {
        self.0.lock().unwrap().status.requested_visible
    }
    /// A click on a visibly closing popup means close, not reverse its dismissal.
    /// Explicit Show/CLI commands can still reverse a transition when that is intended.
    pub fn presented_for_tray(&self) -> bool {
        self.0.lock().unwrap().status.phase != Phase::Hidden
    }
    fn current(&self, epoch: u64) -> bool {
        let inner = self.0.lock().unwrap();
        inner.status.sequence == epoch
            && matches!(inner.status.phase, Phase::Opening | Phase::Closing)
    }
    pub fn settled_size(&self) -> Option<tauri::PhysicalSize<u32>> {
        let inner = self.0.lock().unwrap();
        matches!(inner.status.phase, Phase::Opening | Phase::Closing)
            .then_some(inner.host_size)
            .flatten()
            .map(|(w, h)| tauri::PhysicalSize::new(w, h))
    }
}
pub fn sample(from: Frame, to: Frame, t: f64, _opening: bool) -> Frame {
    let t = t.clamp(0., 1.);
    // Respond on the first frame, then decelerate to rest. A flat start made both
    // full-height motion and its fade look stalled before suddenly taking off.
    let eased = 1. - (1. - t).powi(3);
    let interpolate = |a: i32, b: i32| (a as f64 + (b as f64 - a as f64) * eased).round() as i32;
    Frame {
        x: interpolate(from.x, to.x),
        y: interpolate(from.y, to.y),
        alpha: interpolate(from.alpha as i32, to.alpha as i32).clamp(0, 255) as u8,
        slide: interpolate(from.slide, to.slide),
    }
}
/// Actual HWND geometry. The dock-side edge stays fixed while the whole glass host rises/falls.
pub fn host_geometry(pose: Frame, size: (u32, u32)) -> (i32, i32, u32, u32) {
    (
        pose.x,
        pose.y.saturating_add(pose.slide.max(0)),
        size.0,
        size.1.saturating_sub(pose.slide.unsigned_abs()).max(1),
    )
}
/// Never move the panel into the taskbar, even at fractional DPI or a negative monitor origin.
pub fn offset(
    area: crate::placement::WorkArea,
    size: (u32, u32),
    rest: (i32, i32),
    scale: f64,
    upward_edge: bool,
) -> (i32, i32) {
    let travel = (20. * scale).round().max(0.) as i32;
    let available = if upward_edge {
        rest.1 - area.y
    } else {
        area.y
            .saturating_add(area.height as i32)
            .saturating_sub(rest.1)
            .saturating_sub(size.1 as i32)
    }
    .max(0);
    (0, travel.min(available) * if upward_edge { -1 } else { 1 })
}
fn restore(window: &tauri::WebviewWindow, inner: &mut Inner) {
    if inner.reveal {
        platform::set_reveal(
            window,
            Frame {
                x: inner.rest.0,
                y: inner.rest.1,
                alpha: 255,
                slide: 0,
            },
            inner.host_size.unwrap_or((470, 769)),
            true,
        );
        platform::clear_reveal(window);
        inner.reveal = false;
    }
    if let Some(was_layered) = inner.layer.take() {
        platform::restore_opacity(window, was_layered);
    }
    inner.status.alpha = 255;
    inner.status.slide = 0;
}
pub fn ready(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<std::sync::Arc<AppState>>() else {
        return;
    };
    let pending = {
        let mut inner = state.motion.0.lock().unwrap();
        if inner.status.ready {
            return;
        }
        inner.status.ready = true;
        inner
            .status
            .requested_visible
            .then_some((inner.rest, inner.offset))
    };
    if let Some((rest, offset)) = pending {
        show(app, rest, offset);
    }
}
pub fn show(app: &tauri::AppHandle, rest: (i32, i32), offset: (i32, i32)) {
    request(app, true, Some((rest, offset)));
}
pub fn hide(app: &tauri::AppHandle) {
    request(app, false, None);
}
pub fn reposition(app: &tauri::AppHandle, rest: (i32, i32), offset: (i32, i32)) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || reposition_on_main(&handle, rest, offset));
}
fn reposition_on_main(app: &tauri::AppHandle, rest: (i32, i32), offset: (i32, i32)) {
    let _activity = crate::runtime_health::activity(crate::runtime_health::Stage::Motion);
    let (Some(window), Some(state)) = (
        app.get_webview_window("main"),
        app.try_state::<std::sync::Arc<AppState>>(),
    ) else {
        return;
    };
    let mut inner = state.motion.0.lock().unwrap();
    if !inner.status.requested_visible {
        return;
    }
    inner.status.sequence += 1;
    inner.rest = rest;
    inner.offset = offset;
    restore(&window, &mut inner);
    let _ = window.set_position(PhysicalPosition::new(rest.0, rest.1));
    inner.status.phase = if window.is_visible().unwrap_or(false) {
        Phase::Open
    } else {
        Phase::Waiting
    };
}
fn request(app: &tauri::AppHandle, opening: bool, destination: Option<((i32, i32), (i32, i32))>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || begin(&handle, opening, destination));
}
fn begin(app: &tauri::AppHandle, opening: bool, destination: Option<((i32, i32), (i32, i32))>) {
    let _activity = crate::runtime_health::activity(crate::runtime_health::Stage::Motion);
    let (Some(window), Some(state)) = (
        app.get_webview_window("main"),
        app.try_state::<std::sync::Arc<AppState>>(),
    ) else {
        return;
    };
    let mut inner = state.motion.0.lock().unwrap();
    let previous_phase = inner.status.phase;
    let destination_changed = destination.is_some_and(|(rest, _)| rest != inner.rest);
    if let Some((rest, offset)) = destination {
        inner.rest = rest;
        inner.offset = offset;
    }
    if inner.status.requested_visible == opening
        && !(destination_changed && previous_phase == Phase::Opening)
        && matches!(
            (opening, previous_phase),
            (true, Phase::Opening | Phase::Open) | (false, Phase::Closing | Phase::Hidden)
        )
    {
        if opening && previous_phase == Phase::Open {
            let rest = inner.rest;
            drop(inner);
            let _ = window.set_position(PhysicalPosition::new(rest.0, rest.1));
            let _ = window.set_focus();
            focus_content(&window);
        }
        return;
    }
    inner.status.sequence += 1;
    inner.status.requested_visible = opening;
    let epoch = inner.status.sequence;
    if opening && !inner.status.ready {
        inner.status.phase = Phase::Waiting;
        drop(inner);
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let app = handle.clone();
            let _ = handle.run_on_main_thread(move || ready(&app));
        });
        return;
    }
    let visible = window.is_visible().unwrap_or(false);
    if !opening && !visible {
        restore(&window, &mut inner);
        inner.status.phase = Phase::Hidden;
        drop(inner);
        crate::visibility(app, false);
        return;
    }
    let current = window
        .outer_position()
        .unwrap_or(PhysicalPosition::new(inner.rest.0, inner.rest.1));
    if !matches!(previous_phase, Phase::Opening | Phase::Closing) {
        inner.host_size = window.outer_size().ok().map(|s| (s.width, s.height));
        inner.viewport_height = window.inner_size().map(|s| s.height as i32).unwrap_or(760);
    }
    // A user-dragged, fully open panel closes from its actual position, without snapping to its dock.
    if !opening && previous_phase == Phase::Open {
        inner.rest = (current.x, current.y);
        if let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) {
            let a = monitor.work_area();
            inner.offset = offset(
                crate::placement::WorkArea {
                    x: a.position.x,
                    y: a.position.y,
                    width: a.size.width,
                    height: a.size.height,
                },
                (size.width, size.height),
                inner.rest,
                monitor.scale_factor(),
                inner.offset.1 < 0,
            );
        }
    }
    let allowed = platform::animations_enabled();
    let base_duration = if opening { 300 } else { 220 };
    let rest = inner.rest;
    // The full-height reveal already supplies the slide. A second positional nudge
    // moved the dock edge as well, making the entire bubble drift during the transition.
    let away = rest;
    let full_slide = if allowed {
        inner.viewport_height * if inner.offset.1 < 0 { -1 } else { 1 }
    } else {
        0
    };
    let from = if visible {
        Frame {
            x: current.x,
            y: current.y.saturating_sub(inner.status.slide.max(0)),
            alpha: inner.status.alpha,
            slide: inner.status.slide,
        }
    } else {
        Frame {
            x: away.0,
            y: away.1,
            alpha: if allowed { 0 } else { 255 },
            slide: full_slide,
        }
    };
    let to = if opening {
        Frame {
            x: rest.0,
            y: rest.1,
            alpha: 255,
            slide: 0,
        }
    } else {
        Frame {
            x: away.0,
            y: away.1,
            alpha: 0,
            slide: full_slide,
        }
    };
    let duration = transition_duration(allowed, base_duration, from, to, inner.viewport_height);
    inner.status.phase = if opening {
        Phase::Opening
    } else {
        Phase::Closing
    };
    inner.status.duration_ms = duration;
    inner.status.animation_enabled = allowed;
    inner.status.frames = 0;
    inner.status.pacing = "idle";
    inner.status.frame_times_ms = vec![0.];
    if allowed && inner.layer.is_none() {
        inner.layer = platform::begin_opacity(&window);
    }
    if allowed && !inner.reveal {
        inner.reveal = platform::begin_reveal(&window, inner.layer.is_some());
    }
    platform::set_reveal(
        &window,
        from,
        inner.host_size.unwrap_or((470, 769)),
        inner.reveal,
    );
    if inner.layer.is_some() {
        platform::set_opacity(&window, from.alpha);
    }
    inner.status.alpha = from.alpha;
    inner.status.slide = from.slide;
    inner.last_frame = Some(from);
    let plan = Plan {
        epoch,
        opening,
        from,
        to,
        rest,
        duration,
        started: Instant::now(),
    };
    drop(inner);
    if opening {
        // Show/focus dispatch native callbacks. Publish state and release its mutex first.
        let _ = window.show();
        let _ = window.set_focus();
        crate::visibility(app, true);
        if state.flyout.modal_count() == 0 {
            focus_content(&window);
        }
    }
    if duration == 0 {
        finish(app, plan);
        return;
    }
    pump_animation(app, plan);
}
/// Distance-aware reversals settle quickly instead of restarting an entire full-height glide.
fn transition_duration(enabled: bool, base: u64, from: Frame, to: Frame, height: i32) -> u64 {
    if !enabled {
        return 0;
    }
    let distance = ((to.slide as f64 - from.slide as f64).abs() / height.max(1) as f64)
        .max((to.alpha as f64 - from.alpha as f64).abs() / 255.)
        .clamp(0., 1.);
    ((base as f64 * distance.sqrt()).round() as u64).clamp(80, base)
}
/// One acknowledged UI frame per compositor wake. The UI thread never waits for vblank.
/// No raw timer callbacks/properties, global timer-resolution changes or idle animation loop.
fn pump_animation(app: &tauri::AppHandle, plan: Plan) {
    let Some(state) = app.try_state::<std::sync::Arc<AppState>>() else {
        return;
    };
    let shared = state.inner().clone();
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        loop {
            if !shared.motion.current(plan.epoch) {
                break;
            }
            let waiting = Instant::now();
            let composed = platform::wait_for_compositor();
            let compositor_paced = composed && waiting.elapsed() >= Duration::from_millis(1);
            if !compositor_paced {
                // Bounded fallback if DWM is unavailable or returns without actually waiting.
                std::thread::sleep(Duration::from_millis(8).saturating_sub(waiting.elapsed()));
            }
            if !shared.motion.current(plan.epoch) {
                break;
            }
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let app = handle.clone();
            if handle
                .run_on_main_thread(move || {
                    // Sample NOW, on the UI thread—not before a potentially delayed queue hop.
                    let progress = (plan.started.elapsed().as_secs_f64() * 1000.
                        / plan.duration as f64)
                        .min(1.);
                    let active = if progress >= 1. {
                        finish(&app, plan)
                    } else {
                        frame(&app, plan, progress, compositor_paced)
                    };
                    let _ = tx.send(active && progress < 1.);
                })
                .is_err()
            {
                break;
            }
            if rx.recv() != Ok(true) {
                break;
            }
        }
    });
}
fn frame(app: &tauri::AppHandle, plan: Plan, progress: f64, compositor_paced: bool) -> bool {
    let _activity = crate::runtime_health::activity(crate::runtime_health::Stage::Motion);
    let (Some(window), Some(state)) = (
        app.get_webview_window("main"),
        app.try_state::<std::sync::Arc<AppState>>(),
    ) else {
        return false;
    };
    let mut inner = state.motion.0.lock().unwrap();
    if inner.status.sequence != plan.epoch {
        return false;
    }
    let value = sample(plan.from, plan.to, progress, plan.opening);
    let previous = inner.last_frame;
    if previous.is_none_or(|p| (p.x, p.y, p.slide) != (value.x, value.y, value.slide)) {
        platform::set_reveal(
            &window,
            value,
            inner.host_size.unwrap_or((470, 769)),
            inner.reveal,
        );
    }
    if inner.layer.is_some() && previous.is_none_or(|p| p.alpha != value.alpha) {
        platform::set_opacity(&window, value.alpha);
    }
    inner.last_frame = Some(value);
    inner.status.pacing = if compositor_paced {
        "compositor"
    } else {
        "fallback"
    };
    if inner.status.frame_times_ms.len() < 256 {
        inner
            .status
            .frame_times_ms
            .push(plan.started.elapsed().as_secs_f64() * 1000.);
    }
    inner.status.alpha = value.alpha;
    inner.status.slide = value.slide;
    inner.status.frames += 1;
    true
}
fn finish(app: &tauri::AppHandle, plan: Plan) -> bool {
    let _activity = crate::runtime_health::activity(crate::runtime_health::Stage::Motion);
    let (Some(window), Some(state)) = (
        app.get_webview_window("main"),
        app.try_state::<std::sync::Arc<AppState>>(),
    ) else {
        return false;
    };
    let mut inner = state.motion.0.lock().unwrap();
    if inner.status.sequence != plan.epoch {
        return false;
    }
    if !plan.opening {
        let _ = window.hide();
    }
    restore(&window, &mut inner);
    let _ = window.set_position(PhysicalPosition::new(plan.rest.0, plan.rest.1));
    inner.status.phase = if plan.opening {
        Phase::Open
    } else {
        Phase::Hidden
    };
    inner.status.frames += 1;
    if inner.status.frame_times_ms.len() < 256 {
        inner
            .status
            .frame_times_ms
            .push(plan.started.elapsed().as_secs_f64() * 1000.);
    }
    inner.last_frame = None;
    drop(inner);
    if !plan.opening {
        crate::visibility(app, false);
    } else if state.flyout.modal_count() == 0 {
        focus_content(&window);
    }
    true
}
fn focus_content(window: &tauri::WebviewWindow) {
    // Focus the WebView, not just its host HWND. Never reclaim focus from another app or popup.
    if platform::foreground(window) == crate::flyout::Foreground::Main {
        let webview: &tauri::Webview = window.as_ref();
        let _ = webview.set_focus();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_and_alpha_are_exact() {
        let a = Frame {
            x: -1100,
            y: 40,
            alpha: 0,
            slide: 760,
        };
        let b = Frame {
            x: -1100,
            y: 20,
            alpha: 255,
            slide: 0,
        };
        assert_eq!(sample(a, b, 0., true), a);
        assert_eq!(sample(a, b, 1., true), b);
        assert_eq!(sample(b, a, 1., false), a);
    }
    #[test]
    fn interpolation_is_monotonic_and_clamped() {
        let a = Frame {
            x: 0,
            y: 20,
            alpha: 0,
            slide: 760,
        };
        let b = Frame {
            x: 0,
            y: 0,
            alpha: 255,
            slide: 0,
        };
        let mut last = a;
        for i in 0..101 {
            let f = sample(a, b, i as f64 / 100., true);
            assert!(f.y <= last.y && f.alpha >= last.alpha && f.slide <= last.slide);
            last = f;
        }
        assert_eq!(sample(a, b, 2., true), b);
    }
    #[test]
    fn full_bubble_reveals_from_edge_and_clamps_in_both_directions() {
        let pose = |slide| Frame {
            x: -1000,
            y: 300,
            alpha: 128,
            slide,
        };
        assert_eq!(host_geometry(pose(760), (470, 769)), (-1000, 1060, 470, 9));
        assert_eq!(host_geometry(pose(400), (470, 769)), (-1000, 700, 470, 369));
        assert_eq!(host_geometry(pose(0), (470, 769)), (-1000, 300, 470, 769));
        assert_eq!(
            host_geometry(pose(-400), (470, 769)),
            (-1000, 300, 470, 369)
        );
        assert_eq!(host_geometry(pose(9999), (470, 769)).3, 1);
    }
    #[test]
    fn reversal_starts_from_current_bubble_and_opacity() {
        let open = Frame {
            x: -500,
            y: 200,
            alpha: 255,
            slide: 0,
        };
        let hidden = Frame {
            x: -500,
            y: 210,
            alpha: 0,
            slide: 760,
        };
        let midway = sample(open, hidden, 0.6, false);
        assert_eq!(sample(midway, open, 0., true), midway);
        assert_eq!(sample(midway, open, 1., true), open);
    }
    #[test]
    fn responsive_curve_moves_immediately_then_settles_in_both_directions() {
        let a = Frame {
            x: 0,
            y: 0,
            slide: 1_000_000,
            alpha: 0,
        };
        let b = Frame {
            x: 0,
            y: 0,
            slide: 0,
            alpha: 255,
        };
        for opening in [false, true] {
            assert!(1_000_000 - sample(a, b, 0.01, opening).slide > 20_000);
            assert!(sample(a, b, 0.99, opening).slide <= 10);
            assert_eq!(sample(a, b, 0.5, opening).slide, 125_000);
        }
    }
    #[test]
    fn dock_edge_is_constant_through_the_whole_bottom_reveal() {
        let a = Frame {
            x: -1000,
            y: 300,
            slide: 760,
            alpha: 0,
        };
        let b = Frame {
            slide: 0,
            alpha: 255,
            ..a
        };
        for i in 0..=100 {
            let (_, y, _, height) = host_geometry(sample(a, b, i as f64 / 100., true), (470, 769));
            assert_eq!(y + height as i32, 1069);
        }
    }
    #[test]
    fn short_reversals_finish_quickly_and_reduced_motion_is_instant() {
        let hidden = Frame {
            x: 0,
            y: 0,
            slide: 760,
            alpha: 0,
        };
        let open = Frame {
            x: 0,
            y: 0,
            slide: 0,
            alpha: 255,
        };
        assert_eq!(transition_duration(true, 300, hidden, open, 760), 300);
        assert_eq!(transition_duration(false, 300, hidden, open, 760), 0);
        let nearly_open = Frame {
            slide: 10,
            alpha: 250,
            ..open
        };
        assert_eq!(transition_duration(true, 300, nearly_open, open, 760), 80);
        assert!(transition_duration(true, 300, sample(hidden, open, 0.5, true), open, 760) < 300);
    }
    #[test]
    fn tray_snapshot_includes_blur_closing_but_not_settled_hidden() {
        let state = State::default();
        for phase in [Phase::Waiting, Phase::Opening, Phase::Open, Phase::Closing] {
            let mut inner = state.0.lock().unwrap();
            inner.status.phase = phase;
            inner.status.requested_visible = false;
            drop(inner);
            assert!(state.presented_for_tray());
        }
        state.0.lock().unwrap().status.phase = Phase::Hidden;
        assert!(!state.presented_for_tray());
    }
    #[test]
    fn driver_rejects_old_generations_and_stops_at_idle() {
        let state = State::default();
        {
            let mut i = state.0.lock().unwrap();
            i.status.sequence = 1;
            i.status.phase = Phase::Opening;
        }
        assert!(state.current(1));
        {
            let mut i = state.0.lock().unwrap();
            i.status.sequence = 2;
            i.status.phase = Phase::Closing;
        }
        assert!(!state.current(1));
        assert!(state.current(2));
        state.0.lock().unwrap().status.phase = Phase::Hidden;
        assert!(!state.current(2));
    }
    #[test]
    fn offsets_respect_taskbar_dpi_and_negative_origins() {
        let a = crate::placement::WorkArea {
            x: -1920,
            y: -100,
            width: 1920,
            height: 1100,
        };
        assert_eq!(offset(a, (470, 769), (-500, 221), 1.25, false), (0, 10));
        assert_eq!(offset(a, (470, 769), (-500, -90), 1.25, true), (0, -10));
        assert_eq!(offset(a, (470, 1200), (-500, -100), 1., false), (0, 0));
    }
}
