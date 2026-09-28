//! Window configuration and the shared platform state.
//!
//! The window is created by [`WinitPlugin`](crate::WinitPlugin)'s runner on the
//! first resume; every plugin that needs it reads the [`SharedWindow`] service
//! the runner publishes. Mutable per-window state (scale factor, cursor,
//! double-click tracker, IME composition) lives in [`WindowState`], shared so
//! the pointer / keyboard / IME plugins stay independent of each other.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use igui_core::Vec2;
use winit::window::Window;

use crate::input::DoubleClickTracker;

/// macOS transparent-title-bar safe area: content under the traffic lights
/// should reserve this many logical pixels of top padding.
pub const TITLEBAR_SAFE_AREA: f32 = 28.0;

/// How the OS window frame is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitlebarMode {
    /// Keep the OS frame as-is.
    #[default]
    Native,
    /// Remove the native title bar entirely (with the traffic lights).
    Hidden,
    /// macOS: keep the traffic lights, hide the title bar background. Other
    /// platforms fall back to [`TitlebarMode::Native`].
    Transparent,
}

/// Everything the runner needs to create the window.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    /// Window size in logical pixels.
    pub size: (f64, f64),
    pub titlebar: TitlebarMode,
    /// Whether the platform IME is enabled for this window.
    pub ime: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "igui".into(),
            size: (1200.0, 780.0),
            titlebar: TitlebarMode::Native,
            ime: false,
        }
    }
}

/// Mutable state for the one window, shared by the platform plugins.
#[derive(Default)]
pub struct WindowState {
    /// The current scale factor (device pixel ratio).
    pub scale_factor: f64,
    /// The last pointer position in logical pixels.
    pub cursor: Vec2,
    /// Left-button press tracker for double-click detection.
    pub double_click: DoubleClickTracker,
    /// The platform IME is enabled (a composition may start).
    pub ime_active: bool,
    /// A composition (preedit) is in progress: the platform owns the text, so
    /// committed keyboard text must be ignored until it ends.
    pub composing: bool,
}

/// Shared [`WindowState`]; created by [`WinitPlugin`](crate::WinitPlugin) before
/// the window exists, updated by the runner and the input plugins.
pub type SharedWindowState = Rc<RefCell<WindowState>>;

/// Shared handle to the created window; the runner fills it on first resume.
pub type SharedWindow = Rc<RefCell<Option<Arc<Window>>>>;
