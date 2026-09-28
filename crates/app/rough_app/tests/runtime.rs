//! Integration tests for the `rough_app` runtime, exercised through its public
//! API only: assembling an app from a plugin + logic, routing input through
//! layers, and the `update -> layout -> paint -> present` frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rough_app::{
    App, AppBuilder, AppConfig, AppLogic, EventContext, FrameContext, FrameObserver, InitContext,
    InputLayer, LifecycleObserver, PlatformEvent, PlatformObserver, Plugin, PresentOutcome,
    Presenter,
};
use rough_core::{EventResult, InputEvent, Key, Size, Vec2, ViewportSize};
use rough_render::{DrawList, Paint, PaintContext};

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

struct LogPlugin {
    name: &'static str,
    log: Log,
}

impl Plugin for LogPlugin {
    fn name(&self) -> &'static str {
        self.name
    }
    fn build(&self, _app: &mut AppBuilder) {
        self.log.borrow_mut().push(format!("build:{}", self.name));
    }
    fn finish(&self, _app: &mut AppBuilder) {
        self.log.borrow_mut().push(format!("finish:{}", self.name));
    }
}

struct RunnerPlugin {
    ran: Rc<Cell<bool>>,
}

impl Plugin for RunnerPlugin {
    fn name(&self) -> &'static str {
        "runner"
    }
    fn build(&self, app: &mut AppBuilder) {
        let ran = self.ran.clone();
        app.set_runner(Box::new(move |_app| ran.set(true)));
    }
}

struct LifecycleLog {
    log: Log,
}

impl LifecycleObserver for LifecycleLog {
    fn resumed(&mut self, _app: &mut App) {
        self.log.borrow_mut().push("resumed".into());
    }
    fn suspended(&mut self, _app: &mut App) {
        self.log.borrow_mut().push("suspended".into());
    }
}

struct LifecyclePlugin {
    log: Log,
}

impl Plugin for LifecyclePlugin {
    fn name(&self) -> &'static str {
        "lifecycle"
    }
    fn build(&self, app: &mut AppBuilder) {
        app.add_lifecycle_observer(LifecycleLog {
            log: self.log.clone(),
        });
    }
}

struct KeyObserver;

impl PlatformObserver for KeyObserver {
    fn on_platform(&mut self, _event: PlatformEvent<'_>, out: &mut Vec<InputEvent>) {
        out.push(InputEvent::KeyDown { key: Key::Enter });
    }
}

struct ConsumingLayer;

impl InputLayer for ConsumingLayer {
    fn on_input(&mut self, _event: &InputEvent) -> EventResult {
        EventResult::Handled
    }
}

/// A plugin that installs a key observer and, optionally, a consuming layer.
struct InputPlugin {
    consume: bool,
}

impl Plugin for InputPlugin {
    fn name(&self) -> &'static str {
        "input"
    }
    fn build(&self, app: &mut AppBuilder) {
        app.add_platform_observer(KeyObserver);
        if self.consume {
            app.add_input_layer(ConsumingLayer);
        }
    }
}

#[derive(Default)]
struct FakeLogic {
    log: Log,
    events: Rc<RefCell<Vec<InputEvent>>>,
    needs: bool,
}

impl AppLogic for FakeLogic {
    fn init(&mut self, _ctx: &InitContext<'_>) {
        self.log.borrow_mut().push("init".into());
    }
    fn event(&mut self, _ctx: &EventContext<'_>, event: &InputEvent) -> EventResult {
        self.log.borrow_mut().push("event".into());
        self.events.borrow_mut().push(event.clone());
        EventResult::Ignored
    }
    fn update(&mut self, _ctx: &FrameContext<'_>) {
        self.log.borrow_mut().push("update".into());
    }
    fn layout(&mut self, _ctx: &FrameContext<'_>) {
        self.log.borrow_mut().push("layout".into());
    }
    fn paint(&mut self, _ctx: &FrameContext<'_>, paint: &mut PaintContext) {
        self.log.borrow_mut().push("paint".into());
        paint.draw_line(Vec2::ZERO, Vec2::new(10.0, 10.0), 1.0, Paint::default());
    }
    fn needs_frame(&self) -> bool {
        self.needs
    }
}

struct FakePresenter {
    presented: Rc<Cell<u32>>,
    last_len: Rc<Cell<usize>>,
}

impl Presenter for FakePresenter {
    fn viewport(&self) -> ViewportSize {
        ViewportSize::new(Size::new(800.0, 600.0))
    }
    fn present(&mut self, list: &DrawList) -> PresentOutcome {
        self.presented.set(self.presented.get() + 1);
        self.last_len.set(list.len());
        PresentOutcome::Presented
    }
}

#[test]
fn plugins_build_then_finish_in_order() {
    let log = log();
    let _app = App::new(AppConfig::default())
        .plugin(LogPlugin {
            name: "a",
            log: log.clone(),
        })
        .plugin(LogPlugin {
            name: "b",
            log: log.clone(),
        })
        .build();
    assert_eq!(
        &*log.borrow(),
        &["build:a", "build:b", "finish:a", "finish:b"]
    );
}

