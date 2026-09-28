//! Directional keyboard focus navigation (Godot-style focus neighbors).
//!
//! A focused control can name its neighbors — `focus_neighbor_up("home")` — so
//! the arrow keys follow an explicit path instead of the layout, exactly like a
//! game menu where `up -> home`, `right -> play`, `down -> next`. Tab and
//! Shift+Tab cycle the focusable controls in tab order.
//!
//! When a named neighbor is unset, hidden or disabled, the move falls back to a
//! spatial search over the resolved rectangles, so an ordinary form still
//! navigates with no wiring at all. The candidate set is always visible,
//! enabled, focusable controls.
//!
//! Wiring lives on each [`Control`](crate::Control) ([`FocusNav`]); this module
//! is the resolver.
//! It reads the resolved rectangles, so run [`layout`](crate::layout) first.

use igui_core::{NodeId, Vec2};
use igui_scene::SceneTree;

use crate::control::{control_mut, control_of, control_visible, gui_state, gui_state_mut};
use igui_core::Rect;

/// A direction for [`focus_move`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDir {
    Up,
    Down,
    Left,
    Right,
    /// Next control in tab order (wraps).
    Next,
    /// Previous control in tab order (wraps).
    Prev,
}

/// Per-control keyboard-focus wiring, stored on [`Control`](crate::Control).
///
/// `name` is a logical key an explicit neighbor refers to; the four directional
/// fields name the control to move to in that direction. A `None` neighbor (or
/// one that no longer resolves to a usable control) falls back to the spatial
/// search. `tab_index` overrides tree order for Tab / Shift+Tab: controls with a
/// value sort first, ascending, and the rest follow in tree order.
///
/// **Non-breaking addition to `igui_ui`** (`Control` gained `focus`, and the
/// resolver functions in this module are new); recorded in
/// `docs/design-system.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusNav {
    /// Logical name an explicit neighbor refers to (`"home"`).
    pub name: Option<String>,
    /// Explicit neighbor name for the up direction.
    pub up: Option<String>,
    /// Explicit neighbor name for the down direction.
    pub down: Option<String>,
    /// Explicit neighbor name for the left direction.
    pub left: Option<String>,
    /// Explicit neighbor name for the right direction.
    pub right: Option<String>,
    /// Tab order override (lower first); `None` falls back to tree order.
    pub tab_index: Option<i32>,
}

impl FocusNav {
    /// Empty wiring: no name, no explicit neighbors, tree-order tab.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the logical name other controls refer to.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Names the neighbor reached by the up direction.
    pub fn above(mut self, name: impl Into<String>) -> Self {
        self.up = Some(name.into());
        self
    }

    /// Names the neighbor reached by the down direction.
    pub fn below(mut self, name: impl Into<String>) -> Self {
        self.down = Some(name.into());
        self
    }

    /// Names the neighbor reached by the left direction.
    pub fn left_of(mut self, name: impl Into<String>) -> Self {
        self.left = Some(name.into());
        self
    }

    /// Names the neighbor reached by the right direction.
    pub fn right_of(mut self, name: impl Into<String>) -> Self {
        self.right = Some(name.into());
        self
    }

    /// Overrides the Tab order (lower sorts first).
    pub fn tab_index(mut self, index: i32) -> Self {
        self.tab_index = Some(index);
        self
    }

    /// Whether this carries no wiring at all (the default, cost-free case).
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.up.is_none()
            && self.down.is_none()
            && self.left.is_none()
            && self.right.is_none()
            && self.tab_index.is_none()
    }

    fn neighbor(&self, dir: FocusDir) -> Option<&str> {
        match dir {
            FocusDir::Up => self.up.as_deref(),
            FocusDir::Down => self.down.as_deref(),
            FocusDir::Left => self.left.as_deref(),
            FocusDir::Right => self.right.as_deref(),
            FocusDir::Next | FocusDir::Prev => None,
        }
    }
}

