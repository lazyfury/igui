//! `DemoApp` behaviour: navigation, the theme rebuild and the headless pipeline.

use std::cell::RefCell;
use std::rc::Rc;

use crate::*;
use igui_backend_recording::RecordingBackend;
use igui_core::{Color, Key, NodeId, PointerButton};
use igui_render::{DrawCommand, PaintContext, RenderBackend};

fn laid_out() -> DemoApp {
    let viewport = ViewportSize::new(Size::new(1200.0, 760.0));
    let mut app = DemoApp::new();
    app.update(viewport, 0.016);
    app.layout(viewport);
    app
}

fn click(app: &mut DemoApp, position: Vec2) {
    app.event(&InputEvent::PointerDown {
        position,
        button: PointerButton::Left,
    });
    app.event(&InputEvent::PointerUp {
        position,
        button: PointerButton::Left,
    });
}

fn group_index(name: &str) -> usize {
    catalog::GROUPS
        .iter()
        .position(|group| group.name == name)
        .unwrap_or_else(|| panic!("group {name}"))
}

/// The gallery installs a host clipboard for its text fields.
#[test]
fn the_demo_installs_a_host_clipboard() {
    let mut app = laid_out();
    let clipboard = Rc::new(RefCell::new(igui_ui::MemoryClipboard::default()));
    app.set_clipboard(clipboard.clone());
    assert!(igui_ui::clipboard(app.tree()).is_some());
}

/// The gallery's live text fields are interactive end to end: a click focuses
/// one and a typed character reaches its committed text (and the `DrawList`).
#[test]
fn a_gallery_text_field_accepts_typed_input() {
    let mut app = laid_out();
    let controls = catalog::GROUPS
        .iter()
        .position(|group| group.name == "Controls")
        .expect("Controls group");
    app.show_group(controls);
    app.update(app.viewport(), 0.016);
    app.layout(app.viewport());

    // The first field with a text callback is the unmasked `TextInput` card.
    let field = app
        .tree()
        .iter()
        .find(|id| {
            app.tree()
                .data::<igui_ui::Control>(*id)
                .is_some_and(|control| control.text_callback.is_some())
        })
        .expect("a text field is mounted");
    let center = igui_ui::control(app.tree(), field)
        .expect("field laid out")
        .rect
        .center();
    click(&mut app, center);
    // Mimic the host: characters arrive as committed text, Space/Tab as a
    // named key.
    for (key, text) in [
        (Key::Character('a'), Some("a")),
        (Key::Space, None),
        (Key::Character('b'), Some("b")),
    ] {
        app.event(&InputEvent::KeyDown { key });
        if let Some(text) = text {
            app.event(&InputEvent::TextInput { text: text.into() });
        }
    }

    let mut ctx = PaintContext::new();
    app.paint(&mut ctx);
    let list = ctx.into_draw_list();
    assert!(
        list.iter().any(|command| matches!(
            command,
            DrawCommand::DrawText { text, .. } if text == "a b"
        )),
        "the typed text (with its space) is painted"
    );
}

#[test]
fn every_group_has_at_least_one_item() {
    assert_eq!(catalog::ITEMS.len(), catalog::GROUPS.len());
    for (group, items) in catalog::ITEMS.iter().enumerate() {
        assert!(!items.is_empty(), "group {group} has no items");
    }
}

#[test]
fn the_primary_button_toggles_the_theme() {
    let mut app = laid_out();
    let center = app.button_center().expect("primary button laid out");
    click(&mut app, center);
    assert_eq!(app.clicks(), 1);

    let viewport = app.viewport();
    app.update(viewport, 0.016);
    app.layout(viewport);
    assert_eq!(app.theme().mode(), Mode::Light);
    assert_eq!(app.clicks(), 1, "the click count survives the rebuild");
}

#[test]
fn show_group_switches_the_preview() {
    let mut app = laid_out();
    for group in 0..catalog::GROUPS.len() {
        app.show_group(group);
        assert_eq!(app.group(), group);
        assert_eq!(app.router().index(), group);
        app.layout(app.viewport());
        assert!(app.control_count() > 0);
    }
}

#[test]
fn titlebar_inset_pads_the_sidebar() {
    let mut app = laid_out();
    app.set_titlebar_inset(28.0);
    assert_eq!(app.titlebar_inset(), 28.0);
}