#[test]
fn a_plugin_installed_runner_receives_the_app() {
    let ran = Rc::new(Cell::new(false));
    App::new(AppConfig::default())
        .plugin(RunnerPlugin { ran: ran.clone() })
        .build()
        .run();
    assert!(ran.get());
}

#[test]
fn resumed_runs_lifecycle_then_init() {
    let log = log();
    let mut app = App::new(AppConfig::default())
        .plugin(LifecyclePlugin { log: log.clone() })
        .logic(FakeLogic {
            log: log.clone(),
            ..Default::default()
        })
        .build();

    app.resumed();
    assert_eq!(&*log.borrow(), &["resumed", "init"]);

    app.suspended();
    assert_eq!(&*log.borrow(), &["resumed", "init", "suspended"]);
}

#[test]
fn platform_events_reach_the_logic() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut app = App::new(AppConfig::default())
        .plugin(InputPlugin { consume: false })
        .logic(FakeLogic {
            events: events.clone(),
            ..Default::default()
        })
        .build();

    let native = 0u32;
    app.platform_event(PlatformEvent::new(&native));
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(events.borrow()[0], InputEvent::KeyDown { key: Key::Enter });
}

#[test]
fn an_input_layer_consumes_before_the_logic() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut app = App::new(AppConfig::default())
        .plugin(InputPlugin { consume: true })
        .logic(FakeLogic {
            events: events.clone(),
            ..Default::default()
        })
        .build();

    let native = 0u32;
    app.platform_event(PlatformEvent::new(&native));
    assert!(events.borrow().is_empty());
}

#[test]
fn a_frame_runs_the_phases_then_presents() {
    let log = log();
    let presented = Rc::new(Cell::new(0));
    let last_len = Rc::new(Cell::new(0));
    let mut app = App::new(AppConfig::default())
        .logic(FakeLogic {
            log: log.clone(),
            ..Default::default()
        })
        .build();
    app.set_presenter(FakePresenter {
        presented: presented.clone(),
        last_len: last_len.clone(),
    });

    app.frame();
    assert_eq!(&*log.borrow(), &["update", "layout", "paint"]);
    assert_eq!(presented.get(), 1);
    assert_eq!(last_len.get(), 1);
}

#[test]
fn headless_run_renders_the_requested_frames() {
    let presented = Rc::new(Cell::new(0));
    let mut app = App::new(AppConfig::default())
        .logic(FakeLogic::default())
        .build();
    app.set_presenter(FakePresenter {
        presented: presented.clone(),
        last_len: Rc::new(Cell::new(0)),
    });

    app.run_headless(3);
    assert_eq!(presented.get(), 3);
}

#[test]
fn needs_frame_comes_from_the_logic() {
    let idle = App::new(AppConfig::default())
        .logic(FakeLogic::default())
        .build();
    assert!(!idle.needs_frame());

    let busy = App::new(AppConfig::default())
        .logic(FakeLogic {
            needs: true,
            ..Default::default()
        })
        .build();
    assert!(busy.needs_frame());
}

/// A platform runner publishes what it created (e.g. a window) into the app's
/// services; a lifecycle observer reads it back on resume.
struct ServiceReader {
    seen: Rc<Cell<bool>>,
}

impl LifecycleObserver for ServiceReader {
    fn resumed(&mut self, app: &mut App) {
        self.seen
            .set(app.services().get::<u32>().copied() == Some(42));
    }
}

struct ServiceReaderPlugin {
    seen: Rc<Cell<bool>>,
}

impl Plugin for ServiceReaderPlugin {
    fn name(&self) -> &'static str {
        "service-reader"
    }
    fn build(&self, app: &mut AppBuilder) {
        app.add_lifecycle_observer(ServiceReader {
            seen: self.seen.clone(),
        });
    }
}

#[test]
fn a_published_service_is_visible_to_lifecycle_observers() {
    let seen = Rc::new(Cell::new(false));
    let mut app = App::new(AppConfig::default())
        .plugin(ServiceReaderPlugin { seen: seen.clone() })
        .build();
    app.services_mut().insert(42u32);
    app.resumed();
    assert!(seen.get());
}

struct FrameLog {
    log: Log,
}

impl FrameObserver for FrameLog {
    fn after_frame(&mut self, app: &App, _list: &DrawList) {
        let width = app.viewport().logical_size().width as u32;
        self.log.borrow_mut().push(format!("after:{width}"));
    }
}

struct FrameLogPlugin {
    log: Log,
}

impl Plugin for FrameLogPlugin {
    fn name(&self) -> &'static str {
        "frame-log"
    }
    fn build(&self, app: &mut AppBuilder) {
        app.add_frame_observer(FrameLog {
            log: self.log.clone(),
        });
    }
}

#[test]
fn frame_observers_run_after_present() {
    let log = log();
    let mut app = App::new(AppConfig::default())
        .plugin(FrameLogPlugin { log: log.clone() })
        .build();
    app.set_presenter(FakePresenter {
        presented: Rc::new(Cell::new(0)),
        last_len: Rc::new(Cell::new(0)),
    });

    app.frame();
    assert_eq!(&*log.borrow(), &["after:800"]);
}