/// Installs focus wiring on `id`.
///
/// Returns `false` when `id` is not a control.
pub fn set_focus_nav(tree: &mut SceneTree, id: NodeId, nav: FocusNav) -> bool {
    match control_mut(tree, id) {
        Some(control) => {
            control.focus = nav;
            true
        }
        None => false,
    }
}

/// The focus wiring on `id`, if it is a control.
pub fn focus_nav<'a>(tree: &'a SceneTree, id: NodeId) -> Option<&'a FocusNav> {
    control_of(tree, id).map(|control| &control.focus)
}

/// Moves keyboard focus to `id` when it is a usable focus target.
///
/// A disabled control, one hidden in the tree, one clipped away entirely, or
/// one that is not [`focusable`](crate::Control::focusable) is rejected and focus is
/// left untouched. Returns whether the focus moved.
pub fn set_focus(tree: &mut SceneTree, id: NodeId) -> bool {
    if !is_focusable(tree, id) {
        return false;
    }
    gui_state_mut(tree).focused = Some(id);
    true
}

/// Every focus target, in tree (pre-order) order.
///
/// The candidate set for [`focus_move`]: visible, enabled, focusable controls
/// that are not clipped away.
pub fn focusable_nodes(tree: &SceneTree) -> Vec<NodeId> {
    tree.iter().filter(|id| is_focusable(tree, *id)).collect()
}

/// Moves focus in `dir`, returning whether it moved.
///
/// A directional move tries the current control's explicit neighbor for that
/// direction first; when it is unset (or no longer resolves to a usable
/// control) it falls back to the nearest focus target in that direction. With
/// nothing focused, any move focuses the first target. Tab / Shift+Tab
/// ([`FocusDir::Next`] / [`FocusDir::Prev`]) walk the tab order and wrap.
pub fn focus_move(tree: &mut SceneTree, dir: FocusDir) -> bool {
    let current = gui_state(tree)
        .and_then(|state| state.focused)
        .map(|id| focus_owner(tree, id));
    match dir {
        FocusDir::Next | FocusDir::Prev => focus_step(tree, dir, current),
        _ => focus_directional(tree, dir, current),
    }
}

/// Moves focus up.
pub fn focus_up(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Up)
}

/// Moves focus down.
pub fn focus_down(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Down)
}

/// Moves focus left.
pub fn focus_left(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Left)
}

/// Moves focus right.
pub fn focus_right(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Right)
}

/// Moves focus to the next target in tab order (wraps).
pub fn focus_next(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Next)
}

/// Moves focus to the previous target in tab order (wraps).
pub fn focus_prev(tree: &mut SceneTree) -> bool {
    focus_move(tree, FocusDir::Prev)
}

/// Nearest ancestor (including `id`) that carries focus wiring.
///
/// A pointer click focuses the topmost control, which for a themed button is
/// its inner label — the `FocusNav` lives on the button root, so navigation
/// reads it from the nearest ancestor that is focusable or named.
fn focus_owner(tree: &SceneTree, id: NodeId) -> NodeId {
    let mut current = Some(id);
    while let Some(node) = current {
        if control_of(tree, node)
            .is_some_and(|control| control.focusable || !control.focus.is_empty())
        {
            return node;
        }
        current = tree.parent(node);
    }
    id
}

fn focus_directional(tree: &mut SceneTree, dir: FocusDir, current: Option<NodeId>) -> bool {
    let Some(current) = current else {
        return match focusable_nodes(tree).into_iter().next() {
            Some(id) => set_focus(tree, id),
            None => false,
        };
    };
    // Explicit neighbor by name wins when it resolves to a usable control; an
    // unset name, or one pointing at a hidden / disabled control, falls through.
    let explicit = control_of(tree, current)
        .and_then(|control| control.focus.neighbor(dir))
        .map(str::to_owned);
    if let Some(name) = explicit {
        if let Some(target) = find_by_name(tree, &name) {
            if target != current && is_focusable(tree, target) {
                return set_focus(tree, target);
            }
        }
    }
    // Spatial fallback: the nearest focus target in that direction.
    match nearest_in_direction(tree, current, dir) {
        Some(target) => set_focus(tree, target),
        None => false,
    }
}

