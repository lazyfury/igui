//! `civ_demo` — a small civilization simulation on a square grid, on the
//! single-tree / logic-on-nodes model.
//!
//! - The pure rules live in [`sim`] (a grid of tiles, resources, gathering,
//!   population, consumption) and are unit tested without a tree.
//! - The tick is a `set_physics_process` callback; the view (tile colours, HUD
//!   counters) is a `set_process` callback; building name tags are world-space
//!   `Control`s under their `Node2D`.
//! - The host only drives the clock and the frame pipeline
//!   ([`CivGame::layout`] -> [`CivGame::advance`] -> [`CivGame::paint`]).
//!
//! Controls: `1..4` pick a building (House / Farm / Camp / Quarry), `0` / `Esc`
//! clears it. A click builds in a build mode, otherwise it assigns or releases a
//! worker on the tile.

pub mod sim;

mod selfcheck;

pub use selfcheck::run_selfcheck;
pub use sim::{Building, Sim, Terrain, GRID_H, GRID_W, TILE};

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use igui_components::{Button, Component, Flex, NodeRef, Panel, Text};
use igui_core::{Color, Edges, InputEvent, Key, NodeId, PointerButton, Size, Vec2, ViewportSize};
use igui_game::FixedTimestep;
use igui_render::PaintContext;
use igui_scene::{SceneTree, Visual};
use igui_theme::{default_theme, space, Mode, Theme};
use igui_ui::{self, TextMeasurer};

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "civ_demo";

/// Top-left of the grid in logical viewport coordinates.
pub const GRID_ORIGIN: Vec2 = Vec2::new(24.0, 96.0);

/// The simulation runs two ticks per second.
const TICK_HZ: f32 = 2.0;

/// A tile's base colour by terrain.
fn terrain_color(terrain: Terrain) -> Color {
    match terrain {
        Terrain::Grass => Color::new(0.30, 0.50, 0.28, 1.0),
        Terrain::Forest => Color::new(0.16, 0.34, 0.20, 1.0),
        Terrain::Mountain => Color::new(0.44, 0.42, 0.40, 1.0),
        Terrain::Water => Color::new(0.16, 0.30, 0.50, 1.0),
    }
}

