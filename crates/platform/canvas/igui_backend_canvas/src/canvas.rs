use std::collections::HashMap;
use std::fmt;

use web_sys::{CanvasRenderingContext2d, HtmlImageElement};

use igui_core::{Rect, Transform2D, Vec2, ViewportSize};
use igui_render::{CornerRadii, DrawCommand, DrawList, RenderBackend, TextureId};

use crate::ops::{self, CanvasOp};

/// Errors from the Canvas 2D backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasError {
    /// The context was not created from a canvas element.
    MissingCanvas,
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCanvas => f.write_str("2d context is not attached to a canvas"),
        }
    }
}

impl std::error::Error for CanvasError {}

/// A [`RenderBackend`] that maps a [`DrawList`] onto the HTML Canvas 2D API.
///
/// Every `DrawCommand` is first resolved by [`ops::resolve`] into a
/// [`CanvasOp`] (CSS colours, device-space clips, clamped radii, font specs);
/// this type only executes that op, so the mapping itself is covered by native
/// tests and this file stays a thin executor.
///
/// # Coordinate handling
///
/// Geometry is authored in logical pixels. The backend scales the backing store
/// to `logical * scale_factor` and multiplies every transform by `scale_factor`,
/// so DPR never leaks into the core or the recorded IR.
pub struct Canvas2dBackend {
    ctx: CanvasRenderingContext2d,
    scale_factor: f32,
    transform: Transform2D,
    images: HashMap<TextureId, HtmlImageElement>,
}

impl Canvas2dBackend {
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Self {
            ctx,
            scale_factor: 1.0,
            transform: Transform2D::IDENTITY,
            images: HashMap::new(),
        }
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor.max(0.0);
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn context(&self) -> &CanvasRenderingContext2d {
        &self.ctx
    }

    /// Registers a decoded image so `DrawImage` commands can reference it.
    pub fn register_image(&mut self, texture: TextureId, image: HtmlImageElement) {
        self.images.insert(texture, image);
    }

    fn apply_transform(&self, transform: Transform2D) {
        let s = self.scale_factor as f64;
        let _ = self.ctx.set_transform(
            transform.x_axis.x as f64 * s,
            transform.x_axis.y as f64 * s,
            transform.y_axis.x as f64 * s,
            transform.y_axis.y as f64 * s,
            transform.origin.x as f64 * s,
            transform.origin.y as f64 * s,
        );
    }

    /// The loaded image's size in logical pixels, which a `DrawImage` without an
    /// explicit source rect samples in full.
    fn image_size(&self, command: &DrawCommand) -> Option<(f32, f32)> {
        match command {
            DrawCommand::DrawImage { texture, .. } => self
                .images
                .get(texture)
                .map(|image| (image.width() as f32, image.height() as f32)),
            _ => None,
        }
    }

