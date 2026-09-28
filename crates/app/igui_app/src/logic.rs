//! The application logic: the update / layout / paint phases and their
//! read-only service contexts.
//!
//! An app implements [`AppLogic`]; the runtime drives it. The phases stay
//! separate so the host can record a stage timing for each (the profiler's
//! `StageTimes`), exactly as the pipeline rule requires:
//! `Update -> Layout -> Paint -> DrawList`.

use std::any::Any;

use igui_core::{Cursor, InputEvent, Rect, ViewportSize};
use igui_render::PaintContext;

use crate::service::ServiceMap;

/// Whether the app consumed an event (re-exported from the core).
pub use igui_core::EventResult;

/// Read-only services, available during [`AppLogic::init`].
pub struct InitContext<'a> {
    viewport: ViewportSize,
    services: &'a ServiceMap,
}

impl<'a> InitContext<'a> {
    pub(crate) fn new(viewport: ViewportSize, services: &'a ServiceMap) -> Self {
        Self { viewport, services }
    }

    pub fn viewport(&self) -> ViewportSize {
        self.viewport
    }

    pub fn service<T: Any>(&self) -> Option<&T> {
        self.services.get::<T>()
    }
}

/// Read-only services, available during [`AppLogic::event`].
pub struct EventContext<'a> {
    services: &'a ServiceMap,
}

impl<'a> EventContext<'a> {
    pub(crate) fn new(services: &'a ServiceMap) -> Self {
        Self { services }
    }

    pub fn service<T: Any>(&self) -> Option<&T> {
        self.services.get::<T>()
    }
}

/// Read-only frame state and services, available during [`AppLogic::update`],
/// [`AppLogic::layout`] and [`AppLogic::paint`].
pub struct FrameContext<'a> {
    delta: f32,
    viewport: ViewportSize,
    services: &'a ServiceMap,
}

impl<'a> FrameContext<'a> {
    pub(crate) fn new(delta: f32, viewport: ViewportSize, services: &'a ServiceMap) -> Self {
        Self {
            delta,
            viewport,
            services,
        }
    }

    /// Seconds since the previous frame, capped by the app config.
    pub fn delta(&self) -> f32 {
        self.delta
    }

    /// The drawing area in logical pixels.
    pub fn viewport(&self) -> ViewportSize {
        self.viewport
    }

    pub fn service<T: Any>(&self) -> Option<&T> {
        self.services.get::<T>()
    }
}

/// The application's own logic, called by the runtime.
///
/// Every method has a default, so an app overrides only what it needs. The
/// frame methods run in a fixed order: `update`, then `layout`, then `paint`.
pub trait AppLogic: 'static {
    /// Called once the platform is ready, before the first frame.
    fn init(&mut self, _ctx: &InitContext<'_>) {}

    /// Called for each core input event the input layers did not consume.
    fn event(&mut self, _ctx: &EventContext<'_>, _event: &InputEvent) -> EventResult {
        EventResult::Ignored
    }

    /// Advances time-driven state.
    fn update(&mut self, _ctx: &FrameContext<'_>) {}

    /// Resolves layout for the current viewport.
    fn layout(&mut self, _ctx: &FrameContext<'_>) {}

    /// Emits this frame's draw commands.
    fn paint(&mut self, _ctx: &FrameContext<'_>, _paint: &mut PaintContext) {}

    /// Whether the app wants another frame (animation, transient UI).
    fn needs_frame(&self) -> bool {
        false
    }

    /// The cursor this frame should show, if any.
    fn cursor(&self) -> Option<Cursor> {
        None
    }

    /// The focused caret for IME placement, if any.
    fn caret(&self) -> Option<Rect> {
        None
    }
}
