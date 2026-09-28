//! Platform-neutral presentation: the graphics boundary and the opaque bridge
//! to a native platform.
//!
//! `rough_app` never names a windowing or GPU type. A platform plugin (e.g.
//! `rough_winit`) drives the runtime through the opaque [`PlatformEvent`]
//! bridge and installs a [`Presenter`] that turns the app's `DrawList` into
//! pixels.

use std::any::Any;

use rough_core::{EventResult, InputEvent, ViewportSize};
use rough_render::DrawList;

/// What [`Presenter::present`] did with a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentOutcome {
    /// The frame reached its target.
    Presented,
    /// No frame was drawn (zero-sized target, timeout, ...).
    Skipped,
    /// The target was lost/outdated and has been reconfigured; try again.
    Reconfigured,
}

/// The graphics boundary. The runtime hands a [`DrawList`] to the presenter; a
/// window host presents it to a surface, a headless host records it.
pub trait Presenter {
    /// The drawing area in logical pixels.
    fn viewport(&self) -> ViewportSize;

    /// Presents one frame.
    fn present(&mut self, list: &DrawList) -> PresentOutcome;
}

/// An opaque native platform event.
///
/// The platform plugin constructs it around a native event type (e.g. a
/// `winit::event::WindowEvent`); a platform observer downcasts it back to the
/// type it expects. The runtime never knows the type.
///
/// The borrowed platform loop (winit's `ActiveEventLoop`) is deliberately **not**
/// carried through this type: a borrowed loop is not `Any + 'static`. A platform
/// runner publishes what it created (e.g. an `Arc<Window>`) into the
/// [`ServiceMap`](crate::ServiceMap) before calling
/// [`App::resumed`](crate::App::resumed), and lifecycle observers read it back.
#[derive(Clone, Copy)]
pub struct PlatformEvent<'a> {
    event: &'a (dyn Any + 'static),
}

impl<'a> PlatformEvent<'a> {
    pub fn new<T: Any>(event: &'a T) -> Self {
        Self { event }
    }

    /// The native event, if it is of type `T`.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.event.downcast_ref::<T>()
    }
}

impl std::fmt::Debug for PlatformEvent<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PlatformEvent(..)")
    }
}

/// Observes native events and turns them into core [`InputEvent`]s.
pub trait PlatformObserver: 'static {
    /// Appends any core input events this platform event produces.
    fn on_platform(&mut self, event: PlatformEvent<'_>, out: &mut Vec<InputEvent>);
}

/// Observes platform lifecycle: window creation on first resume, and teardown.
///
/// A lifecycle observer is handed the [`App`](crate::App) and may read the
/// window/service a platform runner published, then install a [`Presenter`] or
/// a service.
pub trait LifecycleObserver: 'static {
    /// The platform is ready; create the surface / backend here.
    fn resumed(&mut self, _app: &mut crate::app::App) {}

    /// The platform is going away.
    fn suspended(&mut self, _app: &mut crate::app::App) {}
}

/// A layer that may consume a core input event before the app logic sees it.
///
/// Layers are visited in registration order (an overlay before the app); the
/// first [`EventResult::Handled`] stops routing.
pub trait InputLayer: 'static {
    fn on_input(&mut self, event: &InputEvent) -> EventResult;
}

/// A layer painted on top of the app logic each frame.
pub trait PaintLayer: 'static {
    fn paint(
        &mut self,
        ctx: &crate::logic::FrameContext<'_>,
        paint: &mut rough_render::PaintContext,
    );
}

/// Runs after each frame is presented, with the frame's `DrawList`.
///
/// Platform plugins use it for work that needs the frame's result without being
/// part of painting — placing the IME candidate window at the app's current
/// caret ([`App::caret`](crate::App::caret)), or recording/inspecting the frame
/// that was just presented.
pub trait FrameObserver: 'static {
    fn after_frame(&mut self, app: &crate::app::App, list: &DrawList);
}

/// The platform driver installed by a platform plugin.
pub type Runner = Box<dyn FnOnce(crate::app::App)>;
