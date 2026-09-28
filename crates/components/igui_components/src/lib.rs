//! `igui_components` — the component library: base builders + themed widgets.
//!
//! The single widget API on top of `igui_ui` (layout / paint / input). It ships
//! two layers:
//!
//! - **[`base`]** — the [`Component`] trait, [`Spec`] and the unstyled
//!   primitives ([`Flex`], [`Panel`], [`Label`],
//!   [`Button`](base::Button), [`Grid`], ...). Compose with `.child()` and
//!   attach with [`SceneTree::add_child`](igui_scene::SceneTree::add_child).
//! - **themed** — [`Text`], [`Card`], [`Button`], [`Checkbox`], [`Switch`],
//!   [`Divider`], [`Badge`], [`CodeBlock`], [`Terminal`], [`EmptyState`] and the
//!   [`Overlays`] layer. These read the active [`Theme`] as a value and attach
//!   their chrome with the `igui_ui` styling primitives. There is no runtime
//!   object, no theme on the tree and no second paint pass.
//!
//! ```ignore
//! use igui_components::{Card, Checkbox, Text};
//! use igui_scene::SceneTree;
//! use igui_theme::{space, Theme};
//!
//! let theme = Theme::dark();
//! let mut tree = SceneTree::new();
//! let root = tree.root();
//!
//! let panel = tree.add_child(root, Card::new(theme).gap(space::MD)
//!     .child(Text::heading("Settings", theme))
//!     .child(Checkbox::new("Verbose output", theme)));
//!
//! igui_ui::layout(&mut tree, viewport);
//! igui_ui::paint(&tree, &mut ctx);      // surfaces + content + marks, in tree order
//! igui_ui::route_input(&mut tree, &event);
//! ```
//!
//! ## Implemented
//!
//! - Text: [`Text`] (display/title/heading/subheading/body/small/caption).
//! - Surfaces: [`Card`], [`Divider`], [`Badge`], [`CodeBlock`], [`Terminal`],
//!   [`EmptyState`].
//! - Controls: [`Button`], [`Checkbox`], [`Switch`].
//! - Scroll: [`List`] (virtualized rows) and [`ScrollView`] (a clipped, offset
//!   viewport with a draggable scrollbar).
//! - Icons: [`Glyph`] + [`Icon`] — in-code vector geometry, no SVG files.
//! - Menus: [`Menu`] + [`MenuItem`], placed with [`Overlays::menu`].
//! - Floating: [`Overlays`] (`confirm`, `popover`, `menu`, `tips`, `message`).
//!
//! Inputs, selects, tabs and tables are staged next.

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "igui_components";

/// The unstyled construction layer: [`Component`], [`Spec`] and the base
/// primitives. See the [crate docs](crate) for the themed layer.
pub mod base;
mod components;
mod glyph;
pub mod node_ref;
mod overlay;
mod router;

pub use base::{
    apply_spec, control_mut, set_cursor_provider, set_on_click, set_on_drag, set_on_scroll,
    set_on_secondary, set_pointer_callback, set_text, set_text_color, update_control, CaretFn,
    ChildFn, Column, Component, Flex, Grid, HBox, ImeFn, KeyFn, Label, Panel, Row, Spec, TextFn,
    VBox,
};
pub use components::{
    set_button_text, set_disabled, Badge, Button, ButtonVariant, Card, CheckState, Checkbox,
    CodeBlock, Divider, EmptyState, Icon, List, ListColumn, ListLead, ListState, Menu, MenuItem,
    ResizeHandle, RowSource, ScrollView, ScrollViewState, Select, Switch, Terminal, Text, TextArea,
    TextInput, MENU_MIN_WIDTH,
};
pub use glyph::{paint_glyph, Glyph, GLYPH_VIEWBOX};
pub use node_ref::{NodeRef, Ref};
pub use overlay::{OverlayId, Overlays, Placement};
pub use router::Router;

pub use igui_core::FontWeight;
pub use igui_render::CornerRadii;
pub use igui_theme::{self as theme, SurfaceTone, Theme, Tone};

use igui_core::NodeId;
use igui_scene::SceneTree;
use igui_ui::Control;