fn focus_step(tree: &mut SceneTree, dir: FocusDir, current: Option<NodeId>) -> bool {
    let mut nodes = focusable_nodes(tree);
    if nodes.is_empty() {
        return false;
    }
    // Stable sort keeps tree order for controls without a `tab_index`; those
    // with one sort first, ascending.
    nodes.sort_by_key(
        |id| match control_of(tree, *id).and_then(|c| c.focus.tab_index) {
            Some(index) => (0u8, index),
            None => (1u8, 0),
        },
    );
    let position = current.and_then(|current| nodes.iter().position(|id| *id == current));
    let next = match (dir, position) {
        (FocusDir::Next, Some(index)) => (index + 1) % nodes.len(),
        (FocusDir::Next, None) => 0,
        (FocusDir::Prev, Some(index)) => (index + nodes.len() - 1) % nodes.len(),
        (FocusDir::Prev, None) => nodes.len() - 1,
        _ => return false,
    };
    set_focus(tree, nodes[next])
}

fn find_by_name(tree: &SceneTree, name: &str) -> Option<NodeId> {
    tree.iter().find(|id| {
        control_of(tree, *id).is_some_and(|control| control.focus.name.as_deref() == Some(name))
    })
}

/// Nearest focus target whose center is in `dir` from `current`'s center.
///
/// Score is the axis distance plus twice the perpendicular offset, so a control
/// aligned with the current one beats a diagonal one that is closer in a
/// straight line. Ties go to tree order (the iteration order).
fn nearest_in_direction(tree: &SceneTree, current: NodeId, dir: FocusDir) -> Option<NodeId> {
    let from = control_of(tree, current)?.data.rect.center();
    let mut best: Option<(f32, NodeId)> = None;
    for id in tree.iter() {
        if id == current || !is_focusable(tree, id) {
            continue;
        }
        let Some(control) = control_of(tree, id) else {
            continue;
        };
        let at = control.data.rect.center();
        let Some((primary, secondary)) = direction_distance(from, at, dir) else {
            continue;
        };
        let score = primary + 2.0 * secondary;
        if best.map_or(true, |(best_score, _)| score < best_score) {
            best = Some((score, id));
        }
    }
    best.map(|(_, id)| id)
}

/// `Some((axis distance, perpendicular offset))` when `to` is strictly in `dir`
/// from `from`, else `None`.
fn direction_distance(from: Vec2, to: Vec2, dir: FocusDir) -> Option<(f32, f32)> {
    match dir {
        FocusDir::Up if to.y < from.y => Some((from.y - to.y, (to.x - from.x).abs())),
        FocusDir::Down if to.y > from.y => Some((to.y - from.y, (to.x - from.x).abs())),
        FocusDir::Left if to.x < from.x => Some((from.x - to.x, (to.y - from.y).abs())),
        FocusDir::Right if to.x > from.x => Some((to.x - from.x, (to.y - from.y).abs())),
        _ => None,
    }
}

/// Whether `id` can take keyboard focus: a control that is focusable, visible,
/// enabled (itself or via an ancestor), and not clipped away entirely.
fn is_focusable(tree: &SceneTree, id: NodeId) -> bool {
    let Some(control) = control_of(tree, id) else {
        return false;
    };
    if !control.focusable || !control_visible(tree, id) {
        return false;
    }
    if control.data.clip_rect.is_some_and(Rect::is_empty) {
        return false;
    }
    !disabled_in_tree(tree, id)
}

