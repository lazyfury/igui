//! Themed single-line text fields.
//!
//! [`TextInput`] is an interactive editor, not a passive label: it owns a
//! [`TextEdit`] (text, caret, selection, IME preedit), captures the keyboard
//! while focused, and draws its own caret and selection. The caller shares the
//! state with [`TextInput::shared`] to read the value, or sets the initial
//! value with [`TextInput::value`].
//!
//! ```ignore
//! let field = TextInput::new(theme).placeholder("Search");
//! let value = field.shared();
//! tree.add_child(root, field);
//! // after routing input: value.borrow().text()
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use rough_core::Edges;
use rough_theme::{radius, ControlSize, SurfaceLevel, Theme};
use rough_ui::{Align, FlexStyle, Justify, SurfaceStyle, TextEdit, TextMeasurer, Widget};

use crate::base::{Component, Spec};
use crate::components::text_field::{self, FieldState};

/// A compact, editable single-line text field.
pub struct TextInput {
    spec: Spec,
    theme: &'static dyn Theme,
    field: FieldState,
    placeholder: String,
    masked: bool,
    size: ControlSize,
    min_width: f32,
    measurer: Option<Rc<RefCell<Rc<dyn TextMeasurer>>>>,
}

impl TextInput {
    pub fn new(theme: &'static dyn Theme) -> Self {
        Self {
            spec: Spec::leaf(),
            theme,
            field: FieldState::new("", false),
            placeholder: String::new(),
            masked: false,
            size: theme.default_control(),
            min_width: 0.0,
            measurer: None,
        }
    }

    /// The initial value.
    pub fn value(self, value: impl Into<String>) -> Self {
        self.field.edit.borrow_mut().set_text(value);
        self
    }

    /// Shares the field's live state with the caller (read `text()` after
    /// routing input; write through it to change the value).
    pub fn shared(&self) -> Rc<RefCell<TextEdit>> {
        self.field.edit.clone()
    }

    /// Text shown, muted, while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Renders the value as one dot per character (passwords).
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
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

impl Component for TextInput {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "TextInput"
    }

    fn widget(&self) -> Widget {
        Widget::Flex(
            FlexStyle::row()
                .align(Align::Center)
                .justify(Justify::Start)
                .gap(0.0)
                .padding(Edges::ZERO),
        )
    }

    fn bind(&mut self, tree: &mut rough_scene::SceneTree) {
        if self.measurer.is_none() {
            self.measurer = Some(rough_ui::text_measurer_handle(tree));
        }
    }

    fn prepare(&mut self) {
        let theme = self.theme;
        if self.spec.data.min_size.height <= 0.0 {
            self.spec.data.min_size.height = theme.control_height(self.size);
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
            .unwrap_or_else(|| Rc::new(RefCell::new(Rc::new(rough_ui::ApproxTextMeasurer))));
        text_field::wire(
            &mut self.spec,
            theme,
            measurer,
            self.field.clone(),
            self.masked,
            false,
            self.placeholder.clone(),
        );
    }
}

crate::impl_scene_child!(TextInput);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Flex;
    use std::cell::RefCell;
    use std::rc::Rc;

    use rough_core::{Edges, ImeEvent, InputEvent, Key, Modifiers, Size, Vec2, ViewportSize};
    use rough_scene::SceneTree;
    use rough_theme::{default_theme, Mode};
    use rough_ui::{Clipboard, MouseFilter, Widget};

    fn mount(tree: &mut SceneTree, input: TextInput) -> rough_core::NodeId {
        let page = tree.add_child(
            tree.root(),
            Flex::column()
                .gap(0.0)
                .padding(Edges::ZERO)
                .mouse_filter(MouseFilter::Ignore)
                .child(input),
        );
        tree.children(page).unwrap()[0]
    }

    /// Lays out and clicks the field so it has focus.
    fn focus(tree: &mut SceneTree, id: rough_core::NodeId) {
        rough_ui::layout(tree, ViewportSize::new(Size::new(400.0, 300.0)));
        let center = tree
            .data::<rough_ui::Control>(id)
            .unwrap()
            .data
            .rect
            .center();
        rough_ui::handle_input(
            tree,
            &InputEvent::PointerDown {
                position: center,
                button: rough_core::PointerButton::Left,
            },
        );
    }

