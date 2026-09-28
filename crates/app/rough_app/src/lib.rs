//! `cobbled_app` — the application runtime: assemble an app from plugins.
//!
//! This is a **non-core** crate: it depends only on `cobbled_core` + `cobbled_render`
//! and never names `winit`, `wgpu` or a browser API. It exists so an application
//! is *assembled* rather than inherited:
//!
//! ```no_run
//! # use cobbled_app::{App, AppConfig};
//! # struct WinitPlugin; struct PointerPlugin; struct MyApp;
//! # impl cobbled_app::Plugin for WinitPlugin { fn name(&self) -> &'static str { "winit" } fn build(&self, _: &mut cobbled_app::AppBuilder) {} }
//! # impl cobbled_app::Plugin for PointerPlugin { fn name(&self) -> &'static str { "pointer" } fn build(&self, _: &mut cobbled_app::AppBuilder) {} }
//! # impl cobbled_app::AppLogic for MyApp {}
//! App::new(AppConfig::default())
//!     .plugin(WinitPlugin)
//!     .plugin(PointerPlugin)
//!     .logic(MyApp)
//!     .build()
//!     .run();
//! ```
//!
//! - [`Plugin`] registers services, observers and layers into an [`AppBuilder`].
//! - [`AppLogic`] is the application's own `update -> layout -> paint` code; it
//!   never touches a platform type.
//! - [`Presenter`] is the graphics boundary (a window surface or a recorder);
//!   the runtime hands it a `DrawList` and does not know which it is.
//! - [`PlatformEvent`] is an opaque bridge a platform plugin (`cobbled_winit`)
//!   uses to observe native events without the runtime knowing their type.
//!
//! The real platform plugins live in `cobbled_winit` (window + wgpu + input) and
//! `cobbled_headless` (recording); this crate is what makes them swappable.

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "cobbled_app";

pub mod app;
pub mod logic;
pub mod platform;
pub mod plugin;
pub mod service;

pub use app::{App, AppBuilder, AppConfig, FrameClock};
pub use logic::{AppLogic, EventContext, EventResult, FrameContext, InitContext};
pub use platform::{
    FrameObserver, InputLayer, LifecycleObserver, PaintLayer, PlatformEvent, PlatformObserver,
    PresentOutcome, Presenter, Runner,
};
pub use plugin::Plugin;
pub use service::ServiceMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "cobbled_app");
    }
}
