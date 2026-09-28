//! The wgpu demo: the shared [`demo_app::DemoApp`] plus a text measurer injected
//! by [`quill_winit::TextMeasurePlugin`].
//!
//! The shared app owns scene/UI/layout/input. Here we only wrap the backend's
//! `FontMetrics` in a `draw_ui::TextMeasurer` (via the plugin), so the layout
//! engine measures text with the exact advances the backend renders with.

use std::cell::RefCell;
use std::rc::Rc;

use draw_core::{EventResult, InputEvent, ViewportSize};
use draw_render::PaintContext;
use draw_ui::TextMeasurer;

use demo_app::DemoApp;

/// Application state owned by the window runner.
pub struct Demo {
    app: DemoApp,
}

impl Default for Demo {
    fn default() -> Self {
        Self::new()
    }
}

impl Demo {
    pub fn new() -> Self {
        Self {
            app: DemoApp::new(),
        }
    }

    /// Injects the layout measurer (the backend's real font metrics) into the
    /// layout engine.
    pub fn set_text_measurer(&mut self, measurer: Rc<dyn TextMeasurer>) {
        self.app.set_text_measurer(measurer);
    }

    /// Reserves extra top padding on the sidebar for a transparent title bar
    /// (macOS traffic lights). See [`DemoApp::set_titlebar_inset`].
    pub fn set_titlebar_inset(&mut self, inset: f32) {
        self.app.set_titlebar_inset(inset);
    }

    /// Installs the host clipboard for the gallery's text fields.
    pub fn set_clipboard(&mut self, clipboard: Rc<RefCell<dyn draw_ui::Clipboard>>) {
        self.app.set_clipboard(clipboard);
    }

    /// Advances the animation and updates text for the new viewport.
    ///
    /// UI layout is deliberately *not* performed here: the host times it as a
    /// separate pipeline phase via [`Demo::layout`].
    pub fn update(&mut self, viewport: ViewportSize, dt: f32) {
        self.app.update(viewport, dt);
    }

    /// Resolves UI layout for `viewport` (the timed *layout* pipeline phase).
    pub fn layout(&mut self, viewport: ViewportSize) {
        self.app.layout(viewport);
    }

    /// Emits this frame's `DrawList` into `ctx`.
    pub fn paint(&self, ctx: &mut PaintContext) {
        self.app.paint(ctx);
    }

    /// Routes a backend-neutral input event through the UI.
    pub fn event(&mut self, event: &InputEvent) -> EventResult {
        self.app.event(event)
    }

    /// The scene tree shared by world and UI nodes.
    pub fn tree(&self) -> &draw_scene::SceneTree {
        self.app.tree()
    }

    /// Controls in the demo UI.
    pub fn control_count(&self) -> usize {
        self.app.control_count()
    }

    /// Cursor the host should show for the current pointer position.
    pub fn cursor(&self) -> draw_core::Cursor {
        self.app.cursor()
    }

    /// Whether the app still has work for another frame (animation, transient
    /// overlays, pending layout/paint). The host schedules a redraw while this
    /// is `true` and sleeps otherwise.
    pub fn needs_frame(&self) -> bool {
        self.app.needs_frame()
    }
}
