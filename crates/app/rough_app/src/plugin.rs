//! The plugin trait: one unit of composition.

use crate::app::AppBuilder;

/// A unit of composition. A plugin registers the services, observers and layers
/// it owns into the [`AppBuilder`]; it never owns the application loop.
///
/// Plugins are added in registration order. [`build`](Plugin::build) runs as
/// each plugin is added; [`finish`](Plugin::finish) runs once every plugin has
/// been built, so a plugin may wire against a service another plugin
/// registered.
pub trait Plugin: 'static {
    /// A stable name used in diagnostics.
    fn name(&self) -> &'static str;

    /// Registers this plugin's services, observers and layers.
    fn build(&self, app: &mut AppBuilder);

    /// Runs after every plugin's [`build`](Plugin::build), in registration
    /// order.
    fn finish(&self, _app: &mut AppBuilder) {}
}
