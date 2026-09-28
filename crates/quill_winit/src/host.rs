//! The shared window host: window + surface + backend lifecycle, platform
//! input translation, IME wiring and a frame render.
//!
//! A host owns its own application state and [`ApplicationHandler`]; this layer
//! provides the platform services it used to copy in every demo:
//!
//! - window / surface / `wgpu` backend / swap-chain creation on first resume;
//! - resize and scale-factor handling;
//! - platform event → [`InputEvent`] translation (including IME and committed
//!   text);
//! - presenting a `DrawList` with surface-loss recovery;
//! - IME activation and candidate-window placement from the UI caret.
//!
//! ```ignore
//! struct App { host: quill_winit::Host, view: View }
//!
//! impl ApplicationHandler for App {
//!     fn resumed(&mut self, loop_: &ActiveEventLoop) { self.host.resumed(loop_); }
//!     fn window_event(&mut self, loop_: &ActiveEventLoop, _, event: WindowEvent) {
//!         match event {
//!             WindowEvent::CloseRequested => loop_.exit(),
//!             WindowEvent::Resized(size) => self.host.handle_resize(size.width, size.height),
//!             WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.host.handle_scale_factor(scale_factor),
//!             WindowEvent::RedrawRequested => {
//!                 self.view.layout(&mut tree, self.host.viewport());
//!                 let list = /* paint */;
//!                 self.host.render(&list);
//!                 self.host.sync_ime(&self.view.tree());
//!             }
//!             _ => {
//!                 for event in self.host.translate(&event) { self.view.event(&event); }
//!                 self.host.request_redraw();
//!             }
//!         }
//!     }
//! }
//! ```

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use draw_backend_wgpu::{wgpu, FontConfig, WgpuBackend};
use draw_core::{
    Color, Cursor, ImeEvent, InputEvent, PointerButton, Rect, Size, Vec2, ViewportSize,
};
use draw_render::{DrawList, RenderBackend};
use draw_scene::SceneTree;
use draw_ui::Clipboard;
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{CursorIcon, Window},
};

use crate::clipboard::SystemClipboard;
use crate::input;

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

/// Everything needed to create the window and its backend.
#[derive(Debug, Clone)]
pub struct HostOptions {
    pub title: String,
    /// Window size in logical pixels.
    pub size: (f64, f64),
    pub titlebar: TitlebarMode,
    pub present_mode: wgpu::PresentMode,
    pub power_preference: wgpu::PowerPreference,
    pub clear_color: Color,
    pub font: FontConfig,
    /// Whether the platform IME is enabled for this window.
    pub ime: bool,
}

impl Default for HostOptions {
    fn default() -> Self {
        Self {
            title: "quill".into(),
            size: (1200.0, 780.0),
            titlebar: TitlebarMode::Native,
            present_mode: wgpu::PresentMode::Fifo,
            power_preference: wgpu::PowerPreference::HighPerformance,
            clear_color: Color::new(0.039, 0.039, 0.039, 1.0),
            font: FontConfig::default(),
            ime: false,
        }
    }
}

/// What [`Host::render`] did with the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutcome {
    /// The frame reached the swap chain.
    Presented,
    /// No frame was drawn (zero-sized window, surface timeout, ...).
    Skipped,
    /// The surface was lost/outdated and has been reconfigured; try again.
    Reconfigured,
}

/// Owns the window, surface, backend and swap-chain configuration.
pub struct Host {
    instance: wgpu::Instance,
    options: HostOptions,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    backend: Option<WgpuBackend>,
    config: Option<wgpu::SurfaceConfiguration>,
    scale_factor: f64,
    cursor: Vec2,
    /// The platform IME is enabled (a composition may start).
    ime_active: bool,
    /// A composition (preedit) is in progress: the platform owns the text, so
    /// committed `KeyboardInput` text must be ignored until it ends.
    composing: bool,
    /// Left-button press tracker for double-click detection.
    double_click: input::DoubleClickTracker,
    /// System clipboard (text-field copy / cut / paste).
    clipboard: Rc<RefCell<dyn Clipboard>>,
}

