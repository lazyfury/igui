//! Platform-neutral presentation: the graphics boundary and the opaque bridge
//! to a native platform.
//!
//! `quill_app` never names a windowing or GPU type. A platform plugin (e.g.
//! `quill_winit`) drives the runtime through the opaque [`PlatformEvent`] /
//! [`PlatformContext`] bridge and installs a [`Presenter`] that turns the app's
//! `DrawList` into pixels.

use std::any::Any;

use draw_core::{EventResult, InputEvent, ViewportSize};
use draw_render::DrawList;

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

/// An opaque native platform context (e.g. winit's `ActiveEventLoop`), passed
/// to lifecycle observers that need to create the window.
pub struct PlatformContext<'a> {
    context: &'a mut (dyn Any + 'static),
}

impl<'a> PlatformContext<'a> {
    pub fn new<T: Any>(context: &'a mut T) -> Self {
        Self { context }
    }

    /// A short-lived reborrow so several observers can be visited in turn.
    pub fn reborrow(&mut self) -> PlatformContext<'_> {
        PlatformContext {
            context: &mut *self.context,
        }
    }

    /// The native context, if it is of type `T`.
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.context.downcast_mut::<T>()
    }
}

impl std::fmt::Debug for PlatformContext<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PlatformContext(..)")
    }
}

/// Observes native events and turns them into core [`InputEvent`]s.
pub trait PlatformObserver: 'static {
    /// Appends any core input events this platform event produces.
    fn on_platform(&mut self, event: PlatformEvent<'_>, out: &mut Vec<InputEvent>);
}

/// Observes platform lifecycle: window creation on first resume, and teardown.
///
/// A lifecycle observer runs outside the plugin registry, so it is handed the
/// [`App`](crate::App) and may install a [`Presenter`] or a service once the
/// window / surface actually exist.
pub trait LifecycleObserver: 'static {
    /// The platform is ready; create the window / surface here.
    fn resumed(&mut self, _context: PlatformContext<'_>, _app: &mut crate::app::App) {}

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
        paint: &mut draw_render::PaintContext,
    );
}

/// The platform driver installed by a platform plugin.
pub type Runner = Box<dyn FnOnce(crate::app::App)>;
