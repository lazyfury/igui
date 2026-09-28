//! The `extern "C"` surface: allocation, the state/geometry builders, and the
//! read-back of the command stream.
//!
//! The functions take raw pointers. A null `DrawList` is a no-op (or a zeroed
//! [`IguiCommand`] with [`IguiCommandTag::Unsupported`]), so a host cannot
//! segfault by forgetting a null check; every other pointer must come from
//! [`igui_draw_list_new`] and be released with [`igui_draw_list_free`].

use igui_render::DrawCommand;

use crate::convert::*;
use crate::types::*;

/// Returns [`ABI_VERSION`] so a host can check its header matches.
#[no_mangle]
pub extern "C" fn igui_abi_version() -> u32 {
    ABI_VERSION
}

/// Allocates an empty [`DrawList`](igui_render::DrawList). Release it with
/// [`igui_draw_list_free`].
#[no_mangle]
pub extern "C" fn igui_draw_list_new() -> *mut IguiDrawList {
    Box::into_raw(Box::new(IguiDrawList::new()))
}

/// Releases a list from [`igui_draw_list_new`]. A null pointer is a no-op.
///
/// # Safety
///
/// `list` must be null or a pointer from [`igui_draw_list_new`] that has not
/// been freed yet.
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_free(list: *mut IguiDrawList) {
    if list.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(list) });
}

/// Removes every command, keeping the allocation.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_clear(list: *mut IguiDrawList) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.clear();
    }
}

/// The number of commands; `0` for a null list.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_len(list: *const IguiDrawList) -> usize {
    unsafe { list.as_ref() }.map_or(0, |list| list.list.len())
}

/// Reads one command back as a flat record.
///
/// An out-of-bounds `index` (or a null list) yields
/// [`IguiCommandTag::Unsupported`] with every field zeroed.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_command(
    list: *const IguiDrawList,
    index: usize,
) -> IguiCommand {
    let Some(list) = (unsafe { list.as_ref() }) else {
        return IguiCommand {
            tag: IguiCommandTag::Unsupported,
            ..IguiCommand::default()
        };
    };
    match list.list.commands().get(index) {
        Some(command) => command_record(command),
        None => IguiCommand {
            tag: IguiCommandTag::Unsupported,
            ..IguiCommand::default()
        },
    }
}

// -- state commands --------------------------------------------------------

/// Pushes the transform/opacity/clip state.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_save(list: *mut IguiDrawList) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::Save);
    }
}

/// Pops the state pushed by a matching save.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_restore(list: *mut IguiDrawList) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::Restore);
    }
}

/// Replaces the current transform.
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_set_transform(
    list: *mut IguiDrawList,
    transform: IguiTransform,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list
            .push(DrawCommand::SetTransform(to_transform(transform)));
    }
}

/// Replaces the current opacity multiplier (`0.0..=1.0`).
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_set_opacity(list: *mut IguiDrawList, opacity: f32) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::SetOpacity(opacity));
    }
}

/// Sets the clip rectangle (viewport/logical space).
///
/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_clip_rect(list: *mut IguiDrawList, rect: IguiRect) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::ClipRect(to_rect(rect)));
    }
}

// -- geometry commands -----------------------------------------------------

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_fill_rect(
    list: *mut IguiDrawList,
    rect: IguiRect,
    paint: IguiPaint,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::FillRect {
            rect: to_rect(rect),
            paint: to_paint(paint),
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_stroke_rect(
    list: *mut IguiDrawList,
    rect: IguiRect,
    paint: IguiPaint,
    width: f32,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::StrokeRect {
            rect: to_rect(rect),
            paint: to_paint(paint),
            width,
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_line(
    list: *mut IguiDrawList,
    from: IguiVec2,
    to: IguiVec2,
    paint: IguiPaint,
    width: f32,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::Line {
            from: to_vec2(from),
            to: to_vec2(to),
            paint: to_paint(paint),
            width,
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_fill_circle(
    list: *mut IguiDrawList,
    center: IguiVec2,
    radius: f32,
    paint: IguiPaint,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::FillCircle {
            center: to_vec2(center),
            radius,
            paint: to_paint(paint),
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_stroke_circle(
    list: *mut IguiDrawList,
    center: IguiVec2,
    radius: f32,
    paint: IguiPaint,
    width: f32,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::StrokeCircle {
            center: to_vec2(center),
            radius,
            paint: to_paint(paint),
            width,
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_fill_rounded_rect(
    list: *mut IguiDrawList,
    rect: IguiRect,
    corners: IguiCornerRadii,
    paint: IguiPaint,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::FillRoundedRect {
            rect: to_rect(rect),
            corners: to_corners(corners),
            paint: to_paint(paint),
        });
    }
}

/// # Safety
///
/// `list` must be null or a valid pointer from [`igui_draw_list_new`].
#[no_mangle]
pub unsafe extern "C" fn igui_draw_list_stroke_rounded_rect(
    list: *mut IguiDrawList,
    rect: IguiRect,
    corners: IguiCornerRadii,
    paint: IguiPaint,
    width: f32,
) {
    if let Some(list) = unsafe { list.as_mut() } {
        list.list.push(DrawCommand::StrokeRoundedRect {
            rect: to_rect(rect),
            corners: to_corners(corners),
            paint: to_paint(paint),
            width,
        });
    }
}
