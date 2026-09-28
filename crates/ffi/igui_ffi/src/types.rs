//! The `#[repr(C)]` ABI types and the opaque draw list.
//!
//! Everything here is made of `f32`, a C enum or an opaque pointer, so the
//! layout is stable and mirrored by `include/igui.h`. If you change a field,
//! change the header and bump [`ABI_VERSION`].

use igui_render::DrawList;

/// Version of this ABI. Bump on any layout or signature change; a host should
/// refuse to run when `igui_abi_version()` disagrees with its header.
pub const ABI_VERSION: u32 = 1;

/// The C-visible command discriminator.
///
/// Discriminants are part of the ABI: `include/igui.h` mirrors them and the
/// C++ backend switches on them.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IguiCommandTag {
    #[default]
    Save = 0,
    Restore = 1,
    SetTransform = 2,
    SetOpacity = 3,
    ClipRect = 4,
    FillRect = 5,
    StrokeRect = 6,
    Line = 7,
    FillCircle = 8,
    StrokeCircle = 9,
    FillRoundedRect = 10,
    StrokeRoundedRect = 11,
    /// A command ABI v1 does not model (`DrawImage` / `DrawText`). The record is
    /// otherwise zeroed; a backend skips it.
    Unsupported = 12,
}

/// A 2D point/vector in logical pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiVec2 {
    pub x: f32,
    pub y: f32,
}

/// An axis-aligned rectangle in logical pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// An RGBA color, components in `0.0..=1.0`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// A 2D affine transform: two basis axes plus an origin (six floats).
///
/// A point maps to `x_axis * p.x + y_axis * p.y + origin`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiTransform {
    pub x_axis: IguiVec2,
    pub y_axis: IguiVec2,
    pub origin: IguiVec2,
}

/// Per-corner radii, clockwise from the top-left, in logical pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiCornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

/// A solid fill/stroke style.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiPaint {
    pub color: IguiColor,
}

/// One command, flattened to a fixed layout.
///
/// Only the fields named by [`tag`](Self::tag) are meaningful; every other
/// field is zeroed. See `include/igui.h` for the field-to-command map.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IguiCommand {
    pub tag: IguiCommandTag,
    pub transform: IguiTransform,
    pub rect: IguiRect,
    pub corners: IguiCornerRadii,
    pub from: IguiVec2,
    pub to: IguiVec2,
    pub center: IguiVec2,
    pub paint: IguiPaint,
    pub radius: f32,
    pub width: f32,
    pub opacity: f32,
}

/// An opaque, Rust-owned [`DrawList`].
pub struct IguiDrawList {
    pub(crate) list: DrawList,
}

impl IguiDrawList {
    pub(crate) fn new() -> Self {
        Self {
            list: DrawList::new(),
        }
    }

    /// The underlying list, for a Rust companion crate that renders it (e.g.
    /// `wgpu_ffi`). Not part of the C ABI.
    pub fn draw_list(&self) -> &DrawList {
        &self.list
    }
}

impl Default for IguiDrawList {
    fn default() -> Self {
        Self::new()
    }
}

/// Wraps a [`DrawList`] built by another Rust crate for a foreign host.
///
/// This is a Rust helper, not part of the C ABI. A companion FFI crate (e.g.
/// `demoapp_ffi`) builds a list from a Rust app and hands it to the same
/// [`igui_draw_list_command`](crate::igui_draw_list_command) read-back; the
/// host releases it with [`igui_draw_list_free`](crate::igui_draw_list_free).
pub fn wrap_draw_list(list: DrawList) -> *mut IguiDrawList {
    Box::into_raw(Box::new(IguiDrawList { list }))
}
