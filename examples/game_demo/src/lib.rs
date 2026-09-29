//! `game_demo` — a small top-down collect game built on the igui game layer.
//!
//! Game-first, one [`SceneTree`], logic on nodes:
//!
//! - The root viewport is the game; the world (arena `Node2D`, a `Camera2D`,
//!   the player sprite, coins) lives directly under it, and the HUD under a
//!   `CanvasLayer` (screen-fixed, ignores the camera).
//! - **Behaviour is node callbacks.** `set_physics_process` moves the player and
//!   its camera at the fixed step; `set_process` starts/stops the walk sheet and
//!   keeps the score label in sync; an `Area::on_enter` scores and removes the
//!   coin through the tree.
//! - The player's name tag is a `Control` parented to the player `Node2D`, so it
//!   follows the actor and the camera (the H2 world-space `Control`).
//!
//! The host only drives the runners ([`Animator`], [`SpriteAnimations`],
//! [`Timers`], [`Areas`]) and the frame pipeline:
//! [`Game::layout`] -> [`Game::advance`] -> [`Game::paint`], with
//! [`Game::needs_frame`] for on-demand rendering.

mod assets;
mod selfcheck;

pub use selfcheck::run_selfcheck;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use igui_anim::{Animator, Easing, TweenSpec};
use igui_components::{Component, Flex, NodeRef, Text};
use igui_core::{Edges, InputEvent, Key, NodeId, Size, Vec2, ViewportSize};
use igui_game::{
    upload_texture, Area, Areas, CollisionShape, FixedTimestep, Sprite, SpriteAnimations,
    SpriteFrames, Timers,
};
use igui_render::{PaintContext, RenderBackend, TextureId};
use igui_scene::{SceneTree, Visual};
use igui_theme::{default_theme, space, Mode, SurfaceLevel, Theme};
use igui_ui::{self, TextMeasurer};

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "game_demo";

const PLAYER_TEXTURE: TextureId = TextureId::new(1);
const COIN_TEXTURE: TextureId = TextureId::new(2);

const ARENA: f32 = 520.0;
const PLAYER_SPEED: f32 = 150.0;
const PLAYER_RADIUS: f32 = 9.0;
const COIN_RADIUS: f32 = 9.0;
const COIN_LIMIT: u32 = 8;
const SPAWN_INTERVAL: f32 = 0.7;
const HUD_HEIGHT: f32 = 34.0;

#[derive(Default, Clone, Copy)]
struct Keys {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

/// The demo's whole state: one scene tree, its runners and its HUD.
pub struct Game {
    theme: &'static dyn Theme,
    /// The single tree: root viewport (game) + a `CanvasLayer` HUD.
    ui: SceneTree,
    anim: Animator,
    /// Shared with the player's `process` walk-animation callback.
    sprites: Rc<RefCell<SpriteAnimations>>,
    timers: Timers,
    areas: Areas,
    score: Rc<Cell<u32>>,
    keys: Rc<RefCell<Keys>>,
    clock: FixedTimestep,
    player: Option<NodeId>,
    camera: Option<NodeId>,
    score_label: Option<NodeId>,
    spawn_accum: f32,
    spawned: u32,
    viewport: ViewportSize,
    painted_generation: Cell<u64>,
}

impl Game {
    /// Builds the state (no backend needed yet). Call [`Game::init`] with a
    /// backend before the first frame.
    pub fn new() -> Self {
        Self::with_theme(default_theme(Mode::Dark))
    }

    /// Builds the state with an explicit theme.
    pub fn with_theme(theme: &'static dyn Theme) -> Self {
        Self {
            theme,
            ui: SceneTree::new(),
            anim: Animator::new(),
            sprites: Rc::new(RefCell::new(SpriteAnimations::new())),
            timers: Timers::new(),
            areas: Areas::new(),
            score: Rc::new(Cell::new(0)),
            keys: Rc::new(RefCell::new(Keys::default())),
            clock: FixedTimestep::from_hz(120.0).max_steps(8),
            player: None,
            camera: None,
            score_label: None,
            spawn_accum: 0.0,
            spawned: 0,
            viewport: ViewportSize::new(Size::new(640.0, 480.0)),
            painted_generation: Cell::new(u64::MAX),
        }
    }

