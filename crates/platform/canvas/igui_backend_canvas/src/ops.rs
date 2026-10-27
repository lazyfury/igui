//! The CPU-side decision layer: one [`DrawCommand`] resolves to one
//! [`CanvasOp`], with every value (CSS colour, device-space clip rect, clamped
//! corner radii, resolved image source rect, font spec) computed here.
//!
//! This module is deliberately free of `web-sys`: it compiles and is tested on
//! any target, so the mapping is verified by plain `cargo test` while
//! [`super::canvas`] stays a thin executor that only calls the Canvas 2D API.

use igui_core::{Color, FontWeight, Rect, Size, Transform2D, Vec2};
use igui_render::{CornerRadii, DrawCommand, TextAlign, TextureId};

/// One Canvas 2D operation, already resolved from a [`DrawCommand`].
///
/// Styles are CSS strings (what the API takes), geometry stays in logical
/// pixels; only [`CanvasOp::ClipRect`] is pre-multiplied into device space,
/// because Canvas keeps a clip region in device space.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CanvasOp {
    Save,
    Restore,
    SetTransform(Transform2D),
    SetOpacity(f32),
    ClipRect {
        device: Rect,
    },
    FillRect {
        rect: Rect,
        fill: String,
    },
    StrokeRect {
        rect: Rect,
        stroke: String,
        width: f32,
    },
    Line {
        from: Vec2,
        to: Vec2,
        stroke: String,
        width: f32,
    },
    FillCircle {
        center: Vec2,
        radius: f32,
        fill: String,
    },
    StrokeCircle {
        center: Vec2,
        radius: f32,
        stroke: String,
        width: f32,
    },
    FillRoundedRect {
        rect: Rect,
        corners: CornerRadii,
        fill: String,
    },
    StrokeRoundedRect {
        rect: Rect,
        corners: CornerRadii,
        stroke: String,
        width: f32,
    },
    DrawImage {
        texture: TextureId,
        source: Rect,
        destination: Rect,
        alpha: f32,
    },
    DrawText {
        text: String,
        position: Vec2,
        font: String,
        align: &'static str,
        fill: String,
    },
    /// Nothing to draw: an image with no registered texture, or a command the
    /// Canvas backend cannot express yet.
    Skip,
}

/// Resolves `command` into the single Canvas op that draws it.
///
/// `scale_factor` multiplies clip geometry into device space; `image_size` is
/// the loaded image's size in logical pixels (needed when a `DrawImage` carries
/// no explicit source rect).
pub(crate) fn resolve(
    command: &DrawCommand,
    scale_factor: f32,
    image_size: Option<(f32, f32)>,
) -> CanvasOp {
    let fill = |paint: &igui_render::Paint| css_color(paint.color);
    let stroke = |paint: &igui_render::Paint| css_color(paint.color);

    match command {
        DrawCommand::Save => CanvasOp::Save,
        DrawCommand::Restore => CanvasOp::Restore,
        DrawCommand::SetTransform(transform) => CanvasOp::SetTransform(*transform),
        DrawCommand::SetOpacity(opacity) => CanvasOp::SetOpacity(*opacity),
        DrawCommand::ClipRect(rect) => CanvasOp::ClipRect {
            device: device_rect(*rect, scale_factor),
        },
        DrawCommand::FillRect { rect, paint } => CanvasOp::FillRect {
            rect: *rect,
            fill: fill(paint),
        },
        DrawCommand::StrokeRect { rect, paint, width } => CanvasOp::StrokeRect {
            rect: *rect,
            stroke: stroke(paint),
            width: *width,
        },
        DrawCommand::Line {
            from,
            to,
            paint,
            width,
        } => CanvasOp::Line {
            from: *from,
            to: *to,
            stroke: stroke(paint),
            width: *width,
        },
        DrawCommand::FillCircle {
            center,
            radius,
            paint,
        } => CanvasOp::FillCircle {
            center: *center,
            radius: *radius,
            fill: fill(paint),
        },
        DrawCommand::StrokeCircle {
            center,
            radius,
            paint,
            width,
        } => CanvasOp::StrokeCircle {
            center: *center,
            radius: *radius,
            stroke: stroke(paint),
            width: *width,
        },
        DrawCommand::FillRoundedRect {
            rect,
            corners,
            paint,
        } => CanvasOp::FillRoundedRect {
            rect: *rect,
            corners: clamp_corners(*rect, *corners),
            fill: fill(paint),
        },
        DrawCommand::StrokeRoundedRect {
            rect,
            corners,
            paint,
            width,
        } => CanvasOp::StrokeRoundedRect {
            rect: *rect,
            corners: clamp_corners(*rect, *corners),
            stroke: stroke(paint),
            width: *width,
        },
        DrawCommand::DrawImage {
            texture,
            destination,
            source,
            paint,
        } => {
            let Some((width, height)) = image_size else {
                return CanvasOp::Skip;
            };
            let source =
                source.unwrap_or_else(|| Rect::from_min_size(Vec2::ZERO, Size::new(width, height)));
            CanvasOp::DrawImage {
                texture: *texture,
                source,
                destination: *destination,
                alpha: paint.color.a,
            }
        }
        DrawCommand::DrawText {
            text,
            position,
            font_size,
            weight,
            align,
            paint,
        } => CanvasOp::DrawText {
            text: text.clone(),
            position: *position,
            font: font_spec(*font_size, *weight),
            align: align_keyword(*align),
            fill: fill(paint),
        },
    }
}

