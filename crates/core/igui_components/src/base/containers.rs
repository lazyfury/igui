use super::*;

pub struct VBox {
    spec: Spec,
    style: FlexStyle,
}

impl Default for VBox {
    fn default() -> Self {
        Self {
            spec: Spec::default(),
            style: FlexStyle::column().gap(8.0).padding(Edges::all(16.0)),
        }
    }
}

impl VBox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn separation(mut self, separation: f32) -> Self {
        self.style.gap = separation;
        self.style.cross_gap = separation;
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.style.padding = padding;
        self
    }
}

impl Component for VBox {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "VBox"
    }

    fn container(&self) -> Container {
        Container::Flex(self.style)
    }
}

/// A horizontal stacking container (a row [`Flex`] with a separation).
pub struct HBox {
    spec: Spec,
    style: FlexStyle,
}

impl Default for HBox {
    fn default() -> Self {
        Self {
            spec: Spec::default(),
            style: FlexStyle::row().gap(8.0).padding(Edges::all(16.0)),
        }
    }
}

impl HBox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn separation(mut self, separation: f32) -> Self {
        self.style.gap = separation;
        self.style.cross_gap = separation;
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.style.padding = padding;
        self
    }
}

impl Component for HBox {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "HBox"
    }

    fn container(&self) -> Container {
        Container::Flex(self.style)
    }
}

/// A configurable flex container.
#[derive(Default)]
pub struct Flex {
    spec: Spec,
    style: FlexStyle,
}

impl Flex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn row() -> Self {
        Self {
            style: FlexStyle::row(),
            ..Self::default()
        }
    }

    pub fn column() -> Self {
        Self {
            style: FlexStyle::column(),
            ..Self::default()
        }
    }

    pub fn direction(mut self, direction: FlexDirection) -> Self {
        self.style.direction = direction;
        self
    }

    pub fn justify(mut self, justify: igui_ui::Justify) -> Self {
        self.style.justify = justify;
        self
    }

    pub fn align(mut self, align: igui_ui::Align) -> Self {
        self.style.align = align;
        self
    }

    pub fn align_content(mut self, align_content: igui_ui::AlignContent) -> Self {
        self.style.align_content = align_content;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.style.wrap = wrap;
        self
    }

    /// Gap between items along the main axis (also called separation).
    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self.style.cross_gap = gap;
        self
    }

    pub fn cross_gap(mut self, gap: f32) -> Self {
        self.style.cross_gap = gap;
        self
    }

    pub fn separation(self, separation: f32) -> Self {
        self.gap(separation)
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.style.padding = padding;
        self
    }
}

impl Component for Flex {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Flex"
    }

    fn container(&self) -> Container {
        Container::Flex(self.style)
    }
}

/// A vertical flex stack with zero default padding/gap.
pub struct Column {
    flex: Flex,
}

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

impl Column {
    pub fn new() -> Self {
        Self {
            flex: Flex::column().gap(0.0).padding(Edges::ZERO),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.flex = self.flex.gap(gap);
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.flex = self.flex.padding(padding);
        self
    }

    pub fn align(mut self, align: igui_ui::Align) -> Self {
        self.flex = self.flex.align(align);
        self
    }

    pub fn justify(mut self, justify: igui_ui::Justify) -> Self {
        self.flex = self.flex.justify(justify);
        self
    }
}

impl Component for Column {
    fn spec(&mut self) -> &mut Spec {
        self.flex.spec()
    }

    fn name(&self) -> &'static str {
        "Column"
    }

    fn container(&self) -> Container {
        self.flex.container()
    }
}

/// A horizontal flex row with zero default padding/gap.
pub struct Row {
    flex: Flex,
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    pub fn new() -> Self {
        Self {
            flex: Flex::row().gap(0.0).padding(Edges::ZERO),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.flex = self.flex.gap(gap);
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.flex = self.flex.padding(padding);
        self
    }

    pub fn align(mut self, align: igui_ui::Align) -> Self {
        self.flex = self.flex.align(align);
        self
    }

    pub fn justify(mut self, justify: igui_ui::Justify) -> Self {
        self.flex = self.flex.justify(justify);
        self
    }
}

impl Component for Row {
    fn spec(&mut self) -> &mut Spec {
        self.flex.spec()
    }

    fn name(&self) -> &'static str {
        "Row"
    }

    fn container(&self) -> Container {
        self.flex.container()
    }
}

/// A grid container with fixed / `fr` / auto tracks.
pub struct Grid {
    spec: Spec,
    style: GridStyle,
}

impl Grid {
    pub fn new(columns: Vec<Track>) -> Self {
        Self {
            spec: Spec::default(),
            style: GridStyle::new(columns),
        }
    }

    pub fn rows(mut self, rows: Vec<Track>) -> Self {
        self.style.rows = rows;
        self
    }

    pub fn align_items(mut self, align: igui_ui::Align) -> Self {
        self.style.align_items = align;
        self
    }

    pub fn justify_items(mut self, align: igui_ui::Align) -> Self {
        self.style.justify_items = align;
        self
    }

    pub fn align_content(mut self, align: igui_ui::AlignContent) -> Self {
        self.style.align_content = align;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style.column_gap = gap;
        self.style.row_gap = gap;
        self
    }

    pub fn column_gap(mut self, gap: f32) -> Self {
        self.style.column_gap = gap;
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.row_gap = gap;
        self
    }

    pub fn padding(mut self, padding: Edges) -> Self {
        self.style.padding = padding;
        self
    }
}

impl Component for Grid {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Grid"
    }

    fn container(&self) -> Container {
        Container::Grid(self.style.clone())
    }
}
