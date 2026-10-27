//! `igui_backend_canvas` — HTML Canvas 2D render backend.
//!
//! Maps the backend-neutral [`igui_render::DrawList`] onto the Canvas 2D API.
//! Browser dependencies (`web-sys`) are confined to this crate (and `igui_wasm`),
//! which is the only place a `CanvasRenderingContext2d` may appear.
//!
//! The backend is split in two: [`ops`] resolves each `DrawCommand` into a
//! Canvas operation on the CPU (no `web-sys`, so it is covered by native
//! `cargo test`), and `canvas` only executes those operations. Behind `wasm32`
//! both exist; on every other target the crate keeps the pure half —
//! [`font_spec`] — so a host can build a `TextMeasurer` that matches the
//! browser's font matching.

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui_backend_canvas";

// Only the wasm executor consumes the op builders, but they are tested on every
// target, so they must exist (and stay warning-free) off `wasm32` too.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod ops;

pub use ops::font_spec;

#[cfg(target_arch = "wasm32")]
mod canvas;

#[cfg(target_arch = "wasm32")]
pub use canvas::{Canvas2dBackend, CanvasError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "igui_backend_canvas");
    }

    #[test]
    fn the_font_spec_is_available_off_wasm() {
        // The font spec is the backend-neutral half of this crate: a host
        // measurer must be able to use it without a browser.
        assert_eq!(
            font_spec(14.0, igui_core::FontWeight::NORMAL),
            "400 14px sans-serif"
        );
    }
}
