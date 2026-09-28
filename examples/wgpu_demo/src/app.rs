//! The `winit` window runner for the wgpu demo (native only).
//!
//! The platform plumbing is now a set of [`rough_winit`] plugins assembled
//! through [`rough_app::AppBuilder`]. This file is only the demo's own logic:
//! it times the pipeline phases, drives the shared gallery and paints the
//! debug / performance overlays.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use rough_app::{
    App, AppBuilder, AppConfig, AppLogic, EventContext, FrameContext, FrameObserver, InitContext,
    Plugin,
};
use rough_backend_wgpu::{FontConfig, FontMode};
use rough_core::{EventResult, InputEvent, Key};
use rough_debug_ui::{DebugOverlay, PerformanceOverlay};
use rough_headless::{HeadlessPlugin, RecordingHandle};
use rough_profile::{inspect, FrameCounters, FrameStats, InspectionReport, Profiler, StageTimes};
use rough_render::{DrawList, PaintContext};
use rough_ui::{focused_caret, TextMeasurer};
use rough_winit::{
    BackendTextMeasurer, ClipboardPlugin, GpuConfig, ImePlugin, KeyboardPlugin, PointerPlugin,
    SharedBackend, TextMeasurePlugin, TitlebarMode as WinitTitlebar, WgpuPlugin, WindowConfig,
    WinitPlugin, TITLEBAR_SAFE_AREA,
};

use crate::cli::{Options, TitlebarMode};
use crate::demo::Demo;

/// Runs the demo until the window is closed.
pub fn run(options: Options) {
    build_app(options, false).run();
}

/// Runs the shared gallery headlessly through `rough_headless` (no window) and
/// reports what was drawn. Returns an error when nothing was rendered.
pub fn selfcheck() -> Result<(), String> {
    let mut app = build_app(Options::default(), true);
    let recording = app
        .services()
        .get::<RecordingHandle>()
        .ok_or("no recording handle (HeadlessPlugin missing)")?
        .0
        .clone();

    app.run_headless(3);

    let recording = recording.borrow();
    let frames = recording.frame_count();
    if frames != 3 {
        return Err(format!("expected 3 frames, recorded {frames}"));
    }
    let commands = recording
        .last_frame()
        .map(|frame| frame.command_count())
        .unwrap_or(0);
    if commands == 0 {
        return Err("headless frame drew no commands".into());
    }
    println!("wgpu_demo selfcheck: {frames} frames, {commands} commands in the last frame");
    Ok(())
}

fn build_app(options: Options, headless: bool) -> App {
    let profile = Rc::new(RefCell::new(ProfileState::new(options)));
    let font_mode = if options.pixel_font {
        FontMode::Pixel
    } else {
        FontMode::System
    };
    let logic = DemoLogic::new(options, profile.clone(), font_mode);

    let builder = App::new(AppConfig {
        title: "quill — Notes".into(),
        size: (1200.0, 780.0),
        ..Default::default()
    });

    let builder = if headless {
        builder.plugin(HeadlessPlugin::new(1200.0, 780.0))
    } else {
        let titlebar = match options.titlebar {
            TitlebarMode::Native => WinitTitlebar::Native,
            TitlebarMode::Hidden => WinitTitlebar::Hidden,
            TitlebarMode::Transparent => WinitTitlebar::Transparent,
        };
        builder
            .plugin(WinitPlugin::new(WindowConfig {
                title: "quill — Notes".into(),
                size: (1200.0, 780.0),
                titlebar,
                // The gallery has a text field; enable the platform IME.
                ime: true,
            }))
            .plugin(WgpuPlugin::new(GpuConfig {
                font: FontConfig {
                    mode: font_mode,
                    device_pixel_rasterization: true,
                    ..Default::default()
                },
                ..Default::default()
            }))
            .plugin(PointerPlugin)
            .plugin(KeyboardPlugin)
            .plugin(ImePlugin)
            .plugin(TextMeasurePlugin)
            .plugin(ClipboardPlugin)
    };

    builder
        .plugin(ProfilePlugin {
            profile: profile.clone(),
        })
        .logic(logic)
        .build()
}