    /// Decodes/upload the art, builds the world and mounts the HUD.
    pub fn init<B: RenderBackend>(&mut self, backend: &mut B) -> Result<(), B::Error> {
        let player_image =
            igui_assets::decode_png(assets::PLAYER_SHEET).expect("player sheet decodes");
        let coin_image = igui_assets::decode_png(assets::COIN).expect("coin decodes");
        upload_texture(backend, PLAYER_TEXTURE, &player_image)?;
        upload_texture(backend, COIN_TEXTURE, &coin_image)?;
        let player_frames = SpriteFrames::from_grid(
            igui_core::Rect::from_min_size(Vec2::ZERO, Size::new(64.0, 16.0)),
            4,
            1,
            4,
        )
        .fps(9.0);

        let root = self.ui.root();
        let arena = self.ui.add_node2d(root, "arena");
        self.ui.set_visual(
            arena,
            Visual::Rect {
                size: Size::splat(ARENA),
                color: self.theme.surface(SurfaceLevel::Base),
            },
        );
        self.ui.set_position(arena, Vec2::splat(-ARENA * 0.5));

        let camera = self.ui.add_camera_2d(root, "camera");
        self.ui.set_camera_current(camera, true);
        self.camera = Some(camera);

        let player = self.ui.add_child(
            root,
            Sprite::new(PLAYER_TEXTURE, Size::splat(16.0)).named("player"),
        );
        self.player = Some(player);
        self.ui.set_scale(player, Vec2::ZERO);
        self.areas
            .add(Area::new(player, CollisionShape::circle(PLAYER_RADIUS)));
        self.anim.tween_scale(
            player,
            Vec2::splat(1.0),
            TweenSpec::new(0.45).easing(Easing::BackOut),
        );

        // World-space name tag: a `Control` parented to the player `Node2D`, so
        // it follows the actor and the camera.
        self.ui
            .add_child(player, Text::caption("player", self.theme));

        // Behaviour #1 — move the player (and the camera) at the fixed step.
        let keys = self.keys.clone();
        self.ui.set_physics_process(player, move |tree, id, step| {
            let keys = *keys.borrow();
            let mut direction = Vec2::ZERO;
            if keys.left {
                direction.x -= 1.0;
            }
            if keys.right {
                direction.x += 1.0;
            }
            if keys.up {
                direction.y -= 1.0;
            }
            if keys.down {
                direction.y += 1.0;
            }
            let direction = direction.normalize_or_zero();
            let limit = ARENA * 0.5 - 8.0;
            let current = tree.position(id).unwrap_or(Vec2::ZERO);
            let next = current + direction * (PLAYER_SPEED * step);
            let next = Vec2::new(next.x.clamp(-limit, limit), next.y.clamp(-limit, limit));
            tree.set_position(id, next);
            tree.set_position(camera, next);
        });

        // Behaviour #2 — start/stop the walk sheet as the movement keys change.
        let sprites = self.sprites.clone();
        let walk_keys = self.keys.clone();
        let mut walking = false;
        self.ui.set_process(player, move |tree, id, _dt| {
            let moving = {
                let keys = *walk_keys.borrow();
                keys.left || keys.right || keys.up || keys.down
            };
            if moving && !walking {
                sprites.borrow_mut().play(tree, id, player_frames.clone());
                walking = true;
            } else if !moving && walking {
                sprites.borrow_mut().stop(id);
                walking = false;
            }
        });

        for index in 0..4 {
            self.add_pickup_at(pickup_position(index as f32));
        }

        self.build_hud();
        Ok(())
    }

    /// Installs `measurer` for the HUD.
    pub fn set_text_measurer(&mut self, measurer: Rc<dyn TextMeasurer>) {
        igui_ui::set_text_measurer(&mut self.ui, measurer);
    }

    /// Resolves the HUD layout for `viewport`.
    pub fn layout(&mut self, viewport: ViewportSize) {
        self.viewport = viewport;
        igui_ui::layout(&mut self.ui, viewport);
        self.ui.update();
    }

    /// Advances the fixed-step world and its runners for `dt` seconds.
    ///
    /// The host owns the clock and drives the runners; all gameplay itself lives
    /// on the nodes' callbacks.
    pub fn advance(&mut self, dt: f32) {
        let tick = self.clock.advance(dt);
        for _ in 0..tick.steps {
            self.ui.physics_process(self.clock.step());
        }
        self.ui.process(dt);

        self.spawn_accum += dt;
        if self.spawned < COIN_LIMIT && self.spawn_accum >= SPAWN_INTERVAL {
            self.spawn_accum = 0.0;
            let position = pickup_position(self.spawned as f32 + 4.0);
            self.add_pickup_at(position);
        }

        self.anim.update(dt, &mut self.ui);
        self.sprites.borrow_mut().update(dt, &mut self.ui);
        self.timers.update(dt, &mut self.ui);
        self.areas.update(&mut self.ui);
        self.ui.update();
    }

    /// Routes an input event to the key state.
    pub fn event(&mut self, event: &InputEvent) {
        let (key, pressed) = match event {
            InputEvent::KeyDown { key } => (*key, true),
            InputEvent::KeyUp { key } => (*key, false),
            _ => return,
        };
        let mut keys = self.keys.borrow_mut();
        match key {
            Key::ArrowLeft | Key::Character('a') => keys.left = pressed,
            Key::ArrowRight | Key::Character('d') => keys.right = pressed,
            Key::ArrowUp | Key::Character('w') => keys.up = pressed,
            Key::ArrowDown | Key::Character('s') => keys.down = pressed,
            _ => {}
        }
    }

