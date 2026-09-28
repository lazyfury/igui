//! `quill` — the application facade over the fine-grained core crates.
//!
//! The core stays deliberately split so its dependency boundaries stay
//! enforceable. Applications depend on this one crate with opt-in features
//! instead of listing every crate:
//!
//! | feature | adds |
//! |---|---|
//! | `ui` | `rough_core`, `rough_render`, `rough_scene`, `rough_theme`, `rough_ui`, `rough_components` |
//! | `anim` | `rough_anim` (+ the `rough_core` / `rough_scene` it targets) |
//! | `game` | `rough_game` + `rough_assets` (+ `rough_core` / `rough_render` / `rough_scene`) |
//! | `app` | `rough_app` — the plugin-based `App` runtime (+ `rough_core` / `rough_render`) |
//! | `headless` | `rough_headless` — a recording `Presenter` for headless self-checks (implies `app`) |
//!
//! Disabled crates are not compiled at all. A UI-only app enables `ui` and a
//! backend; it never enables `game` or `anim`. `game` does **not** imply `ui`,
//! and `anim` is independent of both. This crate contains no logic — only
//! re-exports.
//!
//! ```toml
//! # UI app
//! quill = { path = ".../quill", default-features = false, features = ["ui"] }
//! # 2D game
//! quill = { path = ".../quill", default-features = false, features = ["game"] }
//! # plugin runtime + headless self-check
//! quill = { path = ".../quill", default-features = false, features = ["ui", "app", "headless"] }
//! ```

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "quill";

#[cfg(any(feature = "ui", feature = "anim", feature = "game", feature = "app"))]
pub use rough_core;

#[cfg(any(feature = "ui", feature = "anim", feature = "game"))]
pub use rough_scene;

#[cfg(any(feature = "ui", feature = "game", feature = "app"))]
pub use rough_render;

#[cfg(feature = "ui")]
pub use rough_theme;

#[cfg(feature = "ui")]
pub use rough_ui;

#[cfg(feature = "ui")]
pub use rough_components;

#[cfg(feature = "anim")]
pub use rough_anim;

#[cfg(feature = "game")]
pub use rough_game;

#[cfg(feature = "game")]
pub use rough_assets;

#[cfg(feature = "app")]
pub use rough_app;

#[cfg(feature = "headless")]
pub use rough_headless;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "quill");
    }
}
