//! The window plugin: winit event loop, window creation and lifecycle.
//!
//! [`WinitPlugin`] owns the platform loop. On first resume it creates the
//! window, publishes it as [`SharedWindow`] and sets the initial scale, then
//! calls [`App::resumed`](igui_app::App::resumed) so the graphics / IME
//! lifecycle observers can build on it. It never touches `wgpu`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use igui_app::{App, AppBuilder, PlatformEvent, PlatformObserver, Plugin};
use igui_core::InputEvent;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

use crate::input;
use crate::window::{SharedWindow, SharedWindowState, TitlebarMode, WindowConfig, WindowState};

/// The winit window plugin.
///
/// Register it before the graphics / input plugins so it publishes the window
/// and scale services they depend on.
pub struct WinitPlugin {
    config: WindowConfig,
}

impl WinitPlugin {
    pub fn new(config: WindowConfig) -> Self {
        Self { config }
    }
}

impl Default for WinitPlugin {
    fn default() -> Self {
        Self::new(WindowConfig::default())
    }
}

impl Plugin for WinitPlugin {
    fn name(&self) -> &'static str {
        "winit"
    }

    fn build(&self, app: &mut AppBuilder) {
        let state: SharedWindowState = Rc::new(RefCell::new(WindowState::default()));
        app.insert_service(self.config.clone());
        app.insert_service(state.clone());
        app.insert_service(SharedWindow::default());
        app.add_platform_observer(WindowStateObserver { state });
        app.set_runner(Box::new(run_event_loop));
    }
}

/// Keeps the shared scale factor current.
struct WindowStateObserver {
    state: SharedWindowState,
}

impl PlatformObserver for WindowStateObserver {
    fn on_platform(&mut self, event: PlatformEvent<'_>, _out: &mut Vec<InputEvent>) {
        if let Some(WindowEvent::ScaleFactorChanged { scale_factor, .. }) =
            event.downcast_ref::<WindowEvent>()
        {
            self.state.borrow_mut().scale_factor = *scale_factor;
        }
    }
}

fn run_event_loop(app: App) {
    let event_loop = EventLoop::new().expect("igui_winit: create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut handler = WinitHandler { app, window: None };
    event_loop
        .run_app(&mut handler)
        .expect("igui_winit: run event loop");
}

struct WinitHandler {
    app: App,
    window: Option<Arc<Window>>,
}

impl WinitHandler {
    fn request_redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn apply_cursor(&self) {
        if let (Some(window), Some(cursor)) = (self.window.as_ref(), self.app.cursor()) {
            window.set_cursor(input::cursor_icon(cursor));
        }
    }
}

impl ApplicationHandler for WinitHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let config = self
                .app
                .services()
                .get::<WindowConfig>()
                .cloned()
                .unwrap_or_default();
            let window = Arc::new(
                event_loop
                    .create_window(window_attributes(&config))
                    .expect("igui_winit: create window"),
            );
            if let Some(state) = self.app.services().get::<SharedWindowState>() {
                state.borrow_mut().scale_factor = window.scale_factor();
            }
            if let Some(shared) = self.app.services().get::<SharedWindow>() {
                *shared.borrow_mut() = Some(window.clone());
            }
            self.window = Some(window);
        }
        self.app.resumed();
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // A redraw is the frame itself; do not route it as input.
        if matches!(event, WindowEvent::RedrawRequested) {
            self.app.frame();
            self.apply_cursor();
            if self.app.needs_frame() {
                self.request_redraw();
            }
            return;
        }

        if matches!(event, WindowEvent::CloseRequested) {
            self.app.suspended();
            event_loop.exit();
            return;
        }

        self.app.platform_event(PlatformEvent::new(&event));

        // Any event may have changed the UI; schedule exactly one frame.
        self.request_redraw();
    }
}

fn window_attributes(config: &WindowConfig) -> WindowAttributes {
    let mut attributes = Window::default_attributes()
        .with_title(config.title.clone())
        .with_inner_size(LogicalSize::new(config.size.0, config.size.1));
    attributes = match config.titlebar {
        TitlebarMode::Native => attributes,
        TitlebarMode::Hidden => attributes.with_decorations(false),
        TitlebarMode::Transparent => {
            #[cfg(target_os = "macos")]
            {
                attributes
                    .with_titlebar_transparent(true)
                    .with_fullsize_content_view(true)
                    .with_title_hidden(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                attributes
            }
        }
    };
    attributes
}
