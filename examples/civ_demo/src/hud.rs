//! Building the HUD: a `CanvasLayer` with resource counters, build buttons and a
//! mode line.
//!
//! Layout note: a UI root's own `Container` is ignored by the layout entry point
//! (a root is pinned to the viewport and its children are placed by anchors), so
//! the root here is a plain `Panel` and a single `Flex::column` child does the
//! stacking.

use std::cell::Cell;
use std::rc::Rc;

use igui_components::{Button, Component, Flex, NodeRef, Panel, Text};
use igui_core::{Color, Edges};
use igui_scene::SceneTree;
use igui_theme::{space, Theme};

use crate::sim::Building;
use crate::view::Hud;

/// Height the HUD occupies at the top of the viewport (the grid is centred in
/// the space below it).
pub(crate) const HUD_HEIGHT: f32 = 92.0;

/// Builds the HUD and returns its label ids.
pub(crate) fn build_hud(
    tree: &mut SceneTree,
    theme: &'static dyn Theme,
    build_mode: Rc<Cell<Option<Building>>>,
) -> Hud {
    let root = tree.root();
    let hud = tree.add_canvas_layer(root, "hud");
    tree.set_canvas_layer(hud, 10);

    let food = NodeRef::new();
    let wood = NodeRef::new();
    let stone = NodeRef::new();
    let population = NodeRef::new();
    let mode = NodeRef::new();

    let counters = Flex::row()
        .gap(space::MD)
        .padding(Edges::new(space::LG, space::MD, space::LG, space::XS))
        .child(Text::small("Food 0", theme).ref_(&food))
        .child(Text::small("Wood 0", theme).ref_(&wood))
        .child(Text::small("Stone 0", theme).ref_(&stone))
        .child(Text::small("Pop 0", theme).ref_(&population));

    let mut buttons =
        Flex::row()
            .gap(space::SM)
            .padding(Edges::new(space::LG, 0.0, space::LG, 0.0));
    for building in Building::ALL {
        let mode_handle = build_mode.clone();
        buttons = buttons.child(
            Button::secondary(building.label(), theme)
                .on_click(move |_tree, _id| mode_handle.set(Some(building))),
        );
    }
    let mode_handle = build_mode.clone();
    let buttons = buttons
        .child(Button::ghost("Work", theme).on_click(move |_tree, _id| mode_handle.set(None)));

    let mode_row = Flex::row()
        .padding(Edges::new(space::LG, space::XS, space::LG, 0.0))
        .child(Text::caption("Work: click a tile to assign a worker", theme).ref_(&mode));

    // A plain root (pinned to the viewport); the column child does the stacking.
    let panel = tree.add_child(hud, Panel::new().color(Color::TRANSPARENT).flat());
    tree.add_child(
        panel,
        Flex::column()
            .gap(space::SM)
            .child(counters)
            .child(buttons)
            .child(mode_row),
    );

    Hud {
        food: food.get().expect("food label mounted"),
        wood: wood.get().expect("wood label mounted"),
        stone: stone.get().expect("stone label mounted"),
        population: population.get().expect("population label mounted"),
        mode: mode.get().expect("mode label mounted"),
    }
}
