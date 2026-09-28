//! A control's container / content model.
//!
//! A `Control` is a rectangle with two orthogonal roles: how it lays out its
//! children ([`Container`]) and how it sizes and draws itself
//! ([`ControlContent`]). Splitting them replaces the old closed `Widget` enum,
//! so a component can be a container and self-draw at the same time — and any
//! crate can add a control without touching `igui_ui`.
//!
//! `ControlContent` is the Godot pair `_get_minimum_size` + `_draw`: a leaf
//! reports the size it wants and paints its own pixels through [`PaintEnv`],
//! which owns the text measurer and the shared line cache.

use igui_core::{Color, NodeId, Rect, Size, Vec2};
use igui_render::{PaintContext, TextAlign};

use crate::control::LayoutCache;
use crate::decor::InteractState;
use crate::layout::{
    self, layout_text, measure_weighted_with, ContentSize, FlexStyle, GridStyle, TextMeasurer,
    TextOptions, WordBreak,
};
use crate::paint::SurfaceStyle;

/// How a control lays out its child controls.
///
/// `Leaf` controls keep the anchor/offset model and let each child resolve its
/// own rectangle; `Flex` / `Grid` controls own their children's rectangles.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Container {
    /// No container behaviour: each child resolves its own anchor/offset rect.
    #[default]
    Leaf,
    /// Children are placed on the main/cross axes by a flex box.
    Flex(FlexStyle),
    /// Children are placed into tracks by a grid.
    Grid(GridStyle),
}

impl Container {
    /// A flex container.
    pub fn flex(style: FlexStyle) -> Self {
        Self::Flex(style)
    }

    /// A grid container.
    pub fn grid(style: GridStyle) -> Self {
        Self::Grid(style)
    }

    /// The flex style, if this is a flex container.
    pub fn flex_style(&self) -> Option<&FlexStyle> {
        match self {
            Self::Flex(style) => Some(style),
            _ => None,
        }
    }

    /// The grid style, if this is a grid container.
    pub fn grid_style(&self) -> Option<&GridStyle> {
        match self {
            Self::Grid(style) => Some(style),
            _ => None,
        }
    }
}

/// Vertical placement of a text block inside its rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextVAlign {
    /// The first baseline sits at the top of the rectangle.
    Top,
    /// The block is centered vertically.
    Center,
}

/// A control's intrinsic sizing and self-drawing (Godot `_get_minimum_size`
/// and `_draw`).
///
/// Only `draw` is required: a pure container or a control whose pixels are
/// supplied by chrome leaves `measure` at its default and reports
/// [`ContentSize::ZERO`]. `draw` must paint the control's own content; a
/// component that needs reusable chrome around it keeps using
/// `NodeDecor`/`foreground_decor`, or draws it here.
pub trait ControlContent {
    /// The size the content wants, given the space a parent can offer.
    fn measure(&self, _measurer: &dyn TextMeasurer, _available: Size) -> ContentSize {
        ContentSize::ZERO
    }

    /// Paints the content into `rect` in the current transform space.
    fn draw(&self, id: NodeId, env: &mut PaintEnv<'_>, rect: Rect, state: InteractState);

    /// The text this content renders, if any (used for introspection / tests).
    fn as_text(&self) -> Option<&str> {
        None
    }

    /// The text size this content uses, if it renders text.
    fn as_text_size(&self) -> Option<f32> {
        None
    }

    /// Replaces the content's text, returning whether it changed. No-op for
    /// content that has no text.
    fn set_text(&mut self, _text: &str) -> bool {
        false
    }

    /// Replaces the content's main color, returning whether it changed. No-op
    /// for content that has no color.
    fn set_color(&mut self, _color: Color) -> bool {
        false
    }
}

/// A boxed [`ControlContent`], as stored by a `Control`.
pub type ContentRef = Box<dyn ControlContent>;

/// The drawing surface passed to [`ControlContent::draw`].
///
/// It owns the backend-neutral [`PaintContext`], the active [`TextMeasurer`]
/// and the shared text-line cache, so a content implementation draws text
/// without carrying its own cache.
pub struct PaintEnv<'a> {
    /// The backend-neutral paint context to emit commands into.
    pub ctx: &'a mut PaintContext,
    /// The measurer that will also drive layout for this frame.
    pub measurer: &'a dyn TextMeasurer,
    cache: &'a mut LayoutCache,
}

impl<'a> PaintEnv<'a> {
    pub(crate) fn new(
        ctx: &'a mut PaintContext,
        measurer: &'a dyn TextMeasurer,
        cache: &'a mut LayoutCache,
    ) -> Self {
        Self {
            ctx,
            measurer,
            cache,
        }
    }

