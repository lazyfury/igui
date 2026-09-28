//! Shared `winit` + `wgpu` window hosting for quill applications.
//!
//! This is **not** a core crate: it depends on `winit` and the `wgpu` backend,
//! and it never belongs in a backend-neutral layer. It exists so the window
//! hosts (`examples/wgpu_demo`, `examples/file_browser`,
//! `examples/game_demo`, the standalone `deepseek_balance`, and the sibling
//! `image_editor` / `archiver` / `classic-game-box` checkouts) stop copying the
//! same platform code:
//!
//! - [`Host`] owns window / surface / backend / swap-chain lifecycle and
//!   presents a `DrawList`, with surface-loss recovery;
//! - [`Host::translate`] maps platform events to backend-neutral
//!   [`InputEvent`](draw_core::InputEvent)s, including committed text and IME
//!   composition;
//! - [`Host::sync_ime`] places the IME candidate window at the focused control's
//!   caret (from `draw_ui::focused_caret`);
//! - [`input`] exposes the raw mappings for a host that keeps its own loop;
//! - [`FrameClock`] provides the capped frame delta.
//!
//! The *application* is intentionally not abstracted: each host keeps its own
//! `ApplicationHandler`, view model and draw loop. See [`Host`] for a full
//! example skeleton.

pub mod clipboard;
pub mod host;
pub mod input;

pub use clipboard::SystemClipboard;
pub use host::{FrameClock, Host, HostOptions, RenderOutcome, TitlebarMode, TITLEBAR_SAFE_AREA};
