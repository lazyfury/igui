//! Keyboard input: named keys, modifiers and committed text.

use draw_core::InputEvent;
use quill_app::{AppBuilder, PlatformEvent, PlatformObserver, Plugin};
use winit::event::{ElementState, WindowEvent};

use crate::input;
use crate::window::SharedWindowState;

/// Translates keyboard / modifier native events into core input events.
///
/// Committed text is suppressed while an IME composition owns the keyboard, so
/// pair it with [`ImePlugin`](crate::ImePlugin) (which maintains that flag).
#[derive(Default)]
pub struct KeyboardPlugin;

impl Plugin for KeyboardPlugin {
    fn name(&self) -> &'static str {
        "keyboard"
    }

    fn build(&self, app: &mut AppBuilder) {
        let state = app
            .service::<SharedWindowState>()
            .cloned()
            .unwrap_or_default();
        app.add_platform_observer(KeyboardObserver { state });
    }
}

struct KeyboardObserver {
    state: SharedWindowState,
}

impl PlatformObserver for KeyboardObserver {
    fn on_platform(&mut self, event: PlatformEvent<'_>, out: &mut Vec<InputEvent>) {
        let Some(event) = event.downcast_ref::<WindowEvent>() else {
            return;
        };
        match event {
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
                // keyboard. Merely having the IME *enabled* must not drop plain
                // keys: with a CJK IME active a space outside a composition is
                // ordinary text.
                if event.state == ElementState::Pressed && !self.state.borrow().composing {
                    if let Some(text) =
                        input::committed_text(&event.logical_key, event.text.as_deref())
                    {
                        out.push(InputEvent::TextInput { text });
                    }
                }
            }
            _ => {}
        }
    }
}