    #[test]
    fn typing_inserts_and_backspace_deletes() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        rough_ui::handle_input(&mut tree, &InputEvent::TextInput { text: "hi".into() });
        assert_eq!(shared.borrow().text(), "hi");
        rough_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::Backspace,
            },
        );
        assert_eq!(shared.borrow().text(), "h");
    }

    #[test]
    fn dragging_extends_the_selection() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).value("hello world").min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        let rect = tree.data::<rough_ui::Control>(id).unwrap().data.rect;
        rough_ui::handle_input(
            &mut tree,
            &InputEvent::PointerMove {
                position: Vec2::new(rect.left() + 2.0, rect.center().y),
            },
        );
        assert!(shared.borrow().has_selection());
    }

    #[test]
    fn a_double_click_selects_the_word() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).value("foo bar").min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        let rect = tree.data::<rough_ui::Control>(id).unwrap().data.rect;
        // Beyond the text: the caret clamps to the end, inside "bar".
        rough_ui::handle_input(
            &mut tree,
            &InputEvent::DoubleClick {
                position: Vec2::new(rect.right() - 2.0, rect.center().y),
            },
        );
        assert_eq!(shared.borrow().selected_text(), Some("bar"));
    }

    #[test]
    fn copy_cut_and_paste_use_the_clipboard() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let clipboard = Rc::new(RefCell::new(rough_ui::MemoryClipboard::default()));
        rough_ui::set_clipboard(&mut tree, clipboard.clone());
        let input = TextInput::new(theme).value("hello world").min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        shared.borrow_mut().select_all();
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        rough_ui::handle_input(&mut tree, &InputEvent::ModifiersChanged(ctrl));

        rough_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::Character('c'),
            },
        );
        assert_eq!(clipboard.borrow().get().as_deref(), Some("hello world"));

        rough_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::Character('x'),
            },
        );
        assert_eq!(clipboard.borrow().get().as_deref(), Some("hello world"));
        assert_eq!(shared.borrow().text(), "");

        rough_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::Character('v'),
            },
        );
        assert_eq!(shared.borrow().text(), "hello world");
    }

    #[test]
    fn space_and_tab_are_inserted() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        // Space and Tab are named keys the field inserts itself (guarded by the
        // IME preedit); no host `TextInput` is involved.
        rough_ui::handle_input(&mut tree, &InputEvent::KeyDown { key: Key::Space });
        rough_ui::handle_input(&mut tree, &InputEvent::KeyDown { key: Key::Tab });
        assert_eq!(shared.borrow().text(), " \t");
    }

    #[test]
    fn ime_preedit_is_transient_until_committed() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        rough_ui::handle_input(
            &mut tree,
            &InputEvent::Ime(ImeEvent::Preedit {
                text: "ni".into(),
                cursor: None,
            }),
        );
        assert_eq!(shared.borrow().text(), "");
        assert!(shared.borrow().has_preedit());
        rough_ui::handle_input(&mut tree, &InputEvent::Ime(ImeEvent::Commit("你".into())));
        assert_eq!(shared.borrow().text(), "你");
        assert!(!shared.borrow().has_preedit());
    }

    #[test]
    fn shift_arrow_extends_a_selection() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let input = TextInput::new(theme).value("abc").min_width(200.0);
        let shared = input.shared();
        let id = mount(&mut tree, input);
        focus(&mut tree, id);
        rough_ui::handle_input(&mut tree, &InputEvent::ModifiersChanged(Modifiers::SHIFT));
        rough_ui::handle_input(
            &mut tree,
            &InputEvent::KeyDown {
                key: Key::ArrowLeft,
            },
        );
        assert_eq!(shared.borrow().selection(), Some((2, 3)));
    }

    #[test]
    fn a_masked_field_keeps_the_real_text() {
        let theme = default_theme(Mode::Dark);
        let input = TextInput::new(theme).value("hunter2").masked(true);
        assert_eq!(input.shared().borrow().text(), "hunter2");
    }

    /// The field is a leaf control with a caret provider, not a label.
    #[test]
    fn the_field_is_interactive() {
        let theme = default_theme(Mode::Dark);
        let mut tree = SceneTree::new();
        let id = mount(&mut tree, TextInput::new(theme));
        let control = tree.data::<rough_ui::Control>(id).unwrap();
        assert!(control.focusable);
        assert!(control.caret_provider.is_some());
        assert!(control.key_callback.is_some());
        assert!(matches!(control.widget, Widget::Flex(_)));
    }
}
