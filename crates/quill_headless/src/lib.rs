//! `quill_headless` — a recording [`Presenter`] for the `quill_app` runtime.
//!
//! It runs the **same** application logic as a window host with no window and
//! no `winit`: each frame's `DrawList` is captured by
//! [`draw_backend_recording::RecordingBackend`], which is how a `--selfcheck`
//! verifies real rendering without a screenshot.
//!
//! ```no_run
//! # use quill_app::{App, AppConfig};
//! # struct MyApp;
//! # impl quill_app::AppLogic for MyApp {}
//! let mut app = App::new(AppConfig::default())
//!     .plugin(quill_headless::HeadlessPlugin::new(1280.0, 800.0))
//!     .logic(MyApp)
//!     .build();
//! let recording = app
//!     .services()
//!     .get::<quill_headless::RecordingHandle>()
//!     .unwrap()
//!     .0
//!     .clone();
//! app.run_headless(1);
//! assert!(recording.borrow().frame_count() > 0);
//! ```
//!
//! This crate has **no `winit` dependency**; it depends only on the runtime and
//! the recording backend, so a headless check can run anywhere.

use std::cell::RefCell;
use std::rc::Rc;

use draw_backend_recording::RecordingBackend;
use draw_core::{Size, ViewportSize};
use draw_render::{DrawList, RenderBackend};
use quill_app::{AppBuilder, Plugin, PresentOutcome, Presenter};

/// Shared handle to the frames the headless presenter recorded.
///
/// Obtain it from the app's services before running:
/// `app.services().get::<RecordingHandle>().unwrap().0.clone()`.
#[derive(Clone)]
pub struct RecordingHandle(pub Rc<RefCell<RecordingBackend>>);

/// Registers a [`HeadlessPresenter`]: every frame is recorded into
/// [`RecordingBackend`] instead of being presented to a surface.
pub struct HeadlessPlugin {
    viewport: ViewportSize,
}

impl HeadlessPlugin {
    /// Records frames at a fixed `width` x `height` logical viewport.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            viewport: ViewportSize::new(Size::new(width, height)),
        }
    }
}

impl Default for HeadlessPlugin {
    fn default() -> Self {
        Self::new(1280.0, 800.0)
    }
}

impl Plugin for HeadlessPlugin {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn build(&self, app: &mut AppBuilder) {
        let recording = Rc::new(RefCell::new(RecordingBackend::new()));
        app.insert_service(RecordingHandle(recording.clone()));
        app.set_presenter(HeadlessPresenter {
            recording,
            viewport: self.viewport,
        });
    }
}

struct HeadlessPresenter {
    recording: Rc<RefCell<RecordingBackend>>,
    viewport: ViewportSize,
}

impl Presenter for HeadlessPresenter {
    fn viewport(&self) -> ViewportSize {
        self.viewport
    }

    fn present(&mut self, list: &DrawList) -> PresentOutcome {
        let mut backend = self.recording.borrow_mut();
        let _ = backend.begin_frame(self.viewport);
        let _ = backend.submit(list);
        let _ = backend.end_frame();
        PresentOutcome::Presented
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use draw_core::Vec2;
    use draw_render::{Paint, PaintContext};
    use quill_app::{App, AppConfig, AppLogic, FrameContext};

    struct OneLine;

    impl AppLogic for OneLine {
        fn paint(&mut self, _ctx: &FrameContext<'_>, paint: &mut PaintContext) {
            paint.draw_line(Vec2::ZERO, Vec2::new(10.0, 10.0), 1.0, Paint::default());
        }
    }

    #[test]
    fn headless_presenter_records_each_frame() {
        let mut app = App::new(AppConfig::default())
            .plugin(HeadlessPlugin::new(640.0, 480.0))
            .logic(OneLine)
            .build();
        let recording = app.services().get::<RecordingHandle>().unwrap().0.clone();

        app.run_headless(2);

        let recording = recording.borrow();
        assert_eq!(recording.frame_count(), 2);
        assert_eq!(recording.last_frame().unwrap().command_count(), 1);
    }
}
