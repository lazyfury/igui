//! Platform input → backend-neutral [`InputEvent`] translation.
//!
//! These are the pieces every window host used to copy: function-key and named
//! key mapping, pointer-button mapping, the wheel sign convention, and the
//! modifier set. [`Host::translate`](crate::Host::translate) builds the events;
//! these functions stay public so a host that keeps its own loop can reuse them.

use draw_core::{Key, Modifiers, PointerButton, Vec2};
use winit::dpi::PhysicalPosition;
use winit::event::{MouseButton, MouseScrollDelta};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};

/// One wheel notch scrolls this many logical pixels (about three text lines).
pub const WHEEL_LINE_HEIGHT: f32 = 48.0;

/// Maps a platform mouse button; anything unknown is the primary button.
pub fn pointer_button(button: MouseButton) -> PointerButton {
    match button {
        MouseButton::Right => PointerButton::Right,
        MouseButton::Middle => PointerButton::Middle,
        _ => PointerButton::Left,
    }
}

/// Platform wheel → logical pixels (`y > 0` scrolls down, matching
/// [`InputEvent::Wheel`](draw_core::InputEvent::Wheel)).
///
/// Wheel-up (`LineDelta` `y > 0`) scrolls up, so the offset decreases; the sign
/// convention lives here, not in the core.
pub fn wheel_pixels(delta: MouseScrollDelta, scale: f32) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, lines) => -lines * WHEEL_LINE_HEIGHT,
        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => {
            -(y as f32) / if scale > 0.0 { scale } else { 1.0 }
        }
    }
}

/// The platform modifier state as a [`Modifiers`].
pub fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        ctrl: state.control_key(),
        alt: state.alt_key(),
        meta: state.super_key(),
    }
}

/// Maps a logical key to the core key model (`None` for keys the core does not
/// name, e.g. the modifier keys themselves).
pub fn map_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
        WinitKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
        WinitKey::Named(NamedKey::Delete) => Some(Key::Delete),
        WinitKey::Named(NamedKey::Tab) => Some(Key::Tab),
        WinitKey::Named(NamedKey::Space) => Some(Key::Space),
        WinitKey::Named(NamedKey::Home) => Some(Key::Home),
        WinitKey::Named(NamedKey::End) => Some(Key::End),
        WinitKey::Named(NamedKey::ArrowUp) => Some(Key::ArrowUp),
        WinitKey::Named(NamedKey::ArrowDown) => Some(Key::ArrowDown),
        WinitKey::Named(NamedKey::ArrowLeft) => Some(Key::ArrowLeft),
        WinitKey::Named(NamedKey::ArrowRight) => Some(Key::ArrowRight),
        WinitKey::Named(NamedKey::F1) => Some(Key::F1),
        WinitKey::Named(NamedKey::F2) => Some(Key::F2),
        WinitKey::Named(NamedKey::F3) => Some(Key::F3),
        WinitKey::Named(NamedKey::F4) => Some(Key::F4),
        WinitKey::Named(NamedKey::F5) => Some(Key::F5),
        WinitKey::Named(NamedKey::F6) => Some(Key::F6),
        WinitKey::Named(NamedKey::F7) => Some(Key::F7),
        WinitKey::Named(NamedKey::F8) => Some(Key::F8),
        WinitKey::Named(NamedKey::F9) => Some(Key::F9),
        WinitKey::Named(NamedKey::F10) => Some(Key::F10),
        WinitKey::Named(NamedKey::F11) => Some(Key::F11),
        WinitKey::Named(NamedKey::F12) => Some(Key::F12),
        WinitKey::Character(text) => text.chars().next().map(Key::Character),
        _ => None,
    }
}

/// Converts a physical pointer position to logical viewport coordinates.
pub fn to_logical(position: PhysicalPosition<f64>, scale: f64) -> Vec2 {
    let scale = if scale > 0.0 { scale as f32 } else { 1.0 };
    Vec2::new(position.x as f32 / scale, position.y as f32 / scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wheel_up_scrolls_up() {
        assert!(wheel_pixels(MouseScrollDelta::LineDelta(0.0, 1.0), 1.0) < 0.0);
        assert!(wheel_pixels(MouseScrollDelta::LineDelta(0.0, -1.0), 1.0) > 0.0);
    }

    #[test]
    fn pixel_deltas_are_converted_to_logical() {
        let pixels = wheel_pixels(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -40.0)),
            2.0,
        );
        assert_eq!(pixels, 20.0);
    }

    #[test]
    fn named_keys_and_characters_map() {
        assert_eq!(map_key(&WinitKey::Named(NamedKey::Enter)), Some(Key::Enter));
        assert_eq!(
            map_key(&WinitKey::Named(NamedKey::ArrowLeft)),
            Some(Key::ArrowLeft)
        );
        assert_eq!(
            map_key(&WinitKey::Character("a".into())),
            Some(Key::Character('a'))
        );
        assert_eq!(map_key(&WinitKey::Named(NamedKey::Shift)), None);
    }

    #[test]
    fn modifiers_round_trip() {
        let state = ModifiersState::SHIFT | ModifiersState::CONTROL;
        let modifiers = modifiers(state);
        assert!(modifiers.shift);
        assert!(modifiers.ctrl);
        assert!(!modifiers.alt);
        assert!(!modifiers.meta);
    }

    #[test]
    fn pointer_coordinates_scale_to_logical() {
        let logical = to_logical(PhysicalPosition::new(40.0, 60.0), 2.0);
        assert_eq!(logical, Vec2::new(20.0, 30.0));
    }
}
