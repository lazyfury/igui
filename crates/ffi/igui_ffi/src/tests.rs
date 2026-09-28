//! The ABI contract tests: null safety, the round-trip of every command the
//! C++ UI can build, and the version.

use igui_core::Vec2;
use igui_render::{DrawCommand, DrawList, Paint};

use crate::convert::command_record;
use crate::*;

fn color4() -> IguiColor {
    IguiColor {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
    }
}

/// The ABI is a contract: the header mirrors this number.
#[test]
fn abi_version_is_stable() {
    assert_eq!(igui_abi_version(), 1);
    assert_eq!(ABI_VERSION, igui_abi_version());
}

/// A null list is safe and empty, so a host cannot crash on a bad handle.
#[test]
fn a_null_list_is_empty_and_safe() {
    assert_eq!(unsafe { igui_draw_list_len(std::ptr::null()) }, 0);
    let command = unsafe { igui_draw_list_command(std::ptr::null(), 0) };
    assert_eq!(command.tag, IguiCommandTag::Unsupported);
    // These are no-ops, not crashes.
    unsafe {
        igui_draw_list_free(std::ptr::null_mut());
        igui_draw_list_save(std::ptr::null_mut());
        igui_draw_list_fill_rect(
            std::ptr::null_mut(),
            IguiRect::default(),
            IguiPaint::default(),
        );
    }
}

/// Every command the demo's C++ UI can build round-trips through the ABI
/// with its payload intact, in order.
#[test]
fn geometry_and_state_round_trip() {
    let list = igui_draw_list_new();
    assert!(!list.is_null());

    let fill = IguiPaint { color: color4() };
    unsafe {
        igui_draw_list_save(list);
        igui_draw_list_set_opacity(list, 0.5);
        igui_draw_list_set_transform(
            list,
            IguiTransform {
                x_axis: IguiVec2 { x: 2.0, y: 0.0 },
                y_axis: IguiVec2 { x: 0.0, y: 3.0 },
                origin: IguiVec2 { x: 10.0, y: 20.0 },
            },
        );
        igui_draw_list_clip_rect(
            list,
            IguiRect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
        );
        igui_draw_list_fill_rect(
            list,
            IguiRect {
                x: 5.0,
                y: 6.0,
                width: 7.0,
                height: 8.0,
            },
            fill,
        );
        igui_draw_list_stroke_rect(
            list,
            IguiRect {
                x: 9.0,
                y: 10.0,
                width: 11.0,
                height: 12.0,
            },
            fill,
            2.5,
        );
        igui_draw_list_line(
            list,
            IguiVec2 { x: 1.0, y: 1.0 },
            IguiVec2 { x: 2.0, y: 2.0 },
            fill,
            1.5,
        );
        igui_draw_list_fill_circle(list, IguiVec2 { x: 30.0, y: 40.0 }, 5.0, fill);
        igui_draw_list_stroke_circle(list, IguiVec2 { x: 31.0, y: 41.0 }, 6.0, fill, 1.0);
        igui_draw_list_fill_rounded_rect(
            list,
            IguiRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            IguiCornerRadii {
                top_left: 1.0,
                top_right: 2.0,
                bottom_right: 3.0,
                bottom_left: 4.0,
            },
            fill,
        );
        igui_draw_list_stroke_rounded_rect(
            list,
            IguiRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            IguiCornerRadii {
                top_left: 1.0,
                top_right: 2.0,
                bottom_right: 3.0,
                bottom_left: 4.0,
            },
            fill,
            3.0,
        );
        igui_draw_list_restore(list);
    }

    let tags: Vec<IguiCommandTag> = (0..unsafe { igui_draw_list_len(list) })
        .map(|i| unsafe { igui_draw_list_command(list, i) }.tag)
        .collect();
    assert_eq!(
        tags,
        vec![
            IguiCommandTag::Save,
            IguiCommandTag::SetOpacity,
            IguiCommandTag::SetTransform,
            IguiCommandTag::ClipRect,
            IguiCommandTag::FillRect,
            IguiCommandTag::StrokeRect,
            IguiCommandTag::Line,
            IguiCommandTag::FillCircle,
            IguiCommandTag::StrokeCircle,
            IguiCommandTag::FillRoundedRect,
            IguiCommandTag::StrokeRoundedRect,
            IguiCommandTag::Restore,
        ]
    );

    // Spot-check payloads: a value that silently truncates would be worse
    // than a missing command.
    let fill_rect = unsafe { igui_draw_list_command(list, 4) };
    assert_eq!(fill_rect.rect.x, 5.0);
    assert_eq!(fill_rect.rect.height, 8.0);
    assert_eq!(fill_rect.paint.color.g, 0.2);

    let transform = unsafe { igui_draw_list_command(list, 2) };
    assert_eq!(transform.transform.x_axis.x, 2.0);
    assert_eq!(transform.transform.origin.y, 20.0);

    let circle = unsafe { igui_draw_list_command(list, 8) };
    assert_eq!(circle.radius, 6.0);
    assert_eq!(circle.width, 1.0);

    let rounded = unsafe { igui_draw_list_command(list, 9) };
    assert_eq!(rounded.corners.bottom_left, 4.0);

    // Out of bounds is a defined, skippable command.
    let past_end = unsafe { igui_draw_list_command(list, 999) };
    assert_eq!(past_end.tag, IguiCommandTag::Unsupported);

    unsafe { igui_draw_list_clear(list) };
    assert_eq!(unsafe { igui_draw_list_len(list) }, 0);
    unsafe { igui_draw_list_free(list) };
}

/// `DrawText` / `DrawImage` have no ABI v1 record; they must read back as
/// `Unsupported` rather than be reinterpreted as a geometry command.
#[test]
fn unmodelled_commands_read_as_unsupported() {
    let mut list = DrawList::new();
    list.push(DrawCommand::DrawText {
        text: "hi".to_string(),
        position: Vec2::ZERO,
        font_size: 12.0,
        weight: igui_render::FontWeight::NORMAL,
        align: igui_render::TextAlign::Left,
        paint: Paint::default(),
    });
    let record = command_record(&list.commands()[0]);
    assert_eq!(record.tag, IguiCommandTag::Unsupported);
    assert_eq!(record.rect, IguiRect::default());
}