impl Host {
    /// Creates the host; the window is created on the first [`resumed`].
    ///
    /// [`resumed`]: Host::resumed
    pub fn new(options: HostOptions) -> Self {
        Self {
            instance: wgpu::Instance::default(),
            options,
            window: None,
            surface: None,
            backend: None,
            config: None,
            scale_factor: 1.0,
            cursor: Vec2::ZERO,
            ime_active: false,
            composing: false,
            double_click: input::DoubleClickTracker::new(),
            clipboard: Rc::new(RefCell::new(SystemClipboard::new())),
        }
    }

    /// The shared system clipboard, to install into a UI tree with
    /// [`draw_ui::set_clipboard`].
    pub fn clipboard(&self) -> Rc<RefCell<dyn Clipboard>> {
        self.clipboard.clone()
    }

    /// Creates the window, surface, backend and swap chain.
    ///
    /// Returns `false` if they already existed (a second `resumed`).
    pub fn resumed(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.window.is_some() {
            return false;
        }
        let mut attributes = Window::default_attributes()
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(self.options.size.0, self.options.size.1));
        attributes = match self.options.titlebar {
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
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("create surface");

        let mut backend = WgpuBackend::from_instance(
            &self.instance,
            Some(&surface),
            self.options.power_preference,
        )
        .expect("create wgpu backend");

        // Prefer a non-sRGB format so the unorm colors written by the shader
        // match the Canvas backend; fall back to whatever the surface offers.
        let capabilities = surface.get_capabilities(backend.adapter());
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: self.options.present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: Vec::new(),
        };
        surface.configure(backend.device(), &config);

        self.scale_factor = window.scale_factor();
        backend.set_scale_factor(self.scale_factor as f32);
        backend.set_clear_color(self.options.clear_color);
        if let Err(error) = backend.set_font_config(self.options.font.clone()) {
            eprintln!("quill_winit: font setup failed, using fallback: {error}");
        }

        if self.options.ime {
            window.set_ime_allowed(true);
        }