    /// Paints a themed surface (fill + border + corner radius).
    pub fn surface(&mut self, rect: Rect, style: &SurfaceStyle) {
        crate::paint::surface(self.ctx, rect, style);
    }

    /// Lays out and paints a block of text inside `rect`, vertically placed by
    /// `v` and each line aligned horizontally by `h`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_block(
        &mut self,
        id: NodeId,
        text: &str,
        font_size: f32,
        color: Color,
        options: TextOptions,
        rect: Rect,
        h: TextAlign,
        v: TextVAlign,
    ) {
        let lines = crate::ui::Ui::new().layout_text_cached(
            self.cache,
            self.measurer,
            id,
            text,
            font_size,
            rect.size.width,
            options,
        );
        let step = self.measurer.line_height(font_size);
        let ascent = self.measurer.ascent(font_size);
        let block = lines.len() as f32 * step;
        let mut baseline = match v {
            TextVAlign::Top => rect.top() + ascent,
            TextVAlign::Center => rect.center().y - block / 2.0 + ascent,
        };
        for line in lines.iter() {
            let x = match h {
                TextAlign::Left => rect.left(),
                TextAlign::Center => rect.center().x,
                TextAlign::Right => rect.right(),
            };
            self.ctx.draw_text_weighted(
                line.clone(),
                Vec2::new(x, baseline),
                font_size,
                options.weight,
                h,
                color,
            );
            baseline += step;
        }
    }
}

/// A flat filled rectangle with an optional 1px border.
#[derive(Clone)]
pub struct PanelContent {
    /// The fill color.
    pub color: Color,
    /// The border color, or `None` for no border.
    pub border: Option<Color>,
}

impl ControlContent for PanelContent {
    fn draw(&self, _id: NodeId, env: &mut PaintEnv<'_>, rect: Rect, _state: InteractState) {
        env.ctx.fill_rect(rect, self.color);
        if let Some(border) = self.border {
            env.ctx.stroke_rect(rect, 1.0, border);
        }
    }
}

/// A block of text: intrinsic size from the measurer, painted top-aligned.
#[derive(Clone)]
pub struct TextContent {
    /// The text to lay out.
    pub text: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Fill color.
    pub color: Color,
    /// Wrap / weight / max-lines options.
    pub options: TextOptions,
    /// Horizontal alignment inside the control rectangle.
    pub align: TextAlign,
}

impl TextContent {
    /// A top-aligned, left-aligned text block.
    pub fn new(text: impl Into<String>, font_size: f32, color: Color) -> Self {
        Self {
            text: text.into(),
            font_size,
            color,
            options: TextOptions::default(),
            align: TextAlign::Left,
        }
    }
}

impl ControlContent for TextContent {
    fn measure(&self, measurer: &dyn TextMeasurer, available: Size) -> ContentSize {
        let line_h = measurer.line_height(self.font_size);
        let natural =
            measure_weighted_with(measurer, &self.text, self.font_size, self.options.weight);
        // `word_break` only has meaning while soft wrapping is on; with
        // `wrap: false` the text never breaks, so the min must be the full run
        // (the default `Word` granularity), not the per-mode break units.
        let break_mode = if self.options.wrap {
            self.options.word_break
        } else {
            WordBreak::Word
        };
        let mut min_width = layout::longest_unit_width_weighted_with(
            measurer,
            &self.text,
            self.font_size,
            break_mode,
            self.options.weight,
        );
        // A wrapping label hard-breaks an overlong word when it paints, so its
        // minimum must never exceed the width the parent offered.
        if self.options.wrap && available.width > 0.0 {
            min_width = min_width.min(available.width);
        }
        let min = Size::new(min_width, line_h);

        let preferred =
            if self.options.wrap && available.width > 0.0 && available.width + 1e-3 < natural.width
            {
                let lines = layout_text(
                    measurer,
                    &self.text,
                    self.font_size,
                    available.width,
                    self.options,
                );
                Size::new(available.width, lines.len() as f32 * line_h)
            } else {
                let mut height = natural.height;
                if let Some(max_lines) = self.options.max_lines {
                    height = height.min(max_lines as f32 * line_h);
                }
                Size::new(natural.width, height)
            };
        ContentSize::new(min, preferred.max(min))
    }

    fn draw(&self, id: NodeId, env: &mut PaintEnv<'_>, rect: Rect, _state: InteractState) {
        env.text_block(
            id,
            &self.text,
            self.font_size,
            self.color,
            self.options,
            rect,
            self.align,
            TextVAlign::Top,
        );
    }

