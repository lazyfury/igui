//! `igui_debug_ui` — backend-neutral debug visuals for igui.
//!
//! Two independent overlays, both drawn as ordinary backend-neutral
//! `DrawCommand`s (no browser/backend/GPU dependency, verified with native
//! `cargo test`):
//!
//! - [`DebugOverlay`] — **component debug drawing**: a yellow border around
//!   every visible `Control` plus a `Name #id` label in its top-left corner. It
//!   wraps [`igui_ui::paint_debug`](igui_ui::paint_debug) and draws over the
//!   application's own UI.
//! - [`PerformanceOverlay`] — the frame-timing / inspection panel fed by
//!   [`igui_profile`]; it owns its own `Ui` tree and is painted after the app UI.
//!
//! ```no_run
//! # use igui_core::{Size, ViewportSize};
//! # use igui_debug_ui::{DebugOverlay, PerformanceOverlay};
//! # use igui_profile::{inspect, FrameStats, Profiler};
//! # use igui_render::{DrawList, PaintContext};
//! # use igui_scene::SceneTree;
//! # let app_ui = SceneTree::new();
//! # let mut ctx = PaintContext::new();
//! # let profiler = Profiler::new();
//! # let report = inspect(&DrawList::default(), &FrameStats::default());
//! # let viewport = ViewportSize::new(Size::new(800.0, 600.0));
//! let mut debug = DebugOverlay::new();           // component bounds
//! let mut perf = PerformanceOverlay::new();      // profiler panel
//!
//! // per frame, after painting the app UI into `ctx`:
//! debug.paint(&app_ui, &mut ctx);
//! perf.update(&profiler, &report, viewport);
//! perf.paint(&mut ctx);
//! ```

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui_debug_ui";

mod component;
mod performance;

pub use component::DebugOverlay;
pub use performance::{Corner, OverlayConfig, OverlayText, PerformanceOverlay};