fn disabled_in_tree(tree: &SceneTree, id: NodeId) -> bool {
    let mut current = Some(id);
    while let Some(node) = current {
        if control_of(tree, node).is_some_and(|control| control.data.disabled) {
            return true;
        }
        current = tree.parent(node);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Container, ContentRef, PanelContent};
    use crate::control::{Control, ControlData};
    use crate::layout;
    use igui_core::{Color, Edges, Size, ViewportSize};

    fn panel() -> Option<ContentRef> {
        Some(Box::new(PanelContent {
            color: Color::RED,
            border: None,
        }))
    }

    fn host() -> (SceneTree, NodeId) {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let host = tree.add_control(root, "host");
        tree.set_data(
            host,
            Control::new(ControlData::fill_parent(), Container::Leaf, panel()),
        );
        (tree, host)
    }

    /// A focusable control at an absolute rectangle (anchored top-left).
    fn target(tree: &mut SceneTree, parent: NodeId, rect: [f32; 4]) -> NodeId {
        let id = tree.add_control(parent, "target");
        let data = ControlData {
            anchors: Edges::ZERO,
            offsets: Edges::new(rect[0], rect[1], rect[2], rect[3]),
            ..ControlData::default()
        };
        let mut control = Control::new(data, Container::Leaf, panel());
        control.focusable = true;
        tree.set_data(id, control);
        id
    }

    fn layout_400(tree: &mut SceneTree) {
        layout(tree, ViewportSize::new(Size::new(400.0, 400.0)));
    }

    fn focused_of(tree: &SceneTree) -> Option<NodeId> {
        crate::input::focused(tree)
    }

    /// The named neighbor wins over geometry: a control physically closer in the
    /// direction must not be chosen when the focus names another one.
    #[test]
    fn explicit_neighbors_override_geometry() {
        let (mut tree, host) = host();
        let home = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        let closer = target(&mut tree, host, [60.0, 0.0, 90.0, 40.0]);
        let named = target(&mut tree, host, [200.0, 0.0, 250.0, 40.0]);
        set_focus_nav(
            &mut tree,
            home,
            FocusNav::new().named("home").right_of("play"),
        );
        set_focus_nav(&mut tree, closer, FocusNav::new().named("closer"));
        set_focus_nav(&mut tree, named, FocusNav::new().named("play"));
        layout_400(&mut tree);

        assert!(set_focus(&mut tree, home));
        assert!(focus_right(&mut tree));
        assert_eq!(
            focused_of(&tree),
            Some(named),
            "followed the name, not the layout"
        );
    }

    /// Each direction finds the target on that side when nothing is named.
    #[test]
    fn spatial_search_follows_each_direction() {
        let cases = [
            (FocusDir::Up, [80.0, 0.0, 120.0, 40.0]),
            (FocusDir::Down, [80.0, 160.0, 120.0, 200.0]),
            (FocusDir::Left, [0.0, 80.0, 40.0, 120.0]),
            (FocusDir::Right, [160.0, 80.0, 200.0, 120.0]),
        ];
        for (dir, rect) in cases {
            let (mut tree, host) = host();
            let center = target(&mut tree, host, [80.0, 80.0, 120.0, 120.0]);
            let wanted = target(&mut tree, host, rect);
            layout_400(&mut tree);
            assert!(set_focus(&mut tree, center));
            assert!(focus_move(&mut tree, dir));
            assert_eq!(focused_of(&tree), Some(wanted), "dir {dir:?}");
        }
    }

    /// A target aligned on the axis beats an equally close off-axis one.
    #[test]
    fn spatial_search_prefers_the_aligned_target() {
        let (mut tree, host) = host();
        let center = target(&mut tree, host, [80.0, 80.0, 120.0, 120.0]);
        let aligned = target(&mut tree, host, [80.0, 0.0, 120.0, 40.0]);
        let off_axis = target(&mut tree, host, [150.0, 0.0, 190.0, 40.0]);
        layout_400(&mut tree);

        assert!(set_focus(&mut tree, center));
        assert!(focus_up(&mut tree));
        assert_eq!(focused_of(&tree), Some(aligned));
        assert_ne!(focused_of(&tree), Some(off_axis));
    }

    /// A name that resolves to nothing falls back to the spatial search.
    #[test]
    fn a_missing_named_neighbor_falls_back_to_spatial() {
        let (mut tree, host) = host();
        let home = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        let right = target(&mut tree, host, [80.0, 0.0, 120.0, 40.0]);
        set_focus_nav(&mut tree, home, FocusNav::new().right_of("does-not-exist"));
        layout_400(&mut tree);

        assert!(set_focus(&mut tree, home));
        assert!(focus_right(&mut tree));
        assert_eq!(focused_of(&tree), Some(right));
    }

    /// A name that resolves to a disabled control falls back to the next
    /// spatial target, not to the disabled one.
    #[test]
    fn a_disabled_named_neighbor_falls_back_to_spatial() {
        let (mut tree, host) = host();
        let home = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        let blocked = target(&mut tree, host, [80.0, 0.0, 120.0, 40.0]);
        let fallback = target(&mut tree, host, [160.0, 0.0, 200.0, 40.0]);
        set_focus_nav(&mut tree, home, FocusNav::new().right_of("blocked"));
        set_focus_nav(&mut tree, blocked, FocusNav::new().named("blocked"));
        tree.data_mut::<Control>(blocked).unwrap().data.disabled = true;
        layout_400(&mut tree);

        assert!(set_focus(&mut tree, home));
        assert!(focus_right(&mut tree));
        assert_eq!(focused_of(&tree), Some(fallback));
    }

    /// Tab order is `tab_index` first (ascending), then tree order, and wraps.
    #[test]
    fn tab_order_respects_tab_index_and_wraps() {
        let (mut tree, host) = host();
        let a = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        let b = target(&mut tree, host, [0.0, 40.0, 40.0, 80.0]);
        let c = target(&mut tree, host, [0.0, 80.0, 40.0, 120.0]);
        set_focus_nav(&mut tree, c, FocusNav::new().tab_index(0));
        layout_400(&mut tree);

        assert!(set_focus(&mut tree, c));
        assert!(focus_next(&mut tree));
        assert_eq!(
            focused_of(&tree),
            Some(a),
            "after the tab-indexed control, tree order"
        );
        assert!(focus_next(&mut tree));
        assert_eq!(focused_of(&tree), Some(b));
        assert!(focus_next(&mut tree));
        assert_eq!(focused_of(&tree), Some(c), "wraps to the first tab target");
        assert!(focus_prev(&mut tree));
        assert_eq!(focused_of(&tree), Some(b));
    }

    #[test]
    fn a_move_with_nothing_focused_focuses_the_first_target() {
        let (mut tree, host) = host();
        let first = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        target(&mut tree, host, [0.0, 40.0, 40.0, 80.0]);
        layout_400(&mut tree);

        assert_eq!(focused_of(&tree), None);
        assert!(focus_down(&mut tree));
        assert_eq!(focused_of(&tree), Some(first));
    }

    /// Disabled and hidden controls are never focus targets.
    #[test]
    fn disabled_and_hidden_targets_are_not_focusable() {
        let (mut tree, host) = host();
        let disabled = target(&mut tree, host, [0.0, 0.0, 40.0, 40.0]);
        let hidden = target(&mut tree, host, [0.0, 40.0, 40.0, 80.0]);
        let live = target(&mut tree, host, [0.0, 80.0, 40.0, 120.0]);
        tree.data_mut::<Control>(disabled).unwrap().data.disabled = true;
        tree.set_visible(hidden, false);
        layout_400(&mut tree);

        assert_eq!(focusable_nodes(&tree), vec![live]);
        assert!(!set_focus(&mut tree, disabled));
        assert!(!set_focus(&mut tree, hidden));
        assert!(set_focus(&mut tree, live));
    }
}