        self.window = Some(window);
        self.surface = Some(surface);
        self.backend = Some(backend);
        self.config = Some(config);
        true
    }

    /// The window, once [`resumed`](Host::resumed) has run.
    pub fn window(&self) -> Option<&Arc<Window>> {
        self.window.as_ref()
    }

    /// The rendering backend, once [`resumed`](Host::resumed) has run.
    pub fn backend(&self) -> Option<&WgpuBackend> {
        self.backend.as_ref()
    }

    /// The mutable rendering backend (font config, clear color, ...).
    pub fn backend_mut(&mut self) -> Option<&mut WgpuBackend> {
        self.backend.as_mut()
    }

    /// Replaces the clear color (a theme swap).
    pub fn set_clear_color(&mut self, color: Color) {
        self.options.clear_color = color;
        if let Some(backend) = self.backend.as_mut() {
            backend.set_clear_color(color);
        }
    }

    /// The current scale factor.
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// The viewport in logical pixels.
    pub fn viewport(&self) -> ViewportSize {
        match self.config.as_ref() {
            Some(config) => ViewportSize::new(Size::new(
                config.width as f32 / self.scale_factor as f32,
                config.height as f32 / self.scale_factor as f32,
            )),
            None => ViewportSize::new(Size::ZERO),
        }
    }

    /// The last pointer position in logical pixels.
    pub fn cursor(&self) -> Vec2 {
        self.cursor
    }

    /// Resizes the swap chain (no-op for a zero dimension).
    pub fn handle_resize(&mut self, width: u32, height: u32) {
        let (Some(surface), Some(backend), Some(config)) = (
            self.surface.as_ref(),
            self.backend.as_ref(),
            self.config.as_mut(),
        ) else {
            return;
        };
        if width == 0 || height == 0 {
            return;
        }
        config.width = width;
        config.height = height;
        surface.configure(backend.device(), config);
    }

    /// Applies a scale-factor change.
    pub fn handle_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        if let Some(backend) = self.backend.as_mut() {
            backend.set_scale_factor(scale_factor as f32);
        }
    }

    /// Schedules one frame.
    pub fn request_redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// Applies the core cursor to the OS window.
    pub fn apply_cursor(&self, cursor: Cursor) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let icon = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::ColResize => CursorIcon::ColResize,
            Cursor::RowResize => CursorIcon::RowResize,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
        };
        window.set_cursor(icon);
    }

    /// Translates one platform event into zero or more core input events.
    ///
    /// Window lifecycle events (`Resized`, `CloseRequested`, ...) are handled
    /// by the caller directly; this returns an empty list for them.
    pub fn translate(&mut self, event: &WindowEvent) -> Vec<InputEvent> {
        let mut out = Vec::new();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = input::to_logical(*position, self.scale_factor);
                out.push(InputEvent::PointerMove {
                    position: self.cursor,
                });
            }
            WindowEvent::CursorLeft { .. } => out.push(InputEvent::PointerLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = input::pointer_button(*button);
                let position = self.cursor;
                match state {
                    ElementState::Pressed => {
                        out.push(InputEvent::PointerDown { position, button });
                        if button == PointerButton::Left && self.double_click.press(position) {
                            out.push(InputEvent::DoubleClick { position });
                        }
                    }
                    ElementState::Released => out.push(InputEvent::PointerUp { position, button }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = input::wheel_pixels(*delta, self.scale_factor as f32);
                out.push(InputEvent::Wheel {
                    position: self.cursor,
                    delta: Vec2::new(0.0, delta),
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                out.push(InputEvent::ModifiersChanged(input::modifiers(
                    modifiers.state(),
                )));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(key) = input::map_key(&event.logical_key) {
                    out.push(match event.state {
                        ElementState::Pressed => InputEvent::KeyDown { key },
                        ElementState::Released => InputEvent::KeyUp { key },
                    });
                }
                // Committed text, but not while an IME composition owns the
                // keyboard (the platform delivers that through `Ime`). Merely
                // having the IME *enabled* must not drop plain keys: with a CJK
                // IME active a space outside a composition is ordinary text.
                // Space and Tab are named keys whose `text` is not always set,
                // so derive it from the logical key. Tab is allowed through the
                // control-character filter (text fields insert it).
                if event.state == ElementState::Pressed && !self.composing {
                    if let Some(text) =
                        input::committed_text(&event.logical_key, event.text.as_deref())
                    {
                        out.push(InputEvent::TextInput { text });
                    }
                }
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {
                    self.ime_active = true;
                    out.push(InputEvent::Ime(ImeEvent::Enabled));
                }
                Ime::Disabled => {
                    self.ime_active = false;
                    self.composing = false;
                    out.push(InputEvent::Ime(ImeEvent::Disabled));
                }
                Ime::Preedit(text, cursor) => {
                    // An empty preedit means the composition was cleared.
                    self.composing = !text.is_empty();
                    out.push(InputEvent::Ime(ImeEvent::Preedit {
                        text: text.clone(),
                        cursor: *cursor,
                    }));
                }
                Ime::Commit(text) => {
                    self.composing = false;
                    out.push(InputEvent::Ime(ImeEvent::Commit(text.clone())));
                }
            },
            _ => {}
        }
        out
    }

    /// Whether the platform IME is enabled.
    pub fn is_ime_active(&self) -> bool {
        self.ime_active
    }

    /// Whether an IME composition (preedit) is currently in progress.
    pub fn is_composing(&self) -> bool {
        self.composing
    }

    /// Enables or disables the platform IME for this window.
    pub fn set_ime_allowed(&self, allowed: bool) {
        if let Some(window) = self.window.as_ref() {
            window.set_ime_allowed(allowed);
        }
    }

    /// Places the IME candidate window at `rect` (logical viewport coordinates).
    pub fn set_ime_cursor_area(&self, rect: Option<Rect>) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let scale = if self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        let (position, size) = match rect {
            Some(rect) => (
                PhysicalPosition::new(rect.left() as f64 * scale, rect.top() as f64 * scale),
                PhysicalSize::new(
                    (rect.size.width.max(1.0) as f64) * scale,
                    (rect.size.height.max(1.0) as f64) * scale,
                ),
            ),
            None => (PhysicalPosition::new(0.0, 0.0), PhysicalSize::new(0.0, 0.0)),
        };
        window.set_ime_cursor_area(position, size);
    }

    /// Places the IME candidate window at the focused control's caret in `tree`.
    ///
    /// Call after layout/paint each frame; it reads
    /// [`draw_ui::focused_caret`](draw_ui::focused_caret).
    pub fn sync_ime(&self, tree: &SceneTree) {
        if !self.options.ime {
            return;
        }
        self.set_ime_cursor_area(draw_ui::focused_caret(tree));
    }

    /// Renders `list` to the swap chain, presenting it.
    pub fn render(&mut self, list: &DrawList) -> RenderOutcome {
        let viewport = self.viewport();
        let (Some(surface), Some(backend), Some(config)) = (
            self.surface.as_ref(),
            self.backend.as_mut(),
            self.config.as_ref(),
        ) else {
            return RenderOutcome::Skipped;
        };

        let texture = match surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(backend.device(), config);
                return RenderOutcome::Reconfigured;
            }
            Err(wgpu::SurfaceError::Timeout) => return RenderOutcome::Skipped,
            Err(error) => {
                eprintln!("quill_winit: surface error: {error}");
                return RenderOutcome::Skipped;
            }
        };

        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if backend
            .begin_frame_with_view(view, config.width, config.height, config.format, viewport)
            .is_ok()
        {
            let _ = backend.submit(list);
            let _ = backend.end_frame();
        }
        texture.present();
        RenderOutcome::Presented
    }
}