    fn execute(&mut self, command: &DrawCommand) {
        let image_size = self.image_size(command);

        match ops::resolve(command, self.scale_factor, image_size) {
            CanvasOp::Save => self.ctx.save(),
            CanvasOp::Restore => self.ctx.restore(),
            CanvasOp::SetTransform(transform) => {
                self.transform = transform;
                self.apply_transform(transform);
            }
            CanvasOp::SetOpacity(opacity) => self.ctx.set_global_alpha(opacity as f64),
            CanvasOp::ClipRect { device } => {
                // Clip rectangles arrive in device space: build the path with the
                // DPR transform applied, then restore the logical transform.
                let _ = self.ctx.set_transform(
                    self.scale_factor as f64,
                    0.0,
                    0.0,
                    self.scale_factor as f64,
                    0.0,
                    0.0,
                );
                self.ctx.begin_path();
                self.ctx.rect(
                    device.left() as f64,
                    device.top() as f64,
                    device.size.width as f64,
                    device.size.height as f64,
                );
                self.ctx.clip();
                self.apply_transform(self.transform);
            }
            CanvasOp::FillRect { rect, fill } => {
                self.ctx.set_fill_style_str(&fill);
                self.ctx.fill_rect(
                    rect.left() as f64,
                    rect.top() as f64,
                    rect.size.width as f64,
                    rect.size.height as f64,
                );
            }
            CanvasOp::StrokeRect {
                rect,
                stroke,
                width,
            } => {
                self.ctx.set_stroke_style_str(&stroke);
                self.ctx.set_line_width(width as f64);
                self.ctx.stroke_rect(
                    rect.left() as f64,
                    rect.top() as f64,
                    rect.size.width as f64,
                    rect.size.height as f64,
                );
            }
            CanvasOp::Line {
                from,
                to,
                stroke,
                width,
            } => {
                self.ctx.set_stroke_style_str(&stroke);
                self.ctx.set_line_width(width as f64);
                self.ctx.begin_path();
                self.ctx.move_to(from.x as f64, from.y as f64);
                self.ctx.line_to(to.x as f64, to.y as f64);
                self.ctx.stroke();
            }
            CanvasOp::FillCircle {
                center,
                radius,
                fill,
            } => {
                self.ctx.set_fill_style_str(&fill);
                self.path_circle(center, radius);
                self.ctx.fill();
            }
            CanvasOp::StrokeCircle {
                center,
                radius,
                stroke,
                width,
            } => {
                self.ctx.set_stroke_style_str(&stroke);
                self.ctx.set_line_width(width as f64);
                self.path_circle(center, radius);
                self.ctx.stroke();
            }
            CanvasOp::FillRoundedRect {
                rect,
                corners,
                fill,
            } => {
                self.ctx.set_fill_style_str(&fill);
                self.path_rounded_rect(rect, corners);
                self.ctx.fill();
            }
            CanvasOp::StrokeRoundedRect {
                rect,
                corners,
                stroke,
                width,
            } => {
                self.ctx.set_stroke_style_str(&stroke);
                self.ctx.set_line_width(width as f64);
                self.path_rounded_rect(rect, corners);
                self.ctx.stroke();
            }
            CanvasOp::DrawImage {
                texture,
                source,
                destination,
                alpha,
            } => {
                let Some(image) = self.images.get(&texture).cloned() else {
                    return;
                };
                self.ctx.set_global_alpha(alpha as f64);
                let _ = self
                    .ctx
                    .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                        &image,
                        source.left() as f64,
                        source.top() as f64,
                        source.size.width as f64,
                        source.size.height as f64,
                        destination.left() as f64,
                        destination.top() as f64,
                        destination.size.width as f64,
                        destination.size.height as f64,
                    );
            }
            CanvasOp::DrawText {
                text,
                position,
                font,
                align,
                fill,
            } => {
                self.ctx.set_fill_style_str(&fill);
                self.ctx.set_font(&font);
                self.ctx.set_text_align(align);
                let _ = self
                    .ctx
                    .fill_text(&text, position.x as f64, position.y as f64);
            }
            CanvasOp::Skip => {}
        }
    }

    fn path_circle(&self, center: Vec2, radius: f32) {
        self.ctx.begin_path();
        let _ = self.ctx.arc(
            center.x as f64,
            center.y as f64,
            radius as f64,
            0.0,
            std::f64::consts::TAU,
        );
    }

    /// Builds a rounded-rectangle path with per-corner quarter-circle corners.
    /// `corners` arrives already clamped to half the smaller side.
    fn path_rounded_rect(&self, rect: Rect, corners: CornerRadii) {
        let half_pi = std::f64::consts::FRAC_PI_2;
        let (left, top) = (rect.left() as f64, rect.top() as f64);
        let (right, bottom) = (rect.right() as f64, rect.bottom() as f64);
        let (tl, tr, br, bl) = (
            corners.top_left as f64,
            corners.top_right as f64,
            corners.bottom_right as f64,
            corners.bottom_left as f64,
        );

        self.ctx.begin_path();
        self.ctx.move_to(left + tl, top);
        self.ctx.line_to(right - tr, top);
        if tr > 0.0 {
            let _ = self.ctx.arc(right - tr, top + tr, tr, -half_pi, 0.0);
        }
        self.ctx.line_to(right, bottom - br);
        if br > 0.0 {
            let _ = self.ctx.arc(right - br, bottom - br, br, 0.0, half_pi);
        }
        self.ctx.line_to(left + bl, bottom);
        if bl > 0.0 {
            let _ = self
                .ctx
                .arc(left + bl, bottom - bl, bl, half_pi, 2.0 * half_pi);
        }
        self.ctx.line_to(left, top + tl);
        if tl > 0.0 {
            let _ = self
                .ctx
                .arc(left + tl, top + tl, tl, 2.0 * half_pi, 3.0 * half_pi);
        }
        self.ctx.close_path();
    }
}

impl RenderBackend for Canvas2dBackend {
    type Error = CanvasError;

    fn begin_frame(&mut self, viewport: ViewportSize) -> Result<(), Self::Error> {
        let canvas = self.ctx.canvas().ok_or(CanvasError::MissingCanvas)?;
        let device = viewport.device_size(self.scale_factor);
        canvas.set_width(device.width.round().max(0.0) as u32);
        canvas.set_height(device.height.round().max(0.0) as u32);

        self.transform = Transform2D::IDENTITY;
        self.ctx.set_global_alpha(1.0);
        self.apply_transform(Transform2D::IDENTITY);
        Ok(())
    }

    fn submit(&mut self, list: &DrawList) -> Result<(), Self::Error> {
        for command in list.commands() {
            self.execute(command);
        }
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
