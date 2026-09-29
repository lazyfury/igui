//! `civ_demo` — a small civilization simulation on a square grid, built on the
//! single-tree / logic-on-nodes model.
//!
//! - [`sim`] — the pure rules (a grid of tiles, resources, gathering,
//!   population, consumption), unit tested without a tree.
//! - [`view`] — tile colours and the per-frame mirror of the simulation.
//! - [`hud`] — the `CanvasLayer` HUD (counters, build buttons, mode line).
//! - [`game`] — [`CivGame`], which builds the tree, centres the camera, wires
//!   the callbacks and drives the frame pipeline.
//!
//! Controls: `1..4` pick a building (House / Farm / Camp / Quarry), `0` / `Esc`
//! clears it. A click builds in a build mode, otherwise it assigns or releases a
//! worker on the tile.

pub mod sim;

mod game;
mod hud;
mod selfcheck;
mod view;

pub use game::CivGame;
pub use selfcheck::run_selfcheck;
pub use sim::{Building, Resource, Sim, Terrain, GRID_H, GRID_W, TILE};

/// Crate name, kept for lightweight smoke checks.
pub const CRATE: &str = "civ_demo";
