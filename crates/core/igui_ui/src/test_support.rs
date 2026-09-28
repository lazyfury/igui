//! Test-only fixtures.
//!
//! Before the `Widget` removal, tests built controls with
//! `Control::new(data, Widget::…)`. That enum is gone; a `Control` now takes a
//! [`Container`] plus an optional [`ControlContent`]. [`TestControl`] keeps the
//! old, readable vocabulary for layout/paint tests and maps it onto the new
//! pair in one place.
#![cfg(test)]
#![allow(dead_code)]

use crate::content::{Container, ContentRef, PanelContent, TextContent};
use crate::layout::{FlexStyle, GridStyle, TextOptions};
use igui_core::Color;
use igui_render::TextAlign;

/// A test shorthand for a control's container + content.
pub enum TestControl {
    /// A filled panel.
    Panel { color: Color, border: Option<Color> },
    /// A flex container.
    Flex(FlexStyle),
    /// A grid container.
    Grid(GridStyle),
    /// A text block.
    Label {
        text: String,
        font_size: f32,
        color: Color,
        options: TextOptions,
    },
}

impl TestControl {
    /// A borderless filled panel.
    pub fn panel(color: Color) -> Self {
        Self::Panel {
            color,
            border: None,
        }
    }

    /// A left-aligned text block.
    pub fn label(
        text: impl Into<String>,
        font_size: f32,
        color: Color,
        options: TextOptions,
    ) -> Self {
        Self::Label {
            text: text.into(),
            font_size,
            color,
            options,
        }
    }

    /// Splits into the `Control::new` arguments.
    pub fn into_parts(self) -> (Container, Option<ContentRef>) {
        match self {
            Self::Panel { color, border } => (
                Container::Leaf,
                Some(Box::new(PanelContent { color, border })),
            ),
            Self::Flex(style) => (Container::Flex(style), None),
            Self::Grid(style) => (Container::Grid(style), None),
            Self::Label {
                text,
                font_size,
                color,
                options,
            } => (
                Container::Leaf,
                Some(Box::new(TextContent {
                    text,
                    font_size,
                    color,
                    options,
                    align: TextAlign::Left,
                })),
            ),
        }
    }
}