/// Frame timings / counters and the last inspection, shared between the demo
/// logic (which paints the panel) and the [`ProfileObserver`].
struct ProfileState {
    profiler: Profiler,
    report: InspectionReport,
    frame_start: Instant,
    update_ms: f32,
    layout_ms: f32,
    paint_ms: f32,
    paint_done: Instant,
    control_count: usize,
    perf_open: bool,
}

impl ProfileState {
    fn new(options: Options) -> Self {
        let mut profiler = Profiler::new();
        profiler.set_enabled(options.profiler);
        Self {
            profiler,
            report: InspectionReport::new(),
            frame_start: Instant::now(),
            update_ms: 0.0,
            layout_ms: 0.0,
            paint_ms: 0.0,
            paint_done: Instant::now(),
            control_count: 0,
            perf_open: false,
        }
    }
}

/// Records one frame's stats (and audits the `DrawList` when the panel is open)
/// after it is presented.
struct ProfileObserver {
    profile: Rc<RefCell<ProfileState>>,
}

impl Plugin for ProfilePlugin {
    fn name(&self) -> &'static str {
        "wgpu-demo-profile"
    }

    fn build(&self, app: &mut AppBuilder) {
        app.add_frame_observer(ProfileObserver {
            profile: self.profile.clone(),
        });
    }
}

struct ProfilePlugin {
    profile: Rc<RefCell<ProfileState>>,
}

impl FrameObserver for ProfileObserver {
    fn after_frame(&mut self, _app: &App, list: &DrawList) {
        let mut profile = self.profile.borrow_mut();
        let now = Instant::now();
        let render_ms = millis(now - profile.paint_done);
        let stats = FrameStats {
            index: profile.profiler.next_index(),
            frame_ms: millis(now - profile.frame_start),
            stages: StageTimes::new(
                profile.update_ms,
                profile.layout_ms,
                profile.paint_ms,
                render_ms,
            ),
            counters: FrameCounters::new(0, profile.control_count, list.len(), 1),
        };
        profile.profiler.record(stats);
        if profile.perf_open {
            profile.report = inspect(list, &stats);
        }
    }
}

/// The demo's own `AppLogic`: the shared gallery plus the debug / performance
/// overlays and the frame timing.
struct DemoLogic {
    demo: Demo,
    /// Component debug drawing (yellow bounds + `name#id`), toggled with F3 / ` / d.
    debug: DebugOverlay,
    /// Performance panel, toggled with F4 / p.
    perf: PerformanceOverlay,
    /// Current text font mode (toggle with `f`), and the backend to change it.
    font_mode: FontMode,
    backend: Option<SharedBackend>,
    titlebar: TitlebarMode,
    profile: Rc<RefCell<ProfileState>>,
}

impl DemoLogic {
    fn new(options: Options, profile: Rc<RefCell<ProfileState>>, font_mode: FontMode) -> Self {
        let mut debug = DebugOverlay::new();
        debug.set_open(options.debug_ui);
        let mut perf = PerformanceOverlay::new();
        perf.set_open(options.performance);
        Self {
            demo: Demo::new(),
            debug,
            perf,
            font_mode,
            backend: None,
            titlebar: options.titlebar,
            profile,
        }
    }

    /// Switches between the system font and the built-in pixel font.
    fn toggle_font(&mut self) {
        self.font_mode = match self.font_mode {
            FontMode::System => FontMode::Pixel,
            FontMode::Pixel => FontMode::System,
        };
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let config = FontConfig {
            mode: self.font_mode,
            device_pixel_rasterization: true,
            ..Default::default()
        };
        let metrics = {
            let mut backend = backend.borrow_mut();
            if let Err(error) = backend.set_font_config(config) {
                eprintln!("font switch failed: {error}");
            }
            backend.text_metrics()
        };
        self.demo
            .set_text_measurer(Rc::new(BackendTextMeasurer::new(metrics)));
    }

