//! IME composition events and candidate-window placement.

use std::sync::Arc;

use draw_core::{ImeEvent, InputEvent, Rect};
use draw_render::DrawList;
use quill_app::{
    App, AppBuilder, FrameObserver, LifecycleObserver, PlatformEvent, PlatformObserver, Plugin,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{Ime, WindowEvent};
use winit::window::Window;

use crate::window::{SharedWindow, SharedWindowState, WindowConfig};

/// Translates IME events and keeps the candidate window on the app's caret.
///
/// The caret is read from [`App::caret`](quill_app::App::caret) after each frame,
/// so the candidate window follows the focused text field.
#[derive(Default)]
pub struct ImePlugin;

impl Plugin for ImePlugin {
    fn name(&self) -> &'static str {
        "ime"
    }

    fn build(&self, app: &mut AppBuilder) {
        let state = app
            .service::<SharedWindowState>()
            .cloned()
            .unwrap_or_default();
        let window = app.service::<SharedWindow>().cloned().unwrap_or_default();
        app.add_platform_observer(ImeObserver {
            state: state.clone(),
        });
        app.add_lifecycle_observer(ImeLifecycle {
            window: window.clone(),
        });
        app.add_frame_observer(ImeFrame { state, window });
    }
}

struct ImeObserver {
    state: SharedWindowState,
}

impl PlatformObserver for ImeObserver {
    fn on_platform(&mut self, event: PlatformEvent<'_>, out: &mut Vec<InputEvent>) {
        let Some(WindowEvent::Ime(ime)) = event.downcast_ref::<WindowEvent>() else {
            return;
        };
        let mut state = self.state.borrow_mut();
        match ime {
            Ime::Enabled => {
                state.ime_active = true;
                out.push(InputEvent::Ime(ImeEvent::Enabled));
            }
            Ime::Disabled => {
                state.ime_active = false;
                state.composing = false;
                out.push(InputEvent::Ime(ImeEvent::Disabled));
            }
            Ime::Preedit(text, cursor) => {
                // An empty preedit means the composition was cleared.
                state.composing = !text.is_empty();
                out.push(InputEvent::Ime(ImeEvent::Preedit {
                    text: text.clone(),
                    cursor: *cursor,
                }));
            }
            Ime::Commit(text) => {
                state.composing = false;
                out.push(InputEvent::Ime(ImeEvent::Commit(text.clone())));
            }
        }
    }
}

struct ImeLifecycle {
    window: SharedWindow,
}

impl LifecycleObserver for ImeLifecycle {
    fn resumed(&mut self, app: &mut App) {
        let enabled = app
            .services()
            .get::<WindowConfig>()
            .map(|config| config.ime)
            .unwrap_or(false);
        if !enabled {
            return;
        }
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_ime_allowed(true);
        }
    }
}

struct ImeFrame {
    state: SharedWindowState,
    window: SharedWindow,
}

impl FrameObserver for ImeFrame {
    fn after_frame(&mut self, app: &App, _list: &DrawList) {
        let Some(window) = self.window.borrow().clone() else {
            return;
        };
        let scale = self.state.borrow().scale_factor;
        set_ime_cursor_area(&window, app.caret(), scale);
    }
}

/// Places the IME candidate window at `rect` (logical viewport coordinates).
///
/// `None` parks it at the origin (composition over nothing).
pub fn set_ime_cursor_area(window: &Arc<Window>, rect: Option<Rect>, scale: f64) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use quill_app::PlatformObserver;
    use winit::event::Ime;

    use super::*;
    use crate::window::WindowState;

    fn observer() -> (ImeObserver, SharedWindowState) {
        let state: SharedWindowState = Rc::new(RefCell::new(WindowState::default()));
        (
            ImeObserver {
                state: state.clone(),
            },
            state,
        )
    }

    #[test]
    fn ime_events_translate_and_track_composition() {
        let (mut observer, state) = observer();

        let mut out = Vec::new();
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Enabled)),
            &mut out,
        );
        assert_eq!(out, vec![InputEvent::Ime(ImeEvent::Enabled)]);
        assert!(state.borrow().ime_active);

        out.clear();
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Preedit(
                "ni".into(),
                Some((1, 1)),
            ))),
            &mut out,
        );
        assert_eq!(
            out,
            vec![InputEvent::Ime(ImeEvent::Preedit {
                text: "ni".into(),
                cursor: Some((1, 1)),
            })]
        );
        assert!(state.borrow().composing);

        out.clear();
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Commit("你".into()))),
            &mut out,
        );
        assert_eq!(out, vec![InputEvent::Ime(ImeEvent::Commit("你".into()))]);
        assert!(!state.borrow().composing);

        // An empty preedit also clears the composition flag.
        out.clear();
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Preedit("x".into(), None))),
            &mut out,
        );
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Preedit(String::new(), None))),
            &mut out,
        );
        assert!(!state.borrow().composing);

        out.clear();
        observer.on_platform(
            quill_app::PlatformEvent::new(&WindowEvent::Ime(Ime::Disabled)),
            &mut out,
        );
        assert_eq!(out, vec![InputEvent::Ime(ImeEvent::Disabled)]);
        assert!(!state.borrow().ime_active);
    }
}
