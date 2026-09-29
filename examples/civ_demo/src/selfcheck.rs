//! Headless verification for `civ_demo`: runs the real loop and asserts the
//! outcome from the simulation state and the emitted `DrawList`. No window and
//! no screenshot (see `AGENTS.md`).

use std::rc::Rc;

use igui_core::{Size, ViewportSize};
use igui_render::{DrawCommand, PaintContext};
use igui_ui::FixedWidthTextMeasurer;

use crate::{Building, CivGame, GRID_H, GRID_W};

/// Runs the self-check, returning a description of the first failure.
pub fn run_selfcheck() -> Result<(), String> {
    let mut game = CivGame::new();
    game.set_text_measurer(Rc::new(FixedWidthTextMeasurer::default()));
    game.init();

    let viewport = ViewportSize::new(Size::new(640.0, 480.0));
    game.layout(viewport);

    let (food_before, _, _) = game.resources();

    // Build a house on a buildable tile, then run the loop.
    game.select_build(Some(Building::House));
    game.act_on_cell(0, 0);
    game.select_build(None);
    let (_, wood_after_build, _) = game.resources();

    for _ in 0..40 {
        game.advance(0.5);
        let mut ctx = PaintContext::new();
        game.paint(&mut ctx);
        let _ = ctx.into_draw_list();
    }

    if game.ticks() < 40 {
        return Err("the simulation did not tick".into());
    }
    let (food, wood, _) = game.resources();
    if food < food_before {
        return Err("food did not accumulate".into());
    }
    if wood_after_build >= 25 {
        return Err("building the house did not spend wood".into());
    }
    if game.population() == 0 {
        return Err("the settlement died out".into());
    }

    // Draw a frame: the whole grid and the HUD must reach the backend.
    let mut ctx = PaintContext::new();
    game.paint(&mut ctx);
    let list = ctx.into_draw_list();
    let tiles = list
        .commands()
        .iter()
        .filter(|command| matches!(command, DrawCommand::FillRect { .. }))
        .count();
    if tiles < GRID_W * GRID_H {
        return Err(format!("the grid was not drawn ({tiles} tiles)"));
    }
    if !list
        .commands()
        .iter()
        .any(|command| matches!(command, DrawCommand::DrawText { .. }))
    {
        return Err("the HUD was not drawn".into());
    }

    println!(
        "civ_demo selfcheck ok: ticks={}, food={}, wood={}, pop={}",
        game.ticks(),
        food,
        wood,
        game.population()
    );
    Ok(())
}
