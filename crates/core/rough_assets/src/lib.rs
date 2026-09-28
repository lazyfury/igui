//! `rough_assets` — backend-neutral asset decoding.
//!
//! Decodes image files into tightly packed RGBA8 bytes plus their dimensions,
//! so a host can upload the result through the backend-neutral
//! `RenderBackend::register_texture` contract without any codec or backend
//! dependency leaking into `rough_game`.
//!
//! It depends only on `rough_core` (for [`rough_core::Size`]) and the pure-Rust
//! `png` decoder; it never touches a browser, GPU or window. Hosts read the
//! bytes (file / network) and pass them to [`decode_png`].
//!
//! ```ignore
//! let png = std::fs::read("player.png")?;
//! let image = rough_assets::decode_png(&png)?;
//! backend.register_texture(texture, image.width(), image.height(), image.rgba8())?;
//! ```

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "rough_assets";

mod image;
mod png;

pub use image::{AssetError, DecodedImage};
pub use png::decode_png;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "rough_assets");
    }
}