fn building_color(building: Building) -> Color {
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

/// HUD label ids, updated by the view-sync callback.
struct Hud {
    food: NodeId,
    wood: NodeId,
    stone: NodeId,
    population: NodeId,
    mode: NodeId,
}

/// The view's node handles, shared with the view-sync callback.
struct View {
    tile_nodes: Vec<NodeId>,
    worker_cache: Vec<u32>,
    hud: Hud,
}

/// The demo: one scene tree, the simulation and the view.
pub struct CivGame {
    theme: &'static dyn Theme,
    ui: SceneTree,
    sim: Rc<RefCell<Sim>>,
    view: Rc<RefCell<Option<View>>>,
    build_mode: Rc<Cell<Option<Building>>>,
    clock: FixedTimestep,
    painted_generation: Cell<u64>,
}

impl CivGame {
    /// Builds the state (no window needed). Call [`CivGame::init`] next.
    pub fn new() -> Self {
        Self::with_theme(default_theme(Mode::Dark))
    }

    /// Builds the state with an explicit theme.
    pub fn with_theme(theme: &'static dyn Theme) -> Self {
        Self {
            theme,
            ui: SceneTree::new(),
            sim: Rc::new(RefCell::new(Sim::new())),
            view: Rc::new(RefCell::new(None)),
            build_mode: Rc::new(Cell::new(None)),
            clock: FixedTimestep::from_hz(TICK_HZ),
            painted_generation: Cell::new(u64::MAX),
        }
    }

    /// Builds the tile grid, the HUD and the node callbacks.
    pub fn init(&mut self) {
        let root = self.ui.root();

        // -- world: one `Node2D` per tile ->--------------------------------
        let world = self.ui.add_node2d(root, "world");
        let mut tile_nodes = Vec::with_capacity(GRID_W * GRID_H);
        {
            let sim = self.sim.borrow();
            for y in 0..GRID_H {
                for x in 0..GRID_W {
                    let node = self.ui.add_node2d(world, format!("tile_{x}_{y}"));
                    self.ui.set_position(
                        node,
                        GRID_ORIGIN + Vec2::new(x as f32 * TILE, y as f32 * TILE),
                    );
                    self.ui.set_visual(
                        node,
                        Visual::Rect {
                            size: Size::splat(TILE - 1.0),
                            color: terrain_color(sim.tile(x, y).terrain),
                        },
                    );
                    tile_nodes.push(node);
                }
            }
        }

        // -- behaviour #1: the simulation tick (fixed step) ---------------
        let sim_for_tick = self.sim.clone();
        let sim_node = self.ui.add_node(world, "simulation");
        self.ui
            .set_physics_process(sim_node, move |_tree, _id, _dt| {
                sim_for_tick.borrow_mut().tick();
            });

        // -- behaviour #2: the view mirrors the sim (per frame) -----------
        let sim_for_view = self.sim.clone();
        let view_for_sync = self.view.clone();
        let mode_for_view = self.build_mode.clone();
        self.ui.set_process(world, move |tree, _id, _dt| {
            let sim = sim_for_view.borrow();
            let mut slot = view_for_sync.borrow_mut();
            if let Some(view) = slot.as_mut() {
                sync_view(tree, &sim, view);
                let label = match mode_for_view.get() {
                    Some(building) => format!("Build: {}", building.label()),
                    None => "Work: click a tile to assign a worker".to_string(),
                };
                igui_components::set_text(tree, view.hud.mode, label);
            }
        });

        // -- HUD ----------------------------------------------------------
        let hud = self.build_hud();

        *self.view.borrow_mut() = Some(View {
            tile_nodes,
            worker_cache: vec![u32::MAX; GRID_W * GRID_H],
            hud,
        });

        // Draw the initial state.
        {
            let sim = self.sim.borrow();
            let mut slot = self.view.borrow_mut();
            if let Some(view) = slot.as_mut() {
                sync_view(&mut self.ui, &sim, view);
            }
        }
    }

    /// Installs `measurer` for the HUD.
    pub fn set_text_measurer(&mut self, measurer: Rc<dyn TextMeasurer>) {
        igui_ui::set_text_measurer(&mut self.ui, measurer);
    }

    /// Resolves layout for `viewport`.
    pub fn layout(&mut self, viewport: ViewportSize) {
        igui_ui::layout(&mut self.ui, viewport);
        self.ui.update();
    }

    /// Advances the simulation and refreshes the view for `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        let tick = self.clock.advance(dt);
        let step = self.clock.step();
        for _ in 0..tick.steps {
            self.ui.physics_process(step);
        }
        self.ui.process(dt);
        self.ui.update();
    }

    /// Routes a window event: pointer clicks act on the grid, number keys pick
    /// a building.
    pub fn event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::PointerDown {
                position,
                button: PointerButton::Left,
            } => {
                if let Some((x, y)) = self.cell_at(*position) {
                    self.act_on_cell(x, y);
                }
            }
            InputEvent::KeyDown { key } => match key {
                Key::Character('1') => self.select_build(Some(Building::House)),
                Key::Character('2') => self.select_build(Some(Building::Farm)),
                Key::Character('3') => self.select_build(Some(Building::LumberCamp)),
                Key::Character('4') => self.select_build(Some(Building::Quarry)),
                Key::Character('0') | Key::Escape => self.select_build(None),
                _ => {}
            },
            _ => {}
        }
    }

    /// Selects a building to place, or `None` for worker mode.
    pub fn select_build(&mut self, building: Option<Building>) {
        self.build_mode.set(building);
    }

    /// Builds (in a build mode) or toggles a worker (otherwise) on a tile.
    pub fn act_on_cell(&mut self, x: usize, y: usize) {
        match self.build_mode.get() {
            Some(building) => {
                if self.sim.borrow_mut().build(x, y, building) {
                    self.add_building_view(x, y, building);
                }
            }
            None => {
                self.sim.borrow_mut().toggle_worker(x, y);
            }
        }
    }

    /// The tile under a logical viewport point, if any.
    pub fn cell_at(&self, position: Vec2) -> Option<(usize, usize)> {
        let local = position - GRID_ORIGIN;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let x = (local.x / TILE) as usize;
        let y = (local.y / TILE) as usize;
        Sim::in_bounds(x, y).then_some((x, y))
    }

    /// Paints the tree into `ctx`.
    pub fn paint(&self, ctx: &mut PaintContext) {
        igui_ui::paint(&self.ui, ctx);
        self.painted_generation
            .set(igui_ui::paint_generation(&self.ui));
    }

    /// A live simulation always wants another frame.
    pub fn needs_frame(&self) -> bool {
        true
    }

    /// Current population.
    pub fn population(&self) -> u32 {
        self.sim.borrow().population
    }

    /// Current stockpiles as whole numbers: `(food, wood, stone)`.
    pub fn resources(&self) -> (u32, u32, u32) {
        let sim = self.sim.borrow();
        (sim.food as u32, sim.wood as u32, sim.stone as u32)
    }

    /// Number of ticks the simulation has run.
    pub fn ticks(&self) -> u64 {
        self.sim.borrow().ticks
    }

    /// The one scene tree (world + HUD).
    pub fn tree(&self) -> &SceneTree {
        &self.ui
    }

    fn add_building_view(&mut self, x: usize, y: usize, building: Building) {
        let index = Sim::index(x, y);
        let tile = {
            let view = self.view.borrow();
            let Some(view) = view.as_ref() else {
                return;
            };
            view.tile_nodes[index]
        };
        let node = self.ui.add_node2d(tile, building.label());
        self.ui.set_position(node, Vec2::splat(4.0));
        self.ui.set_visual(
            node,
            Visual::Rect {
                size: Size::splat(TILE - 9.0),
                color: building_color(building),
            },
        );
        // A world-space name tag: a `Control` under the building `Node2D`.
        self.ui
            .add_child(node, Text::caption(building.label(), self.theme));
    }

    fn build_hud(&mut self) -> Hud {
        let theme = self.theme;
        let root = self.ui.root();
        let hud = self.ui.add_canvas_layer(root, "hud");
        self.ui.set_canvas_layer(hud, 10);

        let food = NodeRef::new();
        let wood = NodeRef::new();
        let stone = NodeRef::new();
        let population = NodeRef::new();
        let mode = NodeRef::new();

        let counters = Flex::row()
            .gap(space::MD)
            .padding(Edges::new(space::LG, space::MD, space::LG, space::SM))
            .child(Text::small("Food 0", theme).ref_(&food))
            .child(Text::small("Wood 0", theme).ref_(&wood))
            .child(Text::small("Stone 0", theme).ref_(&stone))
            .child(Text::small("Pop 0", theme).ref_(&population));

        let mut buttons =
            Flex::row()
                .gap(space::SM)
                .padding(Edges::new(space::LG, 0.0, space::LG, space::SM));
        for building in Building::ALL {
            let mode_handle = self.build_mode.clone();
            buttons = buttons.child(
                Button::secondary(building.label(), theme)
                    .on_click(move |_tree, _id| mode_handle.set(Some(building))),
            );
        }
        let mode_handle = self.build_mode.clone();
        let buttons = buttons
            .child(Button::ghost("Work", theme).on_click(move |_tree, _id| mode_handle.set(None)));

        // A root panel is pinned to the viewport; the flex rows inside it are
        // what actually arrange the HUD (a UI root's own container is ignored).
        let panel = self
            .ui
            .add_child(hud, Panel::new().color(Color::TRANSPARENT).flat());
        self.ui.add_child(panel, counters);
        self.ui.add_child(panel, buttons);
        self.ui.add_child(
            panel,
            Flex::row()
                .padding(Edges::new(space::LG, 0.0, space::LG, 0.0))
                .child(Text::caption("Build: click a tile", theme).ref_(&mode)),
        );

        Hud {
            food: food.get().expect("food label mounted"),
            wood: wood.get().expect("wood label mounted"),
            stone: stone.get().expect("stone label mounted"),
            population: population.get().expect("population label mounted"),
            mode: mode.get().expect("mode label mounted"),
        }
    }
}

