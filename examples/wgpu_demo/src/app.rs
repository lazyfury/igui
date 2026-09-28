//! The `winit` window runner for the wgpu demo (native only).
//!
//! The platform plumbing (window / surface / backend / swap chain, input
//! translation, IME) lives in [`quill_winit::Host`]; this file is only the
//! demo's own application loop: it times the pipeline phases, drives the shared
//! gallery and paints the debug / performance overlays.

use std::time::Instant;

use draw_backend_wgpu::{FontConfig, FontMode};
use draw_core::{InputEvent, Key};
use draw_debug_ui::{DebugOverlay, PerformanceOverlay};
use draw_profile::{inspect, FrameCounters, FrameStats, InspectionReport, Profiler, StageTimes};
use draw_render::PaintContext;
use quill_winit::{FrameClock, Host, HostOptions, TitlebarMode, TITLEBAR_SAFE_AREA};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::cli::Options;
use crate::demo::Demo;

/// Runs the demo until the window is closed.
pub fn run(options: Options) {
    let event_loop = EventLoop::new().expect("create event loop");
    // Event-driven: the app is static, so render only when something changes
    // (input, resize, overlay toggle). `Poll` would burn CPU redrawing an
    // unchanged frame as fast as possible.
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(options);
    event_loop.run_app(&mut app).expect("run event loop");
}

/// Owns the shared host and the demo state.
struct App {
    host: Host,
    clock: FrameClock,
    demo: Demo,
    /// Current text font mode (toggle with `f`).
    font_mode: FontMode,
    /// Frame timings / counters for the debug overlay.
    profiler: Profiler,
    /// Findings from inspecting the previous frame's `DrawList`.
    report: InspectionReport,
    /// Component debug drawing (yellow bounds + `name#id`), toggled with F3 / ` / d.
    debug: DebugOverlay,
    /// Performance panel, toggled with F4 / p.
    perf: PerformanceOverlay,
    /// Native window frame (title bar) mode chosen on the command line.
    titlebar: TitlebarMode,
}

impl App {
    fn new(options: Options) -> Self {
        // Profiler and overlay start in the state requested on the command line;
        // both remain runtime-switchable (overlay: backtick key).
        let mut profiler = Profiler::new();
        profiler.set_enabled(options.profiler);
        let mut debug = DebugOverlay::new();
        debug.set_open(options.debug_ui);
        let mut perf = PerformanceOverlay::new();
        perf.set_open(options.performance);

        let font_mode = if options.pixel_font {
            FontMode::Pixel
        } else {
            FontMode::System
        };
        let titlebar = match options.titlebar {
            crate::cli::TitlebarMode::Native => TitlebarMode::Native,
            crate::cli::TitlebarMode::Hidden => TitlebarMode::Hidden,
            crate::cli::TitlebarMode::Transparent => TitlebarMode::Transparent,
        };
        let host = Host::new(HostOptions {
            title: "quill — Notes".into(),
            size: (1200.0, 780.0),
            titlebar,
            font: FontConfig {
                mode: font_mode,
                device_pixel_rasterization: true,
                ..Default::default()
            },
            // The gallery has a text field; enable the platform IME.
            ime: true,
            ..Default::default()
        });

        Self {
            host,
            clock: FrameClock::new(),
            demo: Demo::new(),
            font_mode,
            profiler,
            report: InspectionReport::new(),
            debug,
            perf,
            titlebar,
        }
    }

    /// Creates the window, surface, backend and swap chain on first resume.
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        if !self.host.resumed(event_loop) {
            return;
        }
        let metrics = self.host.backend().map(|backend| backend.text_metrics());
        if let Some(metrics) = metrics {
            self.demo.set_text_metrics(metrics);
        }
        self.demo.set_clipboard(self.host.clipboard());

