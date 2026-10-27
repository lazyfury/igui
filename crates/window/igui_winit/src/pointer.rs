//! Pointer input: mouse move / button / wheel / pinch / rotate → core
//! [`InputEvent`]s.
//!
//! Wheel events carry both the device ([`ScrollDevice`](igui_core::ScrollDevice):
//! a detented wheel vs a precise trackpad surface) and the gesture
//! [`TouchPhase`](igui_core::TouchPhase), so a host can tell the two apart
//! without re-reading native events. macOS / iOS pinch and rotate gestures are
//! forwarded as [`InputEvent::Pinch`] / [`InputEvent::Rotate`]; platforms
//! without them simply never send those variants.

use igui_app::{AppBuilder, PlatformEvent, PlatformObserver, Plugin};
use igui_core::{InputEvent, PointerButton};
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
            WindowEvent::MouseWheel {
                delta,
                phase: wheel_phase,
                ..
            } => {
                let (delta, device) = input::wheel(*delta, state.scale_factor as f32);
                out.push(InputEvent::Wheel {
                    position: state.cursor,
                    delta,
                    phase: input::touch_phase(*wheel_phase),
                    device,
                });
            }
            // macOS / iOS only; a magnification of 0 or NaN carries no zoom.
            WindowEvent::PinchGesture { delta, phase, .. }
                if delta.is_finite() && *delta != 0.0 =>
            {
                out.push(InputEvent::Pinch {
                    position: state.cursor,
                    factor: 1.0 + *delta as f32,
                    phase: input::touch_phase(*phase),
                });
            }
            // winit reports degrees counter-clockwise; the core uses degrees
            // clockwise (screen space, y down), so the sign flips here.
            WindowEvent::RotationGesture { delta, phase, .. }
                if delta.is_finite() && *delta != 0.0 =>
            {
                out.push(InputEvent::Rotate {
                    position: state.cursor,
                    delta: -*delta,
                    phase: input::touch_phase(*phase),
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

    use igui_core::{PointerButton, Vec2};
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
    fn a_precise_wheel_means_trackpad_and_keeps_the_phase() {
        let (mut observer, _) = setup(2.0);
        let wheel = translate(
            &mut observer,
            &WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(-40.0, 60.0)),
                phase: TouchPhase::Started,
            },
        );
        assert!(
            matches!(wheel.as_slice(), [InputEvent::Wheel { delta, phase, device, .. }]
                if *device == igui_core::ScrollDevice::Trackpad
                    && *phase == igui_core::TouchPhase::Started
                    && delta.x > 0.0
                    && delta.y < 0.0),
            "{wheel:?}"
        );
    }

    #[test]
    fn pinch_and_rotate_forward_as_gestures() {
        let (mut observer, _) = setup(1.0);
        let pinch = translate(
            &mut observer,
            &WindowEvent::PinchGesture {
                device_id: DeviceId::dummy(),
                delta: 0.25,
                phase: TouchPhase::Moved,
            },
        );
        assert!(
            matches!(pinch.as_slice(), [InputEvent::Pinch { factor, .. }]
                if (*factor - 1.25).abs() < 1e-6),
            "{pinch:?}"
        );
        // winit rotation is counter-clockwise; the core is clockwise (y down).
        let rotate = translate(
            &mut observer,
            &WindowEvent::RotationGesture {
                device_id: DeviceId::dummy(),
                delta: 15.0,
                phase: TouchPhase::Moved,
            },
        );
        assert!(
            matches!(rotate.as_slice(), [InputEvent::Rotate { delta, .. }] if *delta == -15.0),
            "{rotate:?}"
        );
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
            matches!(wheel.as_slice(), [InputEvent::Wheel { delta, device, .. }]
                if delta.y < 0.0 && *device == igui_core::ScrollDevice::Wheel),
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