#[test]
fn a_short_window_scrolls_the_preview() {
    let viewport = ViewportSize::new(Size::new(1200.0, 300.0));
    let mut app = DemoApp::new();
    app.update(viewport, 0.016);
    app.layout(viewport);
    assert_eq!(app.preview_scroll(), 0.0);

    app.event(&InputEvent::Wheel {
        position: Vec2::new(700.0, 200.0),
        delta: Vec2::new(0.0, 200.0),
    });
    app.update(viewport, 0.016);
    app.layout(viewport);
    assert!(app.preview_scroll() > 0.0, "the preview page should scroll");
}

#[test]
fn the_theme_page_paints_palette_colours() {
    let mut app = laid_out();
    app.show_group(8);
    app.layout(app.viewport());

    let mut ctx = PaintContext::new();
    app.paint(&mut ctx);
    let list = ctx.into_draw_list();
    let filled = |color: Color| {
        list.commands().iter().any(|command| match command {
            DrawCommand::FillRect { paint, .. } | DrawCommand::FillRoundedRect { paint, .. } => {
                paint.color == color
            }
            _ => false,
        })
    };

    assert!(
        filled(app.theme().palette().accent),
        "the palette preview should paint the accent colour"
    );
    assert!(
        !filled(Color::new(0.13, 0.15, 0.20, 1.0)),
        "no swatch should fall back to the default Panel grey"
    );
}

#[test]
fn the_icons_page_paints_glyph_strokes() {
    let mut app = laid_out();
    app.show_group(4);
    app.layout(app.viewport());

    let mut ctx = PaintContext::new();
    app.paint(&mut ctx);
    let list = ctx.into_draw_list();
    assert!(
        list.commands()
            .iter()
            .any(|command| matches!(command, DrawCommand::Line { .. })),
        "the Icons page should stroke glyph lines"
    );
    assert!(
        list.commands()
            .iter()
            .any(|command| matches!(command, DrawCommand::FillCircle { .. })),
        "the Icons page should fill glyph dots"
    );
}

#[test]
fn the_animation_page_drives_an_external_value() {
    let viewport = ViewportSize::new(Size::new(1200.0, 760.0));
    let mut app = DemoApp::new();
    app.show_group(catalog::animation_group());
    app.update(viewport, 0.1);
    app.layout(viewport);

    assert!(app.needs_frame(), "a running tween needs frames");
    let before = app.state.animation.get();
    app.update(viewport, 0.3);
    let after = app.state.animation.get();
    assert!(
        after > before,
        "the tween advances the shared value: {before} -> {after}"
    );

    app.paint(&mut PaintContext::new());
    assert!(app.needs_frame(), "still animating after a paint");
}

#[test]
fn leaving_the_animation_page_stops_the_tween() {
    let viewport = ViewportSize::new(Size::new(1200.0, 760.0));
    let mut app = DemoApp::new();
    app.show_group(catalog::animation_group());
    app.update(viewport, 0.2);
    assert!(app.anim.is_animating());

    app.show_group(0);
    app.update(viewport, 0.1);
    assert!(!app.anim.is_animating());
    assert_eq!(app.state.animation.get(), 0.0);
}

#[test]
fn full_pipeline_records_a_draw_list_headlessly() {
    let app = laid_out();
    let viewport = app.viewport();

    let mut ctx = PaintContext::new();
    app.paint(&mut ctx);
    let list = ctx.into_draw_list();

    let mut backend = RecordingBackend::new();
    backend.begin_frame(viewport).unwrap();
    backend.submit(&list).unwrap();
    backend.end_frame().unwrap();

    assert_eq!(backend.frame_count(), 1);
    let commands = backend.last_frame().expect("frame").commands();
    assert!(commands
        .iter()
        .any(|c| matches!(c, DrawCommand::FillRect { .. })));
    assert!(commands
        .iter()
        .any(|c| matches!(c, DrawCommand::DrawText { .. })));
    assert!(commands
        .iter()
        .any(|c| matches!(c, DrawCommand::FillRoundedRect { .. })));
}

