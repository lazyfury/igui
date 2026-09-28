//! Themed multi-line text areas.
//!
//! [`TextArea`] is [`TextInput`](super::TextInput) grown a third dimension: the
//! same editor, but Enter inserts a line break, the caret moves across wrapped
//! lines, and the content scrolls vertically to follow the caret. It shares the
//! [`TextEdit`] state handle (`TextEdit::multiline(true)`).

use std::cell::RefCell;
use std::rc::Rc;

use igui_core::Edges;
use igui_theme::{radius, ControlSize, SurfaceLevel, Theme};
use igui_ui::{Align, Container, FlexStyle, Justify, SurfaceStyle, TextEdit, TextMeasurer};

use crate::base::{Component, Spec};
use crate::components::text_field::{self, FieldState};

/// A multi-line, wrapping text area.
pub struct TextArea {
    spec: Spec,
    theme: &'static dyn Theme,
    field: FieldState,
    placeholder: String,
    size: ControlSize,
    min_width: f32,
    rows: usize,
    measurer: Option<Rc<RefCell<Rc<dyn TextMeasurer>>>>,
}

impl TextArea {
    pub fn new(theme: &'static dyn Theme) -> Self {
        Self {
            spec: Spec::leaf(),
            theme,
            field: FieldState::new("", true),
            placeholder: String::new(),
            size: theme.default_control(),
            min_width: 0.0,
            rows: 3,
            measurer: None,
        }
    }

    /// The initial value.
    pub fn value(self, value: impl Into<String>) -> Self {
        self.field.edit.borrow_mut().set_text(value);
        self
    }

    /// Shares the field's live state with the caller.
    pub fn shared(&self) -> Rc<RefCell<TextEdit>> {
        self.field.edit.clone()
    }

    /// Text shown, muted, while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The visible height, in text rows.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// A minimum field width.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Overrides the theme's default control size.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Component for TextArea {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "TextArea"
    }

    fn container(&self) -> Container {
        Container::Flex(
            FlexStyle::column()
                .align(Align::Start)
                .justify(Justify::Start)
                .gap(0.0)
                .padding(Edges::ZERO),
        )
    }

    fn bind(&mut self, tree: &mut igui_scene::SceneTree) {
        if self.measurer.is_none() {
            self.measurer = Some(igui_ui::text_measurer_handle(tree));
        }
    }

    fn prepare(&mut self) {
        let theme = self.theme;
        let line_height = match self.measurer.as_ref() {
            Some(handle) => handle
                .borrow()
                .line_height(theme.font_size(igui_theme::TextSize::Small)),
            None => theme.row_height(),
        };
        if self.spec.data.min_size.height <= 0.0 {
            self.spec.data.min_size.height =
                line_height * self.rows as f32 + 2.0 * theme.control_padding_y();
        }
        if self.min_width > 0.0 {
            self.spec.data.min_size.width = self.min_width;
        }
        self.spec.background = Some(Box::new(move |_| {
            let palette = theme.palette();
            SurfaceStyle::new(theme.surface(SurfaceLevel::Base))
                .border(palette.border)
                .radius(radius::SM)
        }));
        let measurer = self
            .measurer
            .clone()
            .unwrap_or_else(|| Rc::new(RefCell::new(Rc::new(igui_ui::ApproxTextMeasurer))));
        text_field::wire(
            &mut self.spec,
            theme,
            measurer,
            self.field.clone(),
            false,
            true,
            self.placeholder.clone(),
        );
    }
}

crate::impl_scene_child!(TextArea);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Flex;
    use igui_core::{Edges, InputEvent, Key, Size, ViewportSize};
    use igui_scene::SceneTree;
    use igui_theme::{default_theme, Mode};
    use igui_ui::MouseFilter;

    fn mount(tree: &mut SceneTree, area: TextArea) -> igui_core::NodeId {
        tree.add_child(
            tree.root(),
            Flex::column()
                .gap(0.0)
                .padding(Edges::ZERO)
                .mouse_filter(MouseFilter::Ignore)
                .child(area),
        );
        tree.iter().last().unwrap()
    }

    fn focus(tree: &mut SceneTree, id: igui_core::NodeId) {
        igui_ui::layout(tree, ViewportSize::new(Size::new(400.0, 300.0)));
        let rect = tree.data::<igui_ui::Control>(id).unwrap().data.rect;
        igui_ui::handle_input(
            tree,
            &InputEvent::PointerDown {
                position: rect.center(),
                button: igui_core::PointerButton::Left,
            },
        );
    }

    #[test]
    fn enter_inserts_a_newline() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let area = TextArea::new(theme).min_width(200.0);
        let shared = area.shared();
        let id = mount(&mut tree, area);
        focus(&mut tree, id);
        igui_ui::handle_input(&mut tree, &InputEvent::TextInput { text: "a".into() });
        igui_ui::handle_input(&mut tree, &InputEvent::KeyDown { key: Key::Enter });
        igui_ui::handle_input(&mut tree, &InputEvent::TextInput { text: "b".into() });
        assert_eq!(shared.borrow().text(), "a\nb");
        assert_eq!(shared.borrow().line_count(), 2);
    }

    #[test]
    fn down_moves_across_lines() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let area = TextArea::new(theme).value("ab\ncd").min_width(200.0);
        let shared = area.shared();
        let id = mount(&mut tree, area);
        focus(&mut tree, id);
        // Caret starts at the end; move to the first line, column 1.
        shared.borrow_mut().set_caret(1);
        igui_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::ArrowDown,
            },
        );
        assert_eq!(shared.borrow().caret(), 4); // 'c' line, column 1 => after 'c'
    }
}