/// A frame-to-frame delta clock (capped so a paused app does not jump).
pub struct FrameClock {
    last: Instant,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock {
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
        }
    }

    /// Seconds since the previous tick, capped at 100 ms.
    pub fn tick(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::event::Ime;

    #[test]
    fn ime_events_translate_and_track_activation() {
        let mut host = Host::new(HostOptions::default());
        assert_eq!(
            host.translate(&WindowEvent::Ime(Ime::Enabled)),
            vec![InputEvent::Ime(ImeEvent::Enabled)]
        );
        assert!(host.is_ime_active());
        assert!(!host.is_composing());
        assert_eq!(
            host.translate(&WindowEvent::Ime(Ime::Preedit("ni".into(), Some((1, 1))))),
            vec![InputEvent::Ime(ImeEvent::Preedit {
                text: "ni".into(),
                cursor: Some((1, 1)),
            })]
        );
        assert!(host.is_composing());
        assert_eq!(
            host.translate(&WindowEvent::Ime(Ime::Commit("你".into()))),
            vec![InputEvent::Ime(ImeEvent::Commit("你".into()))]
        );
        assert!(!host.is_composing());
        // An empty preedit also clears the composition flag.
        host.translate(&WindowEvent::Ime(Ime::Preedit("x".into(), None)));
        host.translate(&WindowEvent::Ime(Ime::Preedit(String::new(), None)));
        assert!(!host.is_composing());
        assert_eq!(
            host.translate(&WindowEvent::Ime(Ime::Disabled)),
            vec![InputEvent::Ime(ImeEvent::Disabled)]
        );
        assert!(!host.is_ime_active());
    }
}
