//! `igui_scene` — the scene tree: [`Node`], [`SceneTree`], [`CanvasItem`] and
//! `Node2D` (via [`NodeKind::Node2D`]).
//!
//! May depend on `igui_core` and `igui_render` (the Paint step, Scene -> DrawList,
//! lives here by design). Must not depend on any browser or concrete-backend API,
//! so the whole scene graph stays testable with native `cargo test`.
//!
//! # Model
//!
//! Nodes live in a [`SceneTree`] arena and are addressed by [`igui_core::NodeId`].
//! A `Node2D` carries a [`CanvasItem`] with a local [`igui_core::Transform2D`],
//! visibility and z-index. [`SceneTree::update`] walks the tree once and derives
//! `world_transform` / `world_visible`, using [`DirtyFlags`] to skip clean nodes.

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui_scene";

mod child;
mod input;
mod node;
mod paint;
mod tree;
mod viewport;

pub use child::SceneChild;
pub use input::GuiInput;
pub use node::{
    AnchorMode, Camera2DData, CanvasItem, CanvasLayerData, DirtyFlags, Node, NodeKind, Visual,
};
pub use paint::{paint_visual, PaintItem};
pub use tree::{PreorderIter, SceneTree};
pub use viewport::Viewport;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "igui_scene");
    }
}