        // With a transparent macOS title bar the content fills the title-bar
        // area, so reserve a top safe area on the sidebar for the traffic lights.
        if self.titlebar == TitlebarMode::Transparent {
            self.demo.set_titlebar_inset(TITLEBAR_SAFE_AREA);
        }
        self.host.request_redraw();
    }

    /// Switches between the system font and the built-in pixel font.
    fn toggle_font(&mut self) {
        self.font_mode = match self.font_mode {
            FontMode::System => FontMode::Pixel,
            FontMode::Pixel => FontMode::System,
        };
        let config = FontConfig {
            mode: self.font_mode,
            device_pixel_rasterization: true,
            ..Default::default()
        };
        let metrics = match self.host.backend_mut() {
            Some(backend) => {
                if let Err(error) = backend.set_font_config(config) {
                    eprintln!("font switch failed: {error}");
                }
                Some(backend.text_metrics())
            }
            None => None,
        };
        if let Some(metrics) = metrics {
            self.demo.set_text_metrics(metrics);
        }
    }

    fn feed(&mut self, event: &InputEvent) {
        // Debug/profiler function keys always win.
        if let InputEvent::KeyDown { key } = event {
            match key {
                Key::F3 => {
                    self.debug.toggle();
                    return;
                }
                Key::F4 => {
                    self.perf.toggle();
                    return;
                }
                Key::F5 => {
                    self.profiler.toggle();
                    return;
                }
                _ => {}
            }
        }
        // The performance panel sits on top: consume input over its panel,
        // pass the rest on to the app UI.
        if self.perf.handle_input(event).is_handled() {
            return;
        }
        // The app/UI first: a focused text field consumes typed characters, so
        // the letter toggles below must not fire while typing.
        if self.demo.event(event).is_handled() {
            return;
        }
        // Host-only convenience toggles for keys the UI did not take.
        if let InputEvent::KeyDown { key } = event {
            match key {
                Key::Character('`') | Key::Character('d') => {
                    self.debug.toggle();
                }
                Key::Character('p') => {
                    self.perf.toggle();
                }
                Key::Character('o') => {
                    self.profiler.toggle();
                }
                Key::Character('f') => self.toggle_font(),
                _ => {}
            }
        }
    }

    fn render(&mut self) {
        let viewport = self.host.viewport();
        self.host.apply_cursor(self.demo.cursor());

        // -- timed pipeline phases ----------------------------------------
        let frame_start = Instant::now();
        let dt = self.clock.tick();

        self.demo.update(viewport, dt);
        let update_done = Instant::now();

        self.demo.layout(viewport);
        let layout_done = Instant::now();

        let mut ctx = PaintContext::new();
        self.demo.paint(&mut ctx);
        // 1) component debug bounds, drawn on top of the app UI.
        self.debug.paint(self.demo.tree(), &mut ctx);
        // 2) performance panel, drawn last so it stays readable.
        self.perf.update(&self.profiler, &self.report, viewport);
        self.perf.paint(&mut ctx);
        let list = ctx.into_draw_list();
        let paint_done = Instant::now();

        self.host.render(&list);
        let render_done = Instant::now();

        // Keep the IME candidate window on the focused field's caret.
        self.host.sync_ime(self.demo.tree());

        // -- record + inspect the frame -----------------------------------
        let stats = FrameStats {
            index: self.profiler.next_index(),
            frame_ms: millis(render_done - frame_start),
            stages: StageTimes::new(
                millis(update_done - frame_start),
                millis(layout_done - update_done),
                millis(paint_done - layout_done),
                millis(render_done - paint_done),
            ),
            counters: FrameCounters::new(0, self.demo.control_count(), list.len(), 1),
        };
        self.profiler.record(stats);
        // Only audit when the performance panel can actually show it.
        if self.perf.is_open() {
            self.report = inspect(&list, &stats);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.init(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // A redraw is already the render itself; do not request another.
        if matches!(event, WindowEvent::RedrawRequested) {
            self.render();
            // Animation / transient overlays need more frames. `PresentMode::Fifo`
            // paces this at the display refresh, so it is not a busy loop, and
            // the loop sleeps again as soon as `needs_frame` turns false.
            if self.demo.needs_frame() {
                self.host.request_redraw();
            }
            return;
        }

        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(size) => self.host.handle_resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.host.handle_scale_factor(*scale_factor)
            }
            _ => {
                for input in self.host.translate(&event) {
                    self.feed(&input);
                }
            }
        }

        // Any handled event may have changed the UI; schedule exactly one frame.
        self.host.request_redraw();
    }
}

fn millis(duration: std::time::Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_clock_never_reports_a_huge_delta() {
        let mut clock = FrameClock::new();
        // The first tick is tiny; the cap is what matters after a pause.
        let dt = clock.tick();
        assert!(dt <= 0.1);
    }
}
