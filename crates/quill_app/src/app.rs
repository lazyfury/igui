//! The application runtime: `App`, its builder and the frame loop.
//!
//! ```no_run
//! # use quill_app::{App, AppConfig};
//! # struct MyPlugin;
//! # impl quill_app::Plugin for MyPlugin {
//! #     fn name(&self) -> &'static str { "my" }
//! #     fn build(&self, _app: &mut quill_app::AppBuilder) {}
//! # }
//! # struct MyApp;
//! # impl quill_app::AppLogic for MyApp {}
//! App::new(AppConfig::default())
//!     .plugin(MyPlugin)
//!     .logic(MyApp)
//!     .build()
//!     .run();
//! ```
//!
//! `quill_app` is backend-neutral: it never names a window or GPU type. A
//! platform plugin installs a [`Runner`] (the loop) and a
//! [`Presenter`] (the graphics boundary); headless use installs a recording
//! presenter and calls [`App::run_headless`].

use std::any::Any;
use std::time::Instant;

use draw_core::{Cursor, InputEvent, Rect, ViewportSize};
use draw_render::PaintContext;

use crate::logic::{AppLogic, EventContext, FrameContext, InitContext};
use crate::platform::{
    InputLayer, LifecycleObserver, PaintLayer, PlatformContext, PlatformEvent, PlatformObserver,
    Presenter, Runner,
};
use crate::plugin::Plugin;
use crate::service::ServiceMap;

/// Application-wide configuration, independent of any platform.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Window title hint (the platform plugin decides how to use it).
    pub title: String,
    /// Initial window size in logical pixels.
    pub size: (f64, f64),
    /// Frames are capped at this delta so a paused app does not jump.
    pub max_frame_delta: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "quill".into(),
            size: (1200.0, 780.0),
            max_frame_delta: 0.1,
        }
    }
}

/// Wall-clock frame delta, capped so a paused app does not jump.
pub struct FrameClock {
    last: Instant,
    max_delta: f32,
}

impl FrameClock {
    pub fn new(max_delta: f32) -> Self {
        Self {
            last: Instant::now(),
            max_delta,
        }
    }

    /// Seconds since the previous tick, capped at the configured maximum.
    pub fn tick(&mut self) -> f32 {
        let now = Instant::now();
        let delta = now
            .duration_since(self.last)
            .as_secs_f32()
            .min(self.max_delta);
        self.last = now;
        delta
    }
}

/// Assembles an [`App`] from plugins, the app logic and the runtime services.
///
/// Plugins register during [`Plugin::build`]; the builder is also the place a
/// plugin installs a [`Presenter`], a [`Runner`] and the layer lists. The
/// fluent methods ([`plugin`](AppBuilder::plugin), [`logic`](AppBuilder::logic),
/// [`build`](AppBuilder::build)) consume and return the builder; a plugin's
/// `&mut AppBuilder` registration methods return `&mut Self`.
pub struct AppBuilder {
    config: AppConfig,
    services: ServiceMap,
    plugins: Vec<Box<dyn Plugin>>,
    logic: Option<Box<dyn AppLogic>>,
    presenter: Option<Box<dyn Presenter>>,
    runner: Option<Runner>,
    platform_observers: Vec<Box<dyn PlatformObserver>>,
    lifecycle_observers: Vec<Box<dyn LifecycleObserver>>,
    input_layers: Vec<Box<dyn InputLayer>>,
    paint_layers: Vec<Box<dyn PaintLayer>>,
}

