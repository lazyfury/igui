//! Shared `winit` + `wgpu` platform plugins for the `rough_app` runtime.
//!
//! This is **not** a core crate: it depends on `winit` and the `wgpu` backend,
//! and it never belongs in a backend-neutral layer. It provides the platform
//! plugins that used to be one monolithic `Host`:
//!
//! - [`WinitPlugin`] — event loop, window creation and lifecycle (no `wgpu`);
//! - [`WgpuPlugin`] — surface, backend, swap chain and a
//!   [`Presenter`](rough_app::Presenter);
//! - [`PointerPlugin`] / [`KeyboardPlugin`] / [`ImePlugin`] — native event →
//!   backend-neutral [`InputEvent`](rough_core::InputEvent) translation;
//! - [`TextMeasurePlugin`] — the backend's font metrics as a
//!   [`TextMeasurer`](rough_ui::TextMeasurer);
//! - [`ClipboardPlugin`] — the system clipboard.
//!
//! Minimal assembly:
//!
//! ```no_run
//! # use rough_winit::{WinitPlugin, WgpuPlugin, PointerPlugin, KeyboardPlugin};
//! # use rough_winit::{ImePlugin, TextMeasurePlugin, ClipboardPlugin, WindowConfig};
//! # struct MyApp;
//! # impl rough_app::AppLogic for MyApp {}
//! rough_app::App::new(rough_app::AppConfig::default())
//!     .plugin(WinitPlugin::new(WindowConfig { ime: true, ..Default::default() }))
//!     .plugin(WgpuPlugin::default())
//!     .plugin(PointerPlugin)
//!     .plugin(KeyboardPlugin)
//!     .plugin(ImePlugin)
//!     .plugin(TextMeasurePlugin)
//!     .plugin(ClipboardPlugin)
//!     .logic(MyApp)
//!     .build()
//!     .run();
//! ```
//!
//! Each plugin reads the shared services the earlier ones publish
//! ([`SharedWindow`], [`SharedWindowState`], [`SharedBackend`]); register
//! `WinitPlugin` first, then `WgpuPlugin`, then the input / text plugins.
//!
//! [`input`] keeps the pure mappings so a host that keeps its own loop can
//! reuse them.

pub mod clipboard;
pub mod ime;
pub mod input;
pub mod keyboard;
pub mod pointer;
pub mod text_measure;
pub mod wgpu;
pub mod window;
pub mod winit_plugin;

pub use clipboard::{ClipboardPlugin, SystemClipboard};
pub use ime::ImePlugin;
pub use keyboard::KeyboardPlugin;
pub use pointer::PointerPlugin;
pub use text_measure::{BackendTextMeasurer, TextMeasurePlugin};
pub use wgpu::{GpuConfig, SharedBackend, WgpuPlugin};
pub use window::{
    SharedWindow, SharedWindowState, TitlebarMode, WindowConfig, WindowState, TITLEBAR_SAFE_AREA,
};
pub use winit_plugin::WinitPlugin;