/// The Canvas font spec used for `DrawText` at `font_size` logical pixels and
/// `weight`.
///
/// Uses the CSS font shorthand (`"<weight> <size>px <family>"`), so a numeric
/// weight is passed straight through to the browser's font matcher.
///
/// A host-side `TextMeasurer` must measure with this exact spec so layout
/// baselines match what the backend draws.
pub fn font_spec(font_size: f32, weight: FontWeight) -> String {
    format!("{} {font_size}px sans-serif", weight.value())
}

/// A colour as a Canvas `fillStyle` / `strokeStyle` string.
pub(crate) fn css_color(color: Color) -> String {
    let [r, g, b, a] = color.to_rgba8();
    if a == 255 {
        format!("rgb({r},{g},{b})")
    } else {
        format!("rgba({r},{g},{b},{:.4})", a as f32 / 255.0)
    }
}

/// The `textAlign` keyword for an alignment.
pub(crate) fn align_keyword(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

/// Scales a logical clip rect into the canvas device space.
pub(crate) fn device_rect(rect: Rect, scale_factor: f32) -> Rect {
    Rect::from_min_size(
        Vec2::new(rect.left() * scale_factor, rect.top() * scale_factor),
        Size::new(
            rect.size.width * scale_factor,
            rect.size.height * scale_factor,
        ),
    )
}

/// Clamps per-corner radii the way the Canvas path builder needs them: no
/// corner may exceed half the smaller side of `rect`.
pub(crate) fn clamp_corners(rect: Rect, corners: CornerRadii) -> CornerRadii {
    let max = (rect.size.width * 0.5).min(rect.size.height * 0.5);
    corners.clamp(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use igui_render::Paint;

    fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color::from_rgba8(r, g, b, a)
    }

    fn paint(color: Color) -> Paint {
        Paint { color }
    }

    const RECT: Rect = Rect::from_min_size(Vec2::new(10.0, 20.0), Size::new(30.0, 40.0));

    #[test]
    fn opaque_colour_is_rgb_and_translucent_is_rgba() {
        let opaque = resolve(
            &DrawCommand::FillRect {
                rect: RECT,
                paint: paint(color(255, 128, 0, 255)),
            },
            1.0,
            None,
        );
        assert_eq!(
            opaque,
            CanvasOp::FillRect {
                rect: RECT,
                fill: "rgb(255,128,0)".into()
            }
        );

        let translucent = resolve(
            &DrawCommand::FillRect {
                rect: RECT,
                paint: paint(color(0, 0, 0, 128)),
            },
            1.0,
            None,
        );
        assert_eq!(
            translucent,
            CanvasOp::FillRect {
                rect: RECT,
                fill: "rgba(0,0,0,0.5020)".into()
            }
        );
    }

    #[test]
    fn clip_rect_is_scaled_into_device_space() {
        let op = resolve(&DrawCommand::ClipRect(RECT), 2.0, None);
        assert_eq!(
            op,
            CanvasOp::ClipRect {
                device: Rect::from_min_size(Vec2::new(20.0, 40.0), Size::new(60.0, 80.0))
            }
        );
    }

    #[test]
    fn strokes_carry_their_width_and_colour() {
        let op = resolve(
            &DrawCommand::StrokeRect {
                rect: RECT,
                paint: paint(color(1, 2, 3, 255)),
                width: 2.5,
            },
            1.0,
            None,
        );
        assert_eq!(
            op,
            CanvasOp::StrokeRect {
                rect: RECT,
                stroke: "rgb(1,2,3)".into(),
                width: 2.5
            }
        );
    }

    #[test]
    fn rounded_corners_are_clamped_to_half_the_smaller_side() {
        let op = resolve(
            &DrawCommand::FillRoundedRect {
                rect: Rect::from_min_size(Vec2::ZERO, Size::new(20.0, 10.0)),
                corners: CornerRadii::uniform(50.0),
                paint: paint(Color::BLACK),
            },
            1.0,
            None,
        );
        let CanvasOp::FillRoundedRect { corners, .. } = op else {
            panic!("expected a rounded rect, got {op:?}");
        };
        assert_eq!(corners, CornerRadii::uniform(5.0));
    }

    #[test]
    fn a_draw_image_without_a_source_uses_the_loaded_size() {
        let command = DrawCommand::DrawImage {
            texture: TextureId::new(7),
            destination: RECT,
            source: None,
            paint: paint(Color::WHITE),
        };

        let op = resolve(&command, 1.0, Some((64.0, 32.0)));
        assert_eq!(
            op,
            CanvasOp::DrawImage {
                texture: TextureId::new(7),
                source: Rect::from_min_size(Vec2::ZERO, Size::new(64.0, 32.0)),
                destination: RECT,
                alpha: 1.0
            }
        );
    }

    #[test]
    fn an_explicit_source_rect_wins_and_alpha_comes_from_the_paint() {
        let source = Rect::from_min_size(Vec2::new(4.0, 8.0), Size::new(16.0, 16.0));
        let op = resolve(
            &DrawCommand::DrawImage {
                texture: TextureId::new(1),
                destination: RECT,
                source: Some(source),
                paint: paint(color(255, 255, 255, 64)),
            },
            1.0,
            Some((64.0, 64.0)),
        );
        let CanvasOp::DrawImage {
            source: resolved,
            alpha,
            ..
        } = op
        else {
            panic!("expected an image draw, got {op:?}");
        };
        assert_eq!(resolved, source);
        assert!((alpha - 64.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn drawing_an_unregistered_texture_is_skipped() {
        let op = resolve(
            &DrawCommand::DrawImage {
                texture: TextureId::new(3),
                destination: RECT,
                source: None,
                paint: paint(Color::WHITE),
            },
            1.0,
            None,
        );
        assert_eq!(op, CanvasOp::Skip);
    }

    #[test]
    fn text_resolves_to_a_font_spec_and_an_align_keyword() {
        let op = resolve(
            &DrawCommand::DrawText {
                text: "hi".into(),
                position: Vec2::new(4.0, 12.0),
                font_size: 14.0,
                weight: FontWeight::BOLD,
                align: TextAlign::Center,
                paint: paint(Color::BLACK),
            },
            1.0,
            None,
        );
        assert_eq!(
            op,
            CanvasOp::DrawText {
                text: "hi".into(),
                position: Vec2::new(4.0, 12.0),
                font: font_spec(14.0, FontWeight::BOLD),
                align: "center",
                fill: "rgb(0,0,0)".into()
            }
        );
    }

    #[test]
    fn state_commands_pass_through_unchanged() {
        assert_eq!(resolve(&DrawCommand::Save, 1.0, None), CanvasOp::Save);
        assert_eq!(resolve(&DrawCommand::Restore, 1.0, None), CanvasOp::Restore);
        assert_eq!(
            resolve(&DrawCommand::SetOpacity(0.5), 1.0, None),
            CanvasOp::SetOpacity(0.5)
        );
        let transform = Transform2D::IDENTITY;
        assert_eq!(
            resolve(&DrawCommand::SetTransform(transform), 3.0, None),
            CanvasOp::SetTransform(transform)
        );
    }
}