/// Number of times a control has been activated (clicked / Enter).
///
/// Replaces the old `Widget`-based `button_state`, which carried hover/press
/// snapshots: those now come from [`igui_ui::state_for`], and only the
/// activation count still needs to persist on the control.
pub fn click_count(tree: &SceneTree, id: NodeId) -> u32 {
    tree.data::<Control>(id)
        .map_or(0, |control| control.click_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    use igui_core::{
        Cursor, Edges, EventResult, InputEvent, PointerButton, Size, Vec2, ViewportSize,
    };
    use igui_scene::Visual;
    use igui_ui::{focused, handle_input, hovered_cursor, route_input};
    use igui_ui::{MouseFilter, SizeBasis};

    // The tests exercise the unstyled primitives; the themed `Button` is a
    // different, theme-aware type at the crate root.
    use crate::base::Button;

    fn viewport(w: f32, h: f32) -> ViewportSize {
        ViewportSize::new(Size::new(w, h))
    }

    fn host() -> (SceneTree, NodeId) {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let root = tree.add_child(root, Flex::column().mouse_filter(MouseFilter::Ignore));
        (tree, root)
    }

    fn click(tree: &mut SceneTree, position: Vec2) {
        handle_input(
            tree,
            &InputEvent::PointerDown {
                position,
                button: PointerButton::Left,
            },
        );
        handle_input(
            tree,
            &InputEvent::PointerUp {
                position,
                button: PointerButton::Left,
            },
        );
    }

    #[test]
    fn build_layout_and_paint() {
        let (mut tree, root) = host();
        let panel = tree.add_child(
            root,
            Panel::new().child(
                VBox::new()
                    .child(Label::new("Hello"))
                    .child(Button::new("Click me")),
            ),
        );
        let vbox = tree.children(panel).unwrap()[0];
        let label = tree.children(vbox).unwrap()[0];
        let button = tree.children(vbox).unwrap()[1];
        igui_ui::layout(&mut tree, viewport(800.0, 600.0));
        tree.update();

        assert_eq!(
            igui_ui::control(&tree, panel).unwrap().rect.size,
            Size::new(800.0, 600.0)
        );
        assert!(
            igui_ui::control(&tree, label).unwrap().rect.top()
                < igui_ui::control(&tree, button).unwrap().rect.top()
        );

        let mut ctx = igui_render::PaintContext::new();
        igui_ui::paint(&tree, &mut ctx);
        let list = ctx.into_draw_list();
        assert!(list
            .commands()
            .iter()
            .any(|c| matches!(c, igui_render::DrawCommand::DrawText { .. })));
    }

    #[test]
    fn flex_grow_distributes_leftover() {
        let (mut tree, root) = host();
        let row = tree.add_child(root, Flex::row().gap(0.0).padding(igui_core::Edges::ZERO));
        let a = tree.add_child(row, Panel::new().basis(SizeBasis::Px(100.0)).shrink(0.0));
        let b = tree.add_child(row, Panel::new().basis(SizeBasis::Px(100.0)).grow(1.0));
        igui_ui::layout(&mut tree, viewport(300.0, 100.0));
        assert_eq!(igui_ui::control(&tree, a).unwrap().rect.size.width, 100.0);
        assert_eq!(igui_ui::control(&tree, b).unwrap().rect.size.width, 200.0);
    }

    #[test]
    fn click_fires_callback_and_focus() {
        let (mut tree, root) = host();
        let clicks = Rc::new(Cell::new(0));
        let counter = clicks.clone();
        let button = tree.add_child(
            root,
            Button::new("Click me").on_click(move || counter.set(counter.get() + 1)),
        );
        igui_ui::layout(&mut tree, viewport(400.0, 200.0));
        tree.update();

        let center = igui_ui::control(&tree, button).unwrap().rect.center();
        click(&mut tree, center);
        assert_eq!(clicks.get(), 1);
        assert_eq!(click_count(&tree, button), 1);
        assert_eq!(focused(&tree), Some(button));
    }

    #[test]
    fn drag_callback_accumulates_deltas_and_captures_the_pointer() {
        let (mut tree, root) = host();
        let handle = tree.add_child(
            root,
            Panel::new()
                .anchors(Edges::ZERO)
                .offsets(Edges::new(0.0, 0.0, 20.0, 20.0)),
        );
        let total = Rc::new(Cell::new(0.0f32));
        let acc = total.clone();
        set_on_drag(&mut tree, handle, move |_tree, _phase, delta| {
            acc.set(acc.get() + delta.x);
        });
        igui_ui::layout(&mut tree, viewport(200.0, 200.0));
        tree.update();

        handle_input(
            &mut tree,
            &InputEvent::PointerDown {
                position: Vec2::new(10.0, 10.0),
                button: PointerButton::Left,
            },
        );
        // A move inside the handle, then one far outside it: pointer capture
        // must keep routing to the handle.
        handle_input(
            &mut tree,
            &InputEvent::PointerMove {
                position: Vec2::new(20.0, 10.0),
            },
        );
        handle_input(
            &mut tree,
            &InputEvent::PointerMove {
                position: Vec2::new(60.0, 180.0),
            },
        );
        handle_input(
            &mut tree,
            &InputEvent::PointerUp {
                position: Vec2::new(60.0, 180.0),
                button: PointerButton::Left,
            },
        );

        assert!((total.get() - 50.0).abs() < 1e-3, "total = {}", total.get());
    }

    #[test]
    fn dynamic_cursor_provider_tracks_component_state() {
        let (mut tree, root) = host();
        let state = Rc::new(Cell::new(Cursor::Default));
        let handle = tree.add_child(
            root,
            Panel::new()
                .anchors(Edges::ZERO)
                .offsets(Edges::new(0.0, 0.0, 20.0, 20.0)),
        );
        let s = state.clone();
        set_cursor_provider(&mut tree, handle, move || s.get());
        igui_ui::layout(&mut tree, viewport(200.0, 200.0));
        tree.update();

        handle_input(
            &mut tree,
            &InputEvent::PointerMove {
                position: Vec2::new(10.0, 10.0),
            },
        );
        assert_eq!(hovered_cursor(&tree), Cursor::Default);

        // The component changes its own state; the cursor follows it.
        state.set(Cursor::ColResize);
        assert_eq!(hovered_cursor(&tree), Cursor::ColResize);

        // Leaving the control resets the cursor.
        handle_input(
            &mut tree,
            &InputEvent::PointerMove {
                position: Vec2::new(180.0, 180.0),
            },
        );
        assert_eq!(hovered_cursor(&tree), Cursor::Default);
    }

    #[test]
    fn route_input_prefers_the_world_pick_over_the_gui() {
        let (mut tree, root) = host();
        let clicks = Rc::new(Cell::new(0));
        let counter = clicks.clone();
        let button = tree.add_child(
            root,
            Button::new("Hit").on_click(move || counter.set(counter.get() + 1)),
        );
        igui_ui::layout(&mut tree, viewport(200.0, 200.0));
        tree.update();

        let world = tree.add_node2d(tree.root(), "World");
        tree.set_visual(
            world,
            Visual::Rect {
                size: Size::splat(200.0),
                color: igui_core::Color::RED,
            },
        );
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        tree.set_input_event(world, move |_| {
            h.set(h.get() + 1);
            EventResult::Handled
        });
        tree.update();

        let center = igui_ui::control(&tree, button).unwrap().rect.center();
        let down = InputEvent::PointerDown {
            position: center,
            button: PointerButton::Left,
        };
        let up = InputEvent::PointerUp {
            position: center,
            button: PointerButton::Left,
        };
        assert!(route_input(&mut tree, &down).is_handled());
        assert!(route_input(&mut tree, &up).is_handled());
        assert_eq!(hits.get(), 2);
        assert_eq!(clicks.get(), 0, "world consumed both events");
    }

    #[test]
    fn layout_cache_lives_on_the_tree() {
        let (mut tree, _root) = host();
        igui_ui::layout(&mut tree, viewport(100.0, 100.0));
        assert_eq!(igui_ui::layout_count(&tree), 1);
        igui_ui::layout(&mut tree, viewport(100.0, 100.0));
        assert_eq!(igui_ui::layout_count(&tree), 1, "cache hit");
    }

    #[test]
    fn chrome_paints_around_nodes_in_tree_order() {
        use std::cell::RefCell;

        let (mut tree, root) = host();
        let a = tree.add_child(root, Label::new("A"));
        let b = tree.add_child(root, Label::new("B"));
        let log = Rc::new(RefCell::new(Vec::new()));
        for (id, name) in [(a, "a"), (b, "b")] {
            let behind = log.clone();
            igui_ui::add_background(&mut tree, id, move |_state| {
                behind.borrow_mut().push(name);
                igui_ui::SurfaceStyle::new(igui_core::Color::TRANSPARENT)
            });
            let front = log.clone();
            igui_ui::add_foreground(&mut tree, id, move |_env, _rect, _state| {
                front.borrow_mut().push(name);
            });
        }
        igui_ui::layout(&mut tree, viewport(200.0, 200.0));
        tree.update();
        let mut ctx = igui_render::PaintContext::new();
        igui_ui::paint(&tree, &mut ctx);
        assert_eq!(&*log.borrow(), &["a", "a", "b", "b"]);
    }

    /// Named focus neighbors drive the arrows regardless of where the buttons
    /// sit: a game menu wires `up -> home`, `right -> play`, `down -> next`.
    #[test]
    fn named_directional_focus_overrides_the_layout() {
        let (mut tree, root) = host();
        let play = tree.add_child(
            root,
            base::Button::new("Play")
                .focus_name("play")
                .focus_neighbor_up("home")
                .focus_neighbor_down("next"),
        );
        let next = tree.add_child(root, base::Button::new("Next").focus_name("next"));
        // `home` is the last column child (bottom), yet `up` reaches it by name.
        let home = tree.add_child(root, base::Button::new("Home").focus_name("home"));
        igui_ui::layout(&mut tree, viewport(400.0, 400.0));

        assert!(igui_ui::set_focus(&mut tree, play));
        assert!(igui_ui::focus_up(&mut tree));
        assert_eq!(igui_ui::focused(&tree), Some(home));
        assert!(igui_ui::set_focus(&mut tree, play));
        assert!(igui_ui::focus_down(&mut tree));
        assert_eq!(igui_ui::focused(&tree), Some(next));
    }

    /// `group` + `group_hover` wire a parent's interaction state to a child's
    /// hover.
    #[test]
    fn group_hover_lights_the_declaring_control() {
        let (mut tree, root) = host();
        let card = tree.add_child(root, Flex::column().group_hover("card"));
        let button = tree.add_child(card, base::Button::new("Save").group("card"));
        igui_ui::layout(&mut tree, viewport(400.0, 400.0));

        let center = igui_ui::control(&tree, button).unwrap().rect.center();
        igui_ui::handle_input(&mut tree, &InputEvent::PointerMove { position: center });
        assert!(
            igui_ui::state_for(&tree, card).group_hovered,
            "hovering the button lights the group_declaring card"
        );
    }

    /// A focused themed button activates on Enter (it is a Flex widget, not
    /// click callback is the signal).
    #[test]
    fn enter_activates_a_focused_button() {
        let theme = crate::theme::default_theme(crate::theme::Mode::Dark);
        let (mut tree, root) = host();
        let clicks = Rc::new(Cell::new(0));
        let counter = clicks.clone();
        let button = tree.add_child(
            root,
            crate::Button::primary("Push", theme).on_click(move || counter.set(counter.get() + 1)),
        );
        igui_ui::layout(&mut tree, viewport(400.0, 200.0));

        assert!(igui_ui::set_focus(&mut tree, button));
        igui_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: igui_core::Key::Enter,
            },
        );
        assert_eq!(clicks.get(), 1);
    }

    /// A click focuses the button's inner label, but the arrows still use the
    /// button root's `FocusNav` (names live on the root).
    #[test]
    fn a_clicked_button_uses_its_own_focus_wiring() {
        let theme = crate::theme::default_theme(crate::theme::Mode::Dark);
        let (mut tree, root) = host();
        let col = tree.add_child(root, Column::new().gap(8.0));
        let home = tree.add_child(
            col,
            crate::Button::primary("Home", theme)
                .focus_name("home")
                .focus_neighbor_right("next"),
        );
        tree.add_child(
            col,
            crate::Button::secondary("Play", theme).focus_name("play"),
        );
        let next = tree.add_child(
            col,
            crate::Button::secondary("Next", theme).focus_name("next"),
        );
        igui_ui::layout(&mut tree, viewport(400.0, 400.0));
        tree.update();

        let center = igui_ui::control(&tree, home).unwrap().rect.center();
        click(&mut tree, center);
        assert!(
            igui_ui::focused(&tree).is_some(),
            "the click focused a node"
        );
        assert!(igui_ui::focus_right(&mut tree));
        assert_eq!(
            igui_ui::focused(&tree),
            Some(next),
            "the arrow followed the button's name, not the label's empty wiring"
        );
    }
}
