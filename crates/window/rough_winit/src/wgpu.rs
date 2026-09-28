//! The graphics plugin: surface, wgpu backend, swap chain and presentation.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use rough_app::{
    App, AppBuilder, LifecycleObserver, PlatformEvent, PlatformObserver, Plugin, PresentOutcome,
    Presenter,
};
use rough_backend_wgpu::{wgpu, FontConfig, WgpuBackend};
use rough_core::{Color, InputEvent, Size, ViewportSize};
use rough_render::{DrawList, RenderBackend};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::window::{SharedWindow, SharedWindowState};

/// Everything the graphics plugin needs besides the window.
#[derive(Debug, Clone)]
pub struct GpuConfig {
    pub present_mode: wgpu::PresentMode,
    pub power_preference: wgpu::PowerPreference,
    pub clear_color: Color,
    pub font: FontConfig,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            present_mode: wgpu::PresentMode::Fifo,
            power_preference: wgpu::PowerPreference::HighPerformance,
            clear_color: Color::new(0.039, 0.039, 0.039, 1.0),
            font: FontConfig::default(),
        }
    }
}

/// The wgpu backend, once created. Shared with [`TextMeasurePlugin`](crate::TextMeasurePlugin)
/// and any app that needs GPU work (texture upload, offscreen targets).
pub type SharedBackend = Rc<RefCell<WgpuBackend>>;

struct WgpuState {
    #[allow(dead_code)]
    window: Arc<Window>,
    #[allow(dead_code)]
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    backend: SharedBackend,
}

impl WgpuState {
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface
            .configure(self.backend.borrow().device(), &self.config);
    }
}

type SharedWgpuState = Rc<RefCell<Option<WgpuState>>>;

/// The wgpu rendering plugin: creates the surface / backend on first resume and
/// presents each frame's `DrawList` to the swap chain.
///
/// Requires [`WinitPlugin`](crate::WinitPlugin). Register it before
/// [`TextMeasurePlugin`](crate::TextMeasurePlugin) so the shared backend exists
/// when text metrics are read.
pub struct WgpuPlugin {
    config: GpuConfig,
}

impl WgpuPlugin {
    pub fn new(config: GpuConfig) -> Self {
        Self { config }
    }
}

impl Default for WgpuPlugin {
    fn default() -> Self {
        Self::new(GpuConfig::default())
    }
}

impl Plugin for WgpuPlugin {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn build(&self, app: &mut AppBuilder) {
        let window = app.service::<SharedWindow>().cloned().unwrap_or_default();
        let window_state = app
            .service::<SharedWindowState>()
            .cloned()
            .unwrap_or_default();
        let state: SharedWgpuState = Rc::new(RefCell::new(None));
        app.add_lifecycle_observer(WgpuLifecycle {
            window,
            window_state,
            config: self.config.clone(),
            state: state.clone(),
        });
        app.add_platform_observer(WgpuResize { state });
    }
}

struct WgpuLifecycle {
    window: SharedWindow,
    window_state: SharedWindowState,
    config: GpuConfig,
    state: SharedWgpuState,
}

impl LifecycleObserver for WgpuLifecycle {
    fn resumed(&mut self, app: &mut App) {
        if self.state.borrow().is_some() {
            return;
        }
        let Some(window) = self.window.borrow().clone() else {
            return;
        };

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("rough_winit: create surface");
        let mut backend =
            WgpuBackend::from_instance(&instance, Some(&surface), self.config.power_preference)
                .expect("rough_winit: create wgpu backend");

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
            present_mode: self.config.present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: Vec::new(),
        };
        surface.configure(backend.device(), &config);

        let scale = self.window_state.borrow().scale_factor;
        backend.set_scale_factor(if scale > 0.0 { scale as f32 } else { 1.0 });
        backend.set_clear_color(self.config.clear_color);
        if let Err(error) = backend.set_font_config(self.config.font.clone()) {
            eprintln!("rough_winit: font setup failed, using fallback: {error}");
        }

        let backend: SharedBackend = Rc::new(RefCell::new(backend));
        app.services_mut().insert(backend.clone());
        *self.state.borrow_mut() = Some(WgpuState {
            window,
            instance,
            surface,
            config,
            backend,
        });
        app.set_presenter(WgpuPresenter {
            state: self.state.clone(),
            window_state: self.window_state.clone(),
        });
    }
}

struct WgpuResize {
    state: SharedWgpuState,
}

impl PlatformObserver for WgpuResize {
    fn on_platform(&mut self, event: PlatformEvent<'_>, _out: &mut Vec<InputEvent>) {
        let Some(event) = event.downcast_ref::<WindowEvent>() else {
            return;
        };
        let mut guard = self.state.borrow_mut();
        let Some(state) = guard.as_mut() else {
            return;
        };
        match event {
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state
                    .backend
                    .borrow_mut()
                    .set_scale_factor(*scale_factor as f32);
            }
            _ => {}
        }
    }
}

struct WgpuPresenter {
    state: SharedWgpuState,
    window_state: SharedWindowState,
}

impl WgpuPresenter {
    fn viewport_of(state: &WgpuState, scale: f64) -> ViewportSize {
        let scale = if scale > 0.0 { scale as f32 } else { 1.0 };
        ViewportSize::new(Size::new(
            state.config.width as f32 / scale,
            state.config.height as f32 / scale,
        ))
    }
}

impl Presenter for WgpuPresenter {
    fn viewport(&self) -> ViewportSize {
        let guard = self.state.borrow();
        match guard.as_ref() {
            Some(state) => Self::viewport_of(state, self.window_state.borrow().scale_factor),
            None => ViewportSize::default(),
        }
    }

    fn present(&mut self, list: &DrawList) -> PresentOutcome {
        let mut guard = self.state.borrow_mut();
        let Some(state) = guard.as_mut() else {
            return PresentOutcome::Skipped;
        };
        let viewport = Self::viewport_of(state, self.window_state.borrow().scale_factor);

        let texture = match state.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                state
                    .surface
                    .configure(state.backend.borrow().device(), &state.config);
                return PresentOutcome::Reconfigured;
            }
            Err(wgpu::SurfaceError::Timeout) => return PresentOutcome::Skipped,
            Err(error) => {
                eprintln!("rough_winit: surface error: {error}");
                return PresentOutcome::Skipped;
            }
        };

        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut backend = state.backend.borrow_mut();
        if backend
            .begin_frame_with_view(
                view,
                state.config.width,
                state.config.height,
                state.config.format,
                viewport,
            )
            .is_ok()
        {
            let _ = backend.submit(list);
            let _ = backend.end_frame();
        }
        drop(backend);
        texture.present();
        PresentOutcome::Presented
    }
}
