//! Conversions between the core types and the C ABI records, in both
//! directions. Kept apart from the ABI functions so the marshalling can be
//! tested without touching raw pointers.

use igui_core::{Color, Rect, Size, Transform2D, Vec2};
use igui_render::{CornerRadii, DrawCommand, Paint};

use crate::types::*;

// -- core -> C -------------------------------------------------------------

pub(crate) fn vec2(v: Vec2) -> IguiVec2 {
    IguiVec2 { x: v.x, y: v.y }
}

pub(crate) fn rect(r: Rect) -> IguiRect {
    IguiRect {
        x: r.origin.x,
        y: r.origin.y,
        width: r.size.width,
        height: r.size.height,
    }
}

pub(crate) fn color(c: Color) -> IguiColor {
    IguiColor {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

pub(crate) fn paint(p: Paint) -> IguiPaint {
    IguiPaint {
        color: color(p.color),
    }
}

pub(crate) fn transform(t: Transform2D) -> IguiTransform {
    IguiTransform {
        x_axis: vec2(t.x_axis),
        y_axis: vec2(t.y_axis),
        origin: vec2(t.origin),
    }
}

pub(crate) fn corners(c: CornerRadii) -> IguiCornerRadii {
    IguiCornerRadii {
        top_left: c.top_left,
        top_right: c.top_right,
        bottom_right: c.bottom_right,
        bottom_left: c.bottom_left,
    }
}

// -- C -> core -------------------------------------------------------------

pub(crate) fn to_vec2(v: IguiVec2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

pub(crate) fn to_rect(r: IguiRect) -> Rect {
    Rect::from_min_size(Vec2::new(r.x, r.y), Size::new(r.width, r.height))
}

pub(crate) fn to_color(c: IguiColor) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

pub(crate) fn to_paint(p: IguiPaint) -> Paint {
    Paint::new(to_color(p.color))
}

pub(crate) fn to_transform(t: IguiTransform) -> Transform2D {
    Transform2D::new(to_vec2(t.x_axis), to_vec2(t.y_axis), to_vec2(t.origin))
}

pub(crate) fn to_corners(c: IguiCornerRadii) -> CornerRadii {
    CornerRadii::new(c.top_left, c.top_right, c.bottom_right, c.bottom_left)
}

/// Flattens one IR command into the C record.
pub(crate) fn command_record(command: &DrawCommand) -> IguiCommand {
    let mut out = IguiCommand::default();
    match command {
        DrawCommand::Save => out.tag = IguiCommandTag::Save,
        DrawCommand::Restore => out.tag = IguiCommandTag::Restore,
        DrawCommand::SetTransform(t) => {
            out.tag = IguiCommandTag::SetTransform;
            out.transform = transform(*t);
        }
        DrawCommand::SetOpacity(o) => {
            out.tag = IguiCommandTag::SetOpacity;
            out.opacity = *o;
        }
        DrawCommand::ClipRect(r) => {
            out.tag = IguiCommandTag::ClipRect;
            out.rect = rect(*r);
        }
        DrawCommand::FillRect { rect: r, paint: p } => {
            out.tag = IguiCommandTag::FillRect;
            out.rect = rect(*r);
            out.paint = paint(*p);
        }
        DrawCommand::StrokeRect {
            rect: r,
            paint: p,
            width,
        } => {
            out.tag = IguiCommandTag::StrokeRect;
            out.rect = rect(*r);
            out.paint = paint(*p);
            out.width = *width;
        }
        DrawCommand::Line {
            from,
            to,
            paint: p,
            width,
        } => {
            out.tag = IguiCommandTag::Line;
            out.from = vec2(*from);
            out.to = vec2(*to);
            out.paint = paint(*p);
            out.width = *width;
        }
        DrawCommand::FillCircle {
            center,
            radius,
            paint: p,
        } => {
            out.tag = IguiCommandTag::FillCircle;
            out.center = vec2(*center);
            out.radius = *radius;
            out.paint = paint(*p);
        }
        DrawCommand::StrokeCircle {
            center,
            radius,
            paint: p,
            width,
        } => {
            out.tag = IguiCommandTag::StrokeCircle;
            out.center = vec2(*center);
            out.radius = *radius;
            out.paint = paint(*p);
            out.width = *width;
        }
        DrawCommand::FillRoundedRect {
            rect: r,
            corners: c,
            paint: p,
        } => {
            out.tag = IguiCommandTag::FillRoundedRect;
            out.rect = rect(*r);
            out.corners = corners(*c);
            out.paint = paint(*p);
        }
        DrawCommand::StrokeRoundedRect {
            rect: r,
            corners: c,
            paint: p,
            width,
        } => {
            out.tag = IguiCommandTag::StrokeRoundedRect;
            out.rect = rect(*r);
            out.corners = corners(*c);
            out.paint = paint(*p);
            out.width = *width;
        }
        DrawCommand::DrawImage { .. } | DrawCommand::DrawText { .. } => {
            out.tag = IguiCommandTag::Unsupported;
        }
    }
    out
}