impl AppBuilder {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            services: ServiceMap::new(),
            plugins: Vec::new(),
            logic: None,
            presenter: None,
            runner: None,
            platform_observers: Vec::new(),
            lifecycle_observers: Vec::new(),
            input_layers: Vec::new(),
            paint_layers: Vec::new(),
        }
    }

    /// Adds a plugin, immediately running its [`Plugin::build`].
    pub fn plugin<P: Plugin>(mut self, plugin: P) -> Self {
        plugin.build(&mut self);
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Sets the application logic.
    pub fn logic<L: AppLogic>(mut self, logic: L) -> Self {
        self.logic = Some(Box::new(logic));
        self
    }

    /// Runs every plugin's [`Plugin::finish`] in registration order, then
    /// freezes the builder into an [`App`].
    pub fn build(mut self) -> App {
        let plugins = std::mem::take(&mut self.plugins);
        for plugin in &plugins {
            plugin.finish(&mut self);
        }
        let clock = FrameClock::new(self.config.max_frame_delta);
        App {
            config: self.config,
            services: self.services,
            logic: self.logic,
            presenter: self.presenter,
            runner: self.runner,
            platform_observers: self.platform_observers,
            lifecycle_observers: self.lifecycle_observers,
            input_layers: self.input_layers,
            paint_layers: self.paint_layers,
            clock,
        }
    }

    // -- registration surface used by plugins -------------------------------

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// Registers a service, replacing any previous value of the same type.
    pub fn insert_service<T: Any>(&mut self, service: T) -> &mut Self {
        self.services.insert(service);
        self
    }

    pub fn service<T: Any>(&self) -> Option<&T> {
        self.services.get::<T>()
    }

    pub fn service_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.services.get_mut::<T>()
    }

    /// Installs the graphics boundary.
    ///
    /// A platform plugin may instead install it later from a lifecycle observer
    /// (once the surface exists) via [`App::set_presenter`].
    pub fn set_presenter(&mut self, presenter: impl Presenter + 'static) -> &mut Self {
        self.presenter = Some(Box::new(presenter));
        self
    }

    /// Installs the platform loop driver.
    pub fn set_runner(&mut self, runner: Runner) -> &mut Self {
        self.runner = Some(runner);
        self
    }

    pub fn add_platform_observer(&mut self, observer: impl PlatformObserver) -> &mut Self {
        self.platform_observers.push(Box::new(observer));
        self
    }

    pub fn add_lifecycle_observer(&mut self, observer: impl LifecycleObserver) -> &mut Self {
        self.lifecycle_observers.push(Box::new(observer));
        self
    }

    pub fn add_input_layer(&mut self, layer: impl InputLayer) -> &mut Self {
        self.input_layers.push(Box::new(layer));
        self
    }

    pub fn add_paint_layer(&mut self, layer: impl PaintLayer) -> &mut Self {
        self.paint_layers.push(Box::new(layer));
        self
    }
}

/// The application runtime: services, logic, layers, presenter and the frame
/// clock. Built by [`AppBuilder`].
pub struct App {
    config: AppConfig,
    services: ServiceMap,
    logic: Option<Box<dyn AppLogic>>,
    presenter: Option<Box<dyn Presenter>>,
    runner: Option<Runner>,
    platform_observers: Vec<Box<dyn PlatformObserver>>,
    lifecycle_observers: Vec<Box<dyn LifecycleObserver>>,
    input_layers: Vec<Box<dyn InputLayer>>,
    paint_layers: Vec<Box<dyn PaintLayer>>,
    clock: FrameClock,
}

