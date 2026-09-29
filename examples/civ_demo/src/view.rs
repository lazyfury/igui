//! The view: tile colours and the per-frame mirror of the simulation.
//!
//! Kept free of the host so the colour/tint rules and the label updates are one
//! concern, and the callback that drives them stays tiny.

use igui_core::{Color, NodeId, Size};
use igui_scene::{SceneTree, Visual};

use crate::sim::{Building, Sim, Terrain, TILE};

/// The HUD label ids, updated by [`sync_view`].
pub(crate) struct Hud {
    pub(crate) food: NodeId,
    pub(crate) wood: NodeId,
    pub(crate) stone: NodeId,
    pub(crate) population: NodeId,
    pub(crate) mode: NodeId,
}

/// The view's node handles, shared with the view-sync callback.
pub(crate) struct View {
    pub(crate) tile_nodes: Vec<NodeId>,
    /// Last actor count per tile, so a tile's visual is only touched when it
    /// changed.
    pub(crate) worker_cache: Vec<u32>,
    pub(crate) hud: Hud,
}

/// Base colour for a terrain.
pub(crate) fn terrain_color(terrain: Terrain) -> Color {
    match terrain {
        Terrain::Grass => Color::new(0.30, 0.50, 0.28, 1.0),
        Terrain::Forest => Color::new(0.16, 0.34, 0.20, 1.0),
        Terrain::Mountain => Color::new(0.44, 0.42, 0.40, 1.0),
        Terrain::Water => Color::new(0.16, 0.30, 0.50, 1.0),
    }
}

/// Colour for a building.
pub(crate) fn building_color(building: Building) -> Color {
    match building {
        Building::House => Color::new(0.82, 0.66, 0.34, 1.0),
        Building::Farm => Color::new(0.86, 0.82, 0.38, 1.0),
        Building::LumberCamp => Color::new(0.55, 0.36, 0.22, 1.0),
        Building::Quarry => Color::new(0.62, 0.62, 0.68, 1.0),
    }
}

/// The per-tile worker tint: brighter with more workers.
fn worker_tint(color: Color, workers: u32) -> Color {
    let boost = 1.0 + workers as f32 * 0.22;
    Color::new(
        (color.r * boost).min(1.0),
        (color.g * boost).min(1.0),
        (color.b * boost).min(1.0),
        color.a,
    )
}

/// A tile's colour: base terrain, brightened by its workers.
pub(crate) fn tile_color(sim: &Sim, index: usize) -> Color {
    let tile = sim.tiles[index];
    let base = terrain_color(tile.terrain);
    if tile.workers > 0 {
        worker_tint(base, tile.workers)
    } else {
        base
    }
}

/// Mirrors `sim` onto the tile colours and the HUD counters.
pub(crate) fn sync_view(tree: &mut SceneTree, sim: &Sim, view: &mut View) {
    for index in 0..sim.tiles.len() {
        let workers = sim.tiles[index].workers;
        if view.worker_cache[index] == workers {
            continue;
        }
        view.worker_cache[index] = workers;
        tree.set_visual(
            view.tile_nodes[index],
            Visual::Rect {
                size: Size::splat(TILE - 1.0),
                color: tile_color(sim, index),
            },
        );
    }

    let (food, wood, stone) = (sim.food as u32, sim.wood as u32, sim.stone as u32);
    igui_components::set_text(tree, view.hud.food, format!("Food {food}"));
    igui_components::set_text(tree, view.hud.wood, format!("Wood {wood}"));
    igui_components::set_text(tree, view.hud.stone, format!("Stone {stone}"));
    igui_components::set_text(
        tree,
        view.hud.population,
        format!("Pop {}/{}", sim.population, sim.pop_cap()),
    );
}
