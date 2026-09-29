//! The demo host: one scene tree, the simulation and the view.
//!
//! The host only builds the tree, drives the clock and the frame pipeline, and
//! forwards input. The rules live in [`crate::sim`], the visuals in
//! [`crate::view`], the HUD in [`crate::hud`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use igui_core::{InputEvent, Key, PointerButton, Size, Vec2, ViewportSize};
use igui_game::FixedTimestep;
use igui_render::PaintContext;
use igui_scene::{SceneTree, Visual};
use igui_theme::{default_theme, Mode, Theme};
use igui_ui::{self, TextMeasurer};

use crate::hud::{build_hud, HUD_HEIGHT};
use crate::sim::{Building, Sim, GRID_H, GRID_W, TILE};
use crate::view::{building_color, sync_view, terrain_color, View};

/// The simulation runs two ticks per second.
const TICK_HZ: f32 = 2.0;

/// The camera position that centres the grid in the space below the HUD.
fn grid_center() -> Vec2 {
    Vec2::new(
        GRID_W as f32 * TILE * 0.5,
        GRID_H as f32 * TILE * 0.5 - HUD_HEIGHT * 0.5,
    )
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

    /// Builds the camera, the tile grid, the HUD and the node callbacks.
    pub fn init(&mut self) {
        let root = self.ui.root();

        // A camera centred on the grid, so the grid is centred rather than
        // hand-placed at a fixed origin.
        let camera = self.ui.add_camera_2d(root, "camera");
        self.ui.set_camera_current(camera, true);
        self.ui.set_position(camera, grid_center());

        // One `Node2D` per tile, in world coordinates.
        let world = self.ui.add_node2d(root, "world");
        let mut tile_nodes = Vec::with_capacity(GRID_W * GRID_H);
        {
            let sim = self.sim.borrow();
            for y in 0..GRID_H {
                for x in 0..GRID_W {
                    let node = self.ui.add_node2d(world, format!("tile_{x}_{y}"));
                    self.ui
                        .set_position(node, Vec2::new(x as f32 * TILE, y as f32 * TILE));
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

        // Behaviour #1: the simulation tick, at the fixed step.
        let sim_for_tick = self.sim.clone();
        let sim_node = self.ui.add_node(world, "simulation");
        self.ui
            .set_physics_process(sim_node, move |_tree, _id, _dt| {
                sim_for_tick.borrow_mut().tick();
            });

        // Behaviour #2: the view mirrors the sim, per frame.
        let sim_for_view = self.sim.clone();
        let view_for_sync = self.view.clone();
        let mode_for_view = self.build_mode.clone();
        self.ui.set_process(world, move |tree, _id, _dt| {
            let sim = sim_for_view.borrow();
            let mut slot = view_for_sync.borrow_mut();
            if let Some(view) = slot.as_mut() {
                sync_view(tree, &sim, view);
                let mode = match mode_for_view.get() {
                    Some(building) => format!("Build: {}", building.label()),
                    None => "Work: click a tile to assign a worker".to_string(),
                };
                igui_components::set_text(tree, view.hud.mode, mode);
            }
        });

        let hud = build_hud(&mut self.ui, self.theme, self.build_mode.clone());

        *self.view.borrow_mut() = Some(View {
            tile_nodes,
            worker_cache: vec![u32::MAX; GRID_W * GRID_H],
            hud,
        });

        // Draw the initial state once.
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

    /// Resolves layout for `viewport` and refreshes the camera.
    pub fn layout(&mut self, viewport: ViewportSize) {
        self.ui.set_viewport_size(viewport.logical_size());
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

    /// The tile under a logical viewport point, through the camera.
    pub fn cell_at(&self, position: Vec2) -> Option<(usize, usize)> {
        let world = self.ui.screen_to_world(position);
        if world.x < 0.0 || world.y < 0.0 {
            return None;
        }
        let x = (world.x / TILE) as usize;
        let y = (world.y / TILE) as usize;
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
        self.ui.add_child(
            node,
            igui_components::Text::caption(building.label(), self.theme),
        );
    }
}

impl Default for CivGame {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use igui_core::NodeId;
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
        assert_eq!(crate::CRATE, "civ_demo");
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

    /// The grid is centred: the middle tile lands near the middle of the
    /// viewport.
    #[test]
    fn the_camera_centres_the_grid() {
        let game = game();
        let center_tile = Vec2::new(GRID_W as f32 * TILE * 0.5, GRID_H as f32 * TILE * 0.5);
        let screen = game.tree().world_to_screen(center_tile);
        let viewport = game.tree().viewport().size();
        // Horizontally dead centre; vertically the centre of the region below the
        // HUD.
        assert!((screen.x - viewport.width * 0.5).abs() < 1.0);
        assert!((screen.y - (viewport.height + HUD_HEIGHT) * 0.5).abs() < 1.0);
    }

    /// The HUD rows stack instead of overlapping (the layout bug this replaced).
    #[test]
    fn the_hud_rows_stack() {
        let game = game();
        let slot = game.view.borrow();
        let view = slot.as_ref().expect("view built");
        let hud = &view.hud;
        let rect = |id: NodeId| igui_ui::control(game.tree(), id).unwrap().rect;
        let (food, wood, stone, pop, mode) = (
            rect(hud.food),
            rect(hud.wood),
            rect(hud.stone),
            rect(hud.population),
            rect(hud.mode),
        );
        // The counters sit side by side...
        assert!(food.center().x < wood.center().x);
        assert!(wood.center().x < stone.center().x);
        assert!(stone.center().x < pop.center().x);
        // ...and the mode line is below all of them.
        assert!(mode.center().y > food.center().y);
        assert!(mode.center().y > pop.center().y);
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
    fn a_tile_click_maps_through_the_camera() {
        let game = game();
        // The centre of the viewport is the camera's centre, a valid tile.
        let viewport = game.tree().viewport().size();
        let center = Vec2::new(viewport.width * 0.5, viewport.height * 0.5);
        assert!(game.cell_at(center).is_some());
        assert!(game.cell_at(Vec2::new(-5.0, -5.0)).is_none());
    }

    #[test]
    fn the_loop_gathers_and_keeps_the_settlement_alive() {
        let mut game = game();
        for _ in 0..20 {
            game.advance(0.5);
        }
        assert!(game.ticks() >= 20);
        let (food, _, _) = game.resources();
        assert!(food > 0);
    }
}