/// The gallery's Focus navigation card is wired end to end: a click focuses
/// `Home`, and the right arrow follows the named neighbor to `Next` (skipping
/// the `Play` in between).
#[test]
fn the_focus_navigation_card_follows_named_neighbors() {
    let viewport = ViewportSize::new(Size::new(1200.0, 1400.0));
    let mut app = DemoApp::new();
    app.update(viewport, 0.016);
    app.show_group(group_index("Controls"));
    app.update(viewport, 0.016);
    app.layout(viewport);

    let home = app
        .tree()
        .iter()
        .find(|id| {
            igui_ui::focus_nav(app.tree(), *id)
                .is_some_and(|nav| nav.name.as_deref() == Some("home"))
        })
        .expect("the Home button");
    let next = app
        .tree()
        .iter()
        .find(|id| {
            igui_ui::focus_nav(app.tree(), *id)
                .is_some_and(|nav| nav.name.as_deref() == Some("next"))
        })
        .expect("the Next button");
    let center = igui_ui::control(app.tree(), home)
        .expect("Home laid out")
        .rect
        .center();

    click(&mut app, center);
    app.event(&InputEvent::KeyDown {
        key: Key::ArrowRight,
    });
    assert_eq!(
        igui_ui::focused(app.tree()),
        Some(next),
        "right from Home reaches Next, skipping Play"
    );
}

/// The gallery's Group hover card reacts while its own button is hovered.
#[test]
fn the_group_hover_card_reacts_to_its_button() {
    let viewport = ViewportSize::new(Size::new(1200.0, 1400.0));
    let mut app = DemoApp::new();
    app.update(viewport, 0.016);
    app.show_group(group_index("Controls"));
    app.update(viewport, 0.016);
    app.layout(viewport);

    let surface = app
        .tree()
        .iter()
        .find(|id| {
            app.tree()
                .data::<igui_ui::Control>(*id)
                .is_some_and(|control| control.group_hover.as_deref() == Some("gallery-card"))
        })
        .expect("the group_hover surface");
    let button = app
        .tree()
        .iter()
        .find(|id| {
            app.tree()
                .data::<igui_ui::Control>(*id)
                .is_some_and(|control| control.group.as_deref() == Some("gallery-card"))
        })
        .expect("the group button");
    let center = igui_ui::control(app.tree(), button)
        .expect("button laid out")
        .rect
        .center();

    app.event(&InputEvent::PointerMove { position: center });
    assert!(
        igui_ui::state_for(app.tree(), surface).group_hovered,
        "hovering the button lights the surface"
    );
}

/// Whether the activation dot (the `success` marker) is painted inside `button`.
fn activation_dot_inside(app: &DemoApp, button: NodeId) -> bool {
    let rect = igui_ui::control(app.tree(), button)
        .expect("button laid out")
        .rect;
    let mut ctx = PaintContext::new();
    app.paint(&mut ctx);
    let list = ctx.into_draw_list();
    let success = app.theme().palette().success;
    list.commands().iter().any(|command| match command {
        DrawCommand::FillCircle { center, paint, .. } => {
            paint.color == success && rect.contains(*center)
        }
        _ => false,
    })
}

/// The gallery's Focus navigation buttons respond to Enter: activating with the
/// keyboard moves the marker to the focused button.
#[test]
fn enter_activates_the_focused_menu_button() {
    let viewport = ViewportSize::new(Size::new(1200.0, 1400.0));
    let mut app = DemoApp::new();
    app.update(viewport, 0.016);
    app.show_group(group_index("Controls"));
    app.update(viewport, 0.016);
    app.layout(viewport);

    let named = |app: &DemoApp, name: &str| {
        app.tree().iter().find(|id| {
            igui_ui::focus_nav(app.tree(), *id).is_some_and(|nav| nav.name.as_deref() == Some(name))
        })
    };
    let home = named(&app, "home").expect("the Home button");
    let next = named(&app, "next").expect("the Next button");

    let center = igui_ui::control(app.tree(), home)
        .expect("laid out")
        .rect
        .center();
    click(&mut app, center);
    assert!(activation_dot_inside(&app, home), "the click marks Home");

    app.event(&InputEvent::KeyDown {
        key: Key::ArrowRight,
    });
    assert_eq!(igui_ui::focused(app.tree()), Some(next));
    app.event(&InputEvent::KeyDown { key: Key::Enter });
    assert!(
        activation_dot_inside(&app, next),
        "Enter activates the focused Next button"
    );
    assert!(
        !activation_dot_inside(&app, home),
        "and the marker moves off Home"
    );
}
