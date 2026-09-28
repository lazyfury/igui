//! `igui_backend_recording` — a headless [`RenderBackend`] for tests.
//!
//! Records each frame's viewport and concatenated [`DrawList`] commands so the
//! full `Scene -> DrawList -> RenderBackend` pipeline can be verified with
//! native `cargo test`, no browser required.
//!
//! [`DrawList`]: igui_render::DrawList

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui_backend_recording";

mod assert;
mod backend;

pub use assert::CommandAsserts;
pub use backend::{
    RecordedFrame, RecordingBackend, RecordingError, RegisteredTarget, RegisteredTexture,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "igui_backend_recording");
    }
}