    fn as_text(&self) -> Option<&str> {
        Some(&self.text)
    }

    fn as_text_size(&self) -> Option<f32> {
        Some(self.font_size)
    }

    fn set_text(&mut self, text: &str) -> bool {
        if self.text == text {
            false
        } else {
            self.text.clear();
            self.text.push_str(text);
            true
        }
    }

    fn set_color(&mut self, color: Color) -> bool {
        if self.color == color {
            false
        } else {
            self.color = color;
            true
        }
    }
}

/// A button's own surface plus centered label, driven by the interaction state.
///
/// The fill reflects hover / press through [`InteractState`] rather than a
/// separately tracked `ButtonState`.
#[derive(Clone)]
pub struct ButtonContent {
    /// The label.
    pub text: String,
    /// Label font size.
    pub font_size: f32,
    /// Resting fill.
    pub color: Color,
    /// Fill while hovered.
    pub hover_color: Color,
    /// Fill while pressed.
    pub pressed_color: Color,
    /// Label color (and the border color at 35% alpha).
    pub text_color: Color,
    /// Wrap / weight / max-lines options.
    pub options: TextOptions,
}

impl ButtonContent {
    /// A button with a label at the given font size.
    pub fn new(text: impl Into<String>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            font_size,
            color: Color::new(0.24, 0.28, 0.38, 1.0),
            hover_color: Color::new(0.32, 0.38, 0.50, 1.0),
            pressed_color: Color::new(0.20, 0.45, 0.78, 1.0),
            text_color: Color::new(0.95, 0.97, 1.0, 1.0),
            options: TextOptions::no_wrap(),
        }
    }

    /// The fill for the current interaction state.
    pub fn fill(&self, state: InteractState) -> Color {
        if state.pressed {
            self.pressed_color
        } else if state.hovered {
            self.hover_color
        } else {
            self.color
        }
    }
}

impl ControlContent for ButtonContent {
    fn measure(&self, measurer: &dyn TextMeasurer, available: Size) -> ContentSize {
        let line_h = measurer.line_height(self.font_size);
        let natural =
            measure_weighted_with(measurer, &self.text, self.font_size, self.options.weight);
        let break_mode = if self.options.wrap {
            self.options.word_break
        } else {
            WordBreak::Word
        };
        let mut text_min = layout::longest_unit_width_weighted_with(
            measurer,
            &self.text,
            self.font_size,
            break_mode,
            self.options.weight,
        );
        if self.options.wrap && available.width > 0.0 {
            text_min = text_min.min((available.width - 32.0).max(0.0));
        }
        let min = Size::new(text_min + 32.0, line_h.max(36.0) + 12.0);

        let preferred = if self.options.wrap
            && available.width > 0.0
            && available.width + 1e-3 < natural.width + 32.0
        {
            let inner = (available.width - 32.0).max(0.0);
            let lines = layout_text(measurer, &self.text, self.font_size, inner, self.options);
            Size::new(available.width, lines.len() as f32 * line_h + 12.0)
        } else {
            let mut height = natural.height;
            if let Some(max_lines) = self.options.max_lines {
                height = height.min(max_lines as f32 * line_h);
            }
            Size::new(natural.width + 32.0, height.max(36.0) + 12.0)
        };
        ContentSize::new(min, preferred.max(min))
    }

    fn draw(&self, id: NodeId, env: &mut PaintEnv<'_>, rect: Rect, state: InteractState) {
        env.ctx.fill_rect(rect, self.fill(state));
        env.ctx
            .stroke_rect(rect, 1.0, self.text_color.with_alpha(0.35));
        env.text_block(
            id,
            &self.text,
            self.font_size,
            self.text_color,
            self.options,
            rect,
            TextAlign::Center,
            TextVAlign::Center,
        );
    }

    fn as_text(&self) -> Option<&str> {
        Some(&self.text)
    }

    fn as_text_size(&self) -> Option<f32> {
        Some(self.font_size)
    }

    fn set_text(&mut self, text: &str) -> bool {
        if self.text == text {
            false
        } else {
            self.text.clear();
            self.text.push_str(text);
            true
        }
    }

    fn set_color(&mut self, color: Color) -> bool {
        if self.text_color == color {
            false
        } else {
            self.text_color = color;
            true
        }
    }
}

/// Rough text size estimate (no font shaping in MVP).
///
/// Width assumes an average glyph advance; wide (CJK) characters count double.
pub fn estimate_text_size(text: &str, font_size: f32) -> Size {
    layout::measure(text, font_size)
}