impl Default for CivGame {
    fn default() -> Self {
        Self::new()
    }
}

/// Mirrors `sim` onto the tile colours and the HUD labels.
fn sync_view(tree: &mut SceneTree, sim: &Sim, view: &mut View) {
    for (index, tile) in sim.tiles.iter().enumerate() {
        if view.worker_cache[index] == tile.workers {
            continue;
        }
        view.worker_cache[index] = tile.workers;
        let base = terrain_color(tile.terrain);
        tree.set_visual(
            view.tile_nodes[index],
            Visual::Rect {
                size: Size::splat(TILE - 1.0),
                color: if tile.workers > 0 {
                    worker_tint(base, tile.workers)
                } else {
                    base
                },
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

#[cfg(test)]
mod tests {
    use super::*;
    use igui_render::DrawCommand;
    use igui_ui::FixedWidthTextMeasurer;

    fn game() -> CivGame {
        let mut game = CivGame::new();
        game.set_text_measurer(Rc::new(FixedWidthTextMeasurer::default()));
        game.init();
        game.layout(ViewportSize::new(Size::new(640.0, 480.0)));
        game
    }

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "civ_demo");
    }

    #[test]
    fn a_frame_draws_the_grid() {
        let game = game();
        let mut ctx = PaintContext::new();
        game.paint(&mut ctx);
        let list = ctx.into_draw_list();
        let tiles = list
            .commands()
            .iter()
            .filter(|command| matches!(command, DrawCommand::FillRect { .. }))
            .count();
        assert!(tiles >= GRID_W * GRID_H, "the whole grid is drawn");
    }

    #[test]
    fn a_build_click_places_a_building_and_spends_resources() {
        let mut game = game();
        let (_, wood_before, _) = game.resources();
        game.select_build(Some(Building::House));
        game.act_on_cell(0, 0);
        let (_, wood_after, _) = game.resources();
        assert_eq!(wood_before - wood_after, 12, "the house costs 12 wood");
    }

    #[test]
    fn the_loop_gathers_and_keeps_the_settlement_alive() {
        let mut game = game();
        for _ in 0..20 {
            game.advance(0.5);
        }
        assert!(game.ticks() >= 20);
        // Two food workers out-earn three citizens, so food grows.
        let (food, _, _) = game.resources();
        assert!(food > 0);
    }
}