    /// Paints the whole tree (world + HUD) into `ctx`.
    pub fn paint(&self, ctx: &mut PaintContext) {
        igui_ui::paint(&self.ui, ctx);
        self.painted_generation
            .set(igui_ui::paint_generation(&self.ui));
    }

    /// Whether another frame is needed (a runner animating, the tree dirty, or
    /// an unpainted change).
    pub fn needs_frame(&self) -> bool {
        self.anim.is_animating()
            || self.sprites.borrow().is_animating()
            || self.timers.is_animating()
            || igui_ui::needs_layout(&self.ui)
            || self.ui.needs_update()
            || self.painted_generation.get() != igui_ui::paint_generation(&self.ui)
    }

    /// Current score.
    pub fn score(&self) -> u32 {
        self.score.get()
    }

    /// The player's world position.
    pub fn player_position(&self) -> Vec2 {
        self.player
            .and_then(|id| self.ui.position(id))
            .unwrap_or(Vec2::ZERO)
    }

    /// Adds a coin at `position` (also used by `--selfcheck`).
    pub fn add_pickup_at(&mut self, position: Vec2) -> NodeId {
        let root = self.ui.root();
        let id = self.ui.add_child(
            root,
            Sprite::new(COIN_TEXTURE, Size::splat(16.0))
                .named("coin")
                .position(position),
        );
        let score = self.score.clone();
        let player = self.player;
        self.areas
            .add(Area::new(id, CollisionShape::circle(COIN_RADIUS)).on_enter(
                move |tree, self_id, other| {
                    if Some(other) == player {
                        score.set(score.get() + 1);
                        tree.remove(self_id);
                    }
                },
            ));
        self.spawned += 1;
        id
    }

    /// The one scene tree (world + HUD).
    pub fn tree(&self) -> &SceneTree {
        &self.ui
    }

    fn build_hud(&mut self) {
        let theme = self.theme;
        let score_slot = NodeRef::new();
        let root = self.ui.root();
        let hud = self.ui.add_canvas_layer(root, "HUD");
        self.ui.set_canvas_layer(hud, 10);
        self.ui.add_child(
            hud,
            Flex::row()
                .min_size(0.0, HUD_HEIGHT)
                .padding(Edges::new(space::MD, space::XS, space::MD, space::XS))
                .child(Text::subheading("Score: 0", theme).ref_(&score_slot)),
        );
        self.score_label = score_slot.get();

        // Behaviour #3 — the label keeps itself in sync with the score.
        if let Some(label) = self.score_label {
            let score = self.score.clone();
            self.ui.set_process(label, move |tree, id, _dt| {
                igui_components::set_text(tree, id, format!("Score: {}", score.get()));
            });
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

/// A deterministic spawn position for the `index`-th coin.
fn pickup_position(index: f32) -> Vec2 {
    let half = ARENA * 0.5 - 40.0;
    Vec2::new(
        ((index * 137.0) % ARENA) - half,
        ((index * 271.0) % ARENA) - half,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use igui_backend_recording::RecordingBackend;
    use igui_render::DrawCommand;
    use igui_ui::FixedWidthTextMeasurer;

    #[test]
    fn crate_identity() {
        assert_eq!(CRATE, "game_demo");
    }

    #[test]
    fn a_frame_draws_the_world_and_the_hud() {
        let mut game = Game::new();
        game.set_text_measurer(Rc::new(FixedWidthTextMeasurer::default()));
        let mut backend = RecordingBackend::new();
        game.init(&mut backend).unwrap();

        let viewport = ViewportSize::new(Size::new(640.0, 480.0));
        game.layout(viewport);
        game.advance(1.0 / 60.0);

        let mut ctx = PaintContext::new();
        game.paint(&mut ctx);
        let list = ctx.into_draw_list();
        assert!(
            list.commands().iter().any(|command| matches!(
                command,
                DrawCommand::DrawImage { texture, .. } if *texture == PLAYER_TEXTURE
            )),
            "the world sprite is drawn into the one tree"
        );
        assert!(
            list.commands()
                .iter()
                .any(|command| matches!(command, DrawCommand::DrawText { .. })),
            "the HUD is drawn in the same pass"
        );
    }

    #[test]
    fn walking_into_a_coin_scores() {
        let mut game = Game::new();
        let mut backend = RecordingBackend::new();
        game.init(&mut backend).unwrap();
        game.layout(ViewportSize::new(Size::new(640.0, 480.0)));

        game.add_pickup_at(Vec2::new(48.0, 0.0));
        game.event(&InputEvent::KeyDown {
            key: Key::ArrowRight,
        });
        for _ in 0..120 {
            game.advance(1.0 / 60.0);
        }
        game.event(&InputEvent::KeyUp {
            key: Key::ArrowRight,
        });

        assert!(game.player_position().x > 0.0, "the player moved right");
        assert!(game.score() >= 1, "the player collected a coin");
    }
}
