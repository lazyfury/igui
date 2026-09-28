//! `quill` — the application facade over the fine-grained core crates.
//!
//! The core stays deliberately split so its dependency boundaries stay
//! enforceable. Applications depend on this one crate with opt-in features
//! instead of listing every crate:
//!
//! | feature | adds |
//! |---|---|
//! | `ui` | `cobbled_core`, `cobbled_render`, `cobbled_scene`, `cobbled_theme`, `cobbled_ui`, `cobbled_components` |
//! | `anim` | `cobbled_anim` (+ the `cobbled_core` / `cobbled_scene` it targets) |
//! | `game` | `cobbled_game` + `cobbled_assets` (+ `cobbled_core` / `cobbled_render` / `cobbled_scene`) |
//! | `app` | `cobbled_app` — the plugin-based `App` runtime (+ `cobbled_core` / `cobbled_render`) |
//! | `headless` | `cobbled_headless` — a recording `Presenter` for headless self-checks (implies `app`) |
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
pub use cobbled_core;

#[cfg(any(feature = "ui", feature = "anim", feature = "game"))]
pub use cobbled_scene;

#[cfg(any(feature = "ui", feature = "game", feature = "app"))]
pub use cobbled_render;

#[cfg(feature = "ui")]
pub use cobbled_theme;

#[cfg(feature = "ui")]
pub use cobbled_ui;

#[cfg(feature = "ui")]
pub use cobbled_components;

#[cfg(feature = "anim")]
pub use cobbled_anim;

#[cfg(feature = "game")]
pub use cobbled_game;

#[cfg(feature = "game")]
pub use cobbled_assets;

#[cfg(feature = "app")]
pub use cobbled_app;

#[cfg(feature = "headless")]
pub use cobbled_headless;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "quill");
    }
}
