//! `igui` — the application facade over the fine-grained core crates.
//!
//! The core stays deliberately split so its dependency boundaries stay
//! enforceable. Applications depend on this one crate with opt-in features
//! instead of listing every crate:
//!
//! | feature | adds |
//! |---|---|
//! | `ui` | `igui_core`, `igui_render`, `igui_scene`, `igui_theme`, `igui_ui`, `igui_components` |
//! | `anim` | `igui_anim` (+ the `igui_core` / `igui_scene` it targets) |
//! | `game` | `igui_game` + `igui_assets` (+ `igui_core` / `igui_render` / `igui_scene`) |
//! | `app` | `igui_app` — the plugin-based `App` runtime (+ `igui_core` / `igui_render`) |
//! | `headless` | `igui_headless` — a recording `Presenter` for headless self-checks (implies `app`) |
//!
//! Disabled crates are not compiled at all. A UI-only app enables `ui` and a
//! backend; it never enables `game` or `anim`. `game` does **not** imply `ui`,
//! and `anim` is independent of both. This crate contains no logic — only
//! re-exports.
//!
//! ```toml
//! # UI app
//! igui = { path = ".../igui", default-features = false, features = ["ui"] }
//! # 2D game
//! igui = { path = ".../igui", default-features = false, features = ["game"] }
//! # plugin runtime + headless self-check
//! igui = { path = ".../igui", default-features = false, features = ["ui", "app", "headless"] }
//! ```

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui";

#[cfg(any(feature = "ui", feature = "anim", feature = "game", feature = "app"))]
pub use igui_core;

#[cfg(any(feature = "ui", feature = "anim", feature = "game"))]
pub use igui_scene;

#[cfg(any(feature = "ui", feature = "game", feature = "app"))]
pub use igui_render;

#[cfg(feature = "ui")]
pub use igui_theme;

#[cfg(feature = "ui")]
pub use igui_ui;

#[cfg(feature = "ui")]
pub use igui_components;

#[cfg(feature = "anim")]
pub use igui_anim;

#[cfg(feature = "game")]
pub use igui_game;

#[cfg(feature = "game")]
pub use igui_assets;

#[cfg(feature = "app")]
pub use igui_app;

#[cfg(feature = "headless")]
pub use igui_headless;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "igui");
    }
}
