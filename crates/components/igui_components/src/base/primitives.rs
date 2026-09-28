use super::*;

pub struct Panel {
    spec: Spec,
    color: Color,
    border: Option<Color>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            spec: Spec::default(),
            color: Color::new(0.13, 0.15, 0.20, 1.0),
            border: Some(Color::new(0.26, 0.30, 0.40, 1.0)),
        }
    }
}

impl Panel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn border(mut self, border: Option<Color>) -> Self {
        self.border = border;
        self
    }

    pub fn flat(mut self) -> Self {
        self.border = None;
        self
    }
}

impl Component for Panel {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Panel"
    }

    fn content(&self) -> Option<ContentRef> {
        Some(Box::new(PanelContent {
            color: self.color,
            border: self.border,
        }))
    }
}

/// A text label.
pub struct Label {
    spec: Spec,
    text: String,
    font_size: f32,
    color: Color,
    options: igui_ui::TextOptions,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            spec: Spec::leaf(),
            text: text.into(),
            font_size: 20.0,
            color: Color::new(0.92, 0.94, 0.98, 1.0),
            options: igui_ui::TextOptions::default(),
        }
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.options.wrap = wrap;
        self
    }

    pub fn word_break(mut self, word_break: igui_ui::WordBreak) -> Self {
        self.options = self.options.word_break(word_break);
        self
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.options = self.options.max_lines(max_lines);
        self
    }

    pub fn ellipsis(mut self, ellipsis: bool) -> Self {
        self.options = self.options.ellipsis(ellipsis);
        self
    }

    pub fn text_options(mut self, options: igui_ui::TextOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets the text weight (regular or bold).
    pub fn weight(mut self, weight: igui_core::FontWeight) -> Self {
        self.options = self.options.weight(weight);
        self
    }
}

impl Component for Label {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Label"
    }

    fn content(&self) -> Option<ContentRef> {
        let mut content = TextContent::new(self.text.clone(), self.font_size, self.color);
        content.options = self.options;
        Some(Box::new(content))
    }
}

/// A clickable button with an optional click callback.
pub struct Button {
    spec: Spec,
    data: ButtonContent,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        // Buttons are keyboard-focusable by default (arrow keys / Tab reach
        // them); `.focusable(false)` opts a specific button out.
        let mut spec = Spec::leaf();
        spec.focusable = true;
        Self {
            spec,
            data: ButtonContent::new(text, 18.0),
        }
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.data.font_size = font_size;
        self
    }

    pub fn fill(mut self, color: Color) -> Self {
        self.data.color = color;
        self
    }

    pub fn hover_fill(mut self, color: Color) -> Self {
        self.data.hover_color = color;
        self
    }

    pub fn pressed_fill(mut self, color: Color) -> Self {
        self.data.pressed_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.data.text_color = color;
        self
    }

    pub fn on_click(mut self, callback: impl FnMut() + 'static) -> Self {
        self = Component::on_click(self, callback);
        self
    }
}

impl Component for Button {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Button"
    }

    fn content(&self) -> Option<ContentRef> {
        Some(Box::new(self.data.clone()))
    }
}