impl App {
    /// Starts building an app with `config`.
    pub fn new(config: AppConfig) -> AppBuilder {
        AppBuilder::new(config)
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn services(&self) -> &ServiceMap {
        &self.services
    }

    pub fn services_mut(&mut self) -> &mut ServiceMap {
        &mut self.services
    }

    pub fn logic(&self) -> Option<&dyn AppLogic> {
        self.logic.as_deref()
    }

    pub fn logic_mut(&mut self) -> Option<&mut (dyn AppLogic + 'static)> {
        self.logic.as_deref_mut()
    }

    /// Installs or replaces the presenter (used by lifecycle observers once the
    /// surface exists).
    pub fn set_presenter(&mut self, presenter: impl Presenter + 'static) {
        self.presenter = Some(Box::new(presenter));
    }

    /// The drawing area in logical pixels, from the presenter.
    pub fn viewport(&self) -> ViewportSize {
        match self.presenter.as_ref() {
            Some(presenter) => presenter.viewport(),
            None => ViewportSize::default(),
        }
    }

    /// Whether the app wants another frame.
    pub fn needs_frame(&self) -> bool {
        self.logic.as_ref().is_some_and(|logic| logic.needs_frame())
    }

    /// The cursor the app wants shown this frame.
    pub fn cursor(&self) -> Option<Cursor> {
        self.logic.as_ref().and_then(|logic| logic.cursor())
    }

    /// The focused caret for IME placement.
    pub fn caret(&self) -> Option<Rect> {
        self.logic.as_ref().and_then(|logic| logic.caret())
    }

    /// Runs the platform driver installed by a plugin.
    ///
    /// # Panics
    ///
    /// Panics if no plugin installed a runner; add a platform plugin (e.g.
    /// `WinitPlugin`).
    pub fn run(mut self) {
        let Some(runner) = self.runner.take() else {
            panic!("quill_app: no runner registered; add a platform plugin such as WinitPlugin");
        };
        runner(self);
    }

    /// Runs exactly `frames` frames with no platform events or redraw pacing.
    ///
    /// This is the headless / self-check path: a recording presenter captures
    /// each frame's `DrawList` and `winit` is never linked.
    pub fn run_headless(&mut self, frames: u32) {
        for _ in 0..frames {
            self.frame();
        }
    }

    /// Advances the platform, creates any windows, then initializes the app.
    pub fn resumed(&mut self, mut context: PlatformContext<'_>) {
        let mut observers = std::mem::take(&mut self.lifecycle_observers);
        for observer in &mut observers {
            observer.resumed(context.reborrow(), self);
        }
        self.lifecycle_observers = observers;

        let ctx = InitContext::new(self.viewport(), &self.services);
        if let Some(logic) = self.logic.as_mut() {
            logic.init(&ctx);
        }
    }

    /// The platform is going away.
    pub fn suspended(&mut self) {
        let mut observers = std::mem::take(&mut self.lifecycle_observers);
        for observer in &mut observers {
            observer.suspended(self);
        }
        self.lifecycle_observers = observers;
    }

    /// Feeds one native platform event through the observers, then routes the
    /// produced core input events through the input layers to the app logic.
    pub fn platform_event(&mut self, event: PlatformEvent<'_>) {
        let mut inputs = Vec::new();
        for observer in &mut self.platform_observers {
            observer.on_platform(event, &mut inputs);
        }
        for input in &inputs {
            self.dispatch_input(input);
        }
    }

    fn dispatch_input(&mut self, event: &InputEvent) {
        for layer in &mut self.input_layers {
            if layer.on_input(event).is_handled() {
                return;
            }
        }
        let ctx = EventContext::new(&self.services);
        if let Some(logic) = self.logic.as_mut() {
            let _ = logic.event(&ctx, event);
        }
    }

    /// Runs one frame: `update -> layout -> paint -> present`.
    pub fn frame(&mut self) {
        let delta = self.clock.tick();
        let viewport = self.viewport();

        {
            let ctx = FrameContext::new(delta, viewport, &self.services);
            if let Some(logic) = self.logic.as_mut() {
                logic.update(&ctx);
            }
        }
        {
            let ctx = FrameContext::new(delta, viewport, &self.services);
            if let Some(logic) = self.logic.as_mut() {
                logic.layout(&ctx);
            }
        }

        let mut paint = PaintContext::new();
        {
            let ctx = FrameContext::new(delta, viewport, &self.services);
            if let Some(logic) = self.logic.as_mut() {
                logic.paint(&ctx, &mut paint);
            }
            for layer in &mut self.paint_layers {
                layer.paint(&ctx, &mut paint);
            }
        }

        let list = paint.into_draw_list();
        if let Some(presenter) = self.presenter.as_mut() {
            let _ = presenter.present(&list);
        }
    }
}