    fn feed(&mut self, event: &InputEvent) -> EventResult {
        // Debug/profiler function keys always win.
        if let InputEvent::KeyDown { key } = event {
            match key {
                Key::F3 => {
                    self.debug.toggle();
                    return EventResult::Handled;
                }
                Key::F4 => {
                    self.perf.toggle();
                    return EventResult::Handled;
                }
                Key::F5 => {
                    self.profile.borrow_mut().profiler.toggle();
                    return EventResult::Handled;
                }
                _ => {}
            }
        }
        // The performance panel sits on top: consume input over its panel,
        // pass the rest on to the app UI.
        if self.perf.handle_input(event).is_handled() {
            return EventResult::Handled;
        }
        // The app/UI first: a focused text field consumes typed characters, so
        // the letter toggles below must not fire while typing.
        if self.demo.event(event).is_handled() {
            return EventResult::Handled;
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
                    self.profile.borrow_mut().profiler.toggle();
                }
                Key::Character('f') => self.toggle_font(),
                _ => {}
            }
        }
        EventResult::Ignored
    }
}

impl AppLogic for DemoLogic {
    fn init(&mut self, ctx: &InitContext<'_>) {
        if let Some(measurer) = ctx.service::<Rc<dyn TextMeasurer>>() {
            self.demo.set_text_measurer(measurer.clone());
        }
        if let Some(clipboard) = ctx.service::<Rc<RefCell<dyn rough_ui::Clipboard>>>() {
            self.demo.set_clipboard(clipboard.clone());
        }
        if let Some(backend) = ctx.service::<SharedBackend>() {
            self.backend = Some(backend.clone());
        }
        // With a transparent macOS title bar the content fills the title-bar
        // area, so reserve a top safe area on the sidebar for the traffic lights.
        if self.titlebar == TitlebarMode::Transparent {
            self.demo.set_titlebar_inset(TITLEBAR_SAFE_AREA);
        }
    }

    fn event(&mut self, _ctx: &EventContext<'_>, event: &InputEvent) -> EventResult {
        self.feed(event)
    }

    fn update(&mut self, ctx: &FrameContext<'_>) {
        let start = Instant::now();
        self.profile.borrow_mut().frame_start = start;
        self.demo.update(ctx.viewport(), ctx.delta());
        self.profile.borrow_mut().update_ms = millis(Instant::now() - start);
    }

    fn layout(&mut self, ctx: &FrameContext<'_>) {
        let start = Instant::now();
        self.demo.layout(ctx.viewport());
        self.profile.borrow_mut().layout_ms = millis(Instant::now() - start);
    }

    fn paint(&mut self, ctx: &FrameContext<'_>, paint: &mut PaintContext) {
        let start = Instant::now();
        let viewport = ctx.viewport();

        self.demo.paint(paint);
        // 1) component debug bounds, drawn on top of the app UI.
        self.debug.paint(self.demo.tree(), paint);
        // 2) performance panel, drawn last so it stays readable.
        {
            let profile = self.profile.clone();
            let profile = profile.borrow();
            self.perf
                .update(&profile.profiler, &profile.report, viewport);
        }
        self.perf.paint(paint);

        let now = Instant::now();
        let mut profile = self.profile.borrow_mut();
        profile.paint_ms = millis(now - start);
        profile.paint_done = now;
        profile.control_count = self.demo.control_count();
        profile.perf_open = self.perf.is_open();
    }

    fn needs_frame(&self) -> bool {
        self.demo.needs_frame()
    }

    fn cursor(&self) -> Option<rough_core::Cursor> {
        Some(self.demo.cursor())
    }

    fn caret(&self) -> Option<rough_core::Rect> {
        focused_caret(self.demo.tree())
    }
}

fn millis(duration: std::time::Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selfcheck_frame_draws_the_gallery() {
        // The headless path never opens a window.
        assert!(selfcheck().is_ok());
    }
}
