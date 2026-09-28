//! Pointer input: mouse move / button / wheel → core [`InputEvent`]s.

use cobbled_app::{AppBuilder, PlatformEvent, PlatformObserver, Plugin};
use cobbled_core::{InputEvent, PointerButton, Vec2};
use winit::event::{ElementState, WindowEvent};

use crate::input;
use crate::window::SharedWindowState;

/// Translates pointer / wheel native events into core input events.
///
/// Requires [`WinitPlugin`](crate::WinitPlugin) so it can read the shared
/// window state (scale factor, cursor, double-click tracker).
#[derive(Default)]
pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn name(&self) -> &'static str {
        "pointer"
    }

    fn build(&self, app: &mut AppBuilder) {
        let state = app
            .service::<SharedWindowState>()
            .cloned()
            .unwrap_or_default();
        app.add_platform_observer(PointerObserver { state });
    }
}

struct PointerObserver {
    state: SharedWindowState,
}

impl PlatformObserver for PointerObserver {
    fn on_platform(&mut self, event: PlatformEvent<'_>, out: &mut Vec<InputEvent>) {
        let Some(event) = event.downcast_ref::<WindowEvent>() else {
            return;
        };
        let mut state = self.state.borrow_mut();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = input::to_logical(*position, state.scale_factor);
                out.push(InputEvent::PointerMove {
                    position: state.cursor,
                });
            }
            WindowEvent::CursorLeft { .. } => out.push(InputEvent::PointerLeave),
            WindowEvent::MouseInput {
                state: element_state,
                button,
                ..
            } => {
                let button = input::pointer_button(*button);
                let position = state.cursor;
                match element_state {
                    ElementState::Pressed => {
                        out.push(InputEvent::PointerDown { position, button });
                        if button == PointerButton::Left && state.double_click.press(position) {
                            out.push(InputEvent::DoubleClick { position });
                        }
                    }
                    ElementState::Released => out.push(InputEvent::PointerUp { position, button }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = input::wheel_pixels(*delta, state.scale_factor as f32);
                out.push(InputEvent::Wheel {
                    position: state.cursor,
                    delta: Vec2::new(0.0, delta),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use cobbled_core::{PointerButton, Vec2};
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, MouseButton, MouseScrollDelta, TouchPhase};

    use super::*;
    use crate::window::WindowState;

    fn setup(scale: f64) -> (PointerObserver, SharedWindowState) {
        let state = Rc::new(RefCell::new(WindowState {
            scale_factor: scale,
            ..Default::default()
        }));
        (
            PointerObserver {
                state: state.clone(),
            },
            state,
        )
    }

    fn translate(observer: &mut PointerObserver, event: &WindowEvent) -> Vec<InputEvent> {
        let mut out = Vec::new();
        observer.on_platform(PlatformEvent::new(event), &mut out);
        out
    }

    #[test]
    fn pointer_move_wheel_and_button_translate() {
        let (mut observer, _) = setup(2.0);

        let moved = translate(
            &mut observer,
            &WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(40.0, 60.0),
            },
        );
        assert_eq!(
            moved,
            vec![InputEvent::PointerMove {
                position: Vec2::new(20.0, 30.0)
            }]
        );

        // Wheel-up scrolls up, so the offset decreases.
        let wheel = translate(
            &mut observer,
            &WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, 1.0),
                phase: TouchPhase::Moved,
            },
        );
        assert!(
            matches!(wheel.as_slice(), [InputEvent::Wheel { delta, .. }] if delta.y < 0.0),
            "{wheel:?}"
        );

        let down = translate(
            &mut observer,
            &WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
        );
        assert!(
            matches!(
                down.as_slice(),
                [InputEvent::PointerDown {
                    button: PointerButton::Left,
                    ..
                }]
            ),
            "{down:?}"
        );
    }
}
