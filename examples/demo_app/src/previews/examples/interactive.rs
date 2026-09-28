//! Stateful preview builders: controls, data and overlays.
//!
//! These either write requests into [`crate::GalleryState`] (drained by
//! [`crate::DemoApp::update`]) or hand back [`ListState`] / [`Router`] handles for
//! the app to sync each frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use igui_components::{
    apply_spec, Button, Card, Checkbox, Column, Component, Flex, List, ListColumn, Menu, MenuItem,
    NodeRef, Panel, ResizeHandle, Router, Row, ScrollView, Spec, Switch, Text, TextArea, TextInput,
};
use igui_core::{Color, Edges, NodeId, Rect, Size, Vec2};
use igui_render::PaintContext;
use igui_scene::{SceneChild, SceneTree};
use igui_theme::{radius, space, Theme, Tone};
use igui_ui::{ContentRef, Control, InteractState, PanelContent, SizeBasis, SurfaceStyle};

use super::Ctx;

pub(crate) fn button(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        Column::new()
            .gap(space::SM)
            .child(
                Row::new()
                    .gap(space::SM)
                    .child(Button::primary("Primary", theme))
                    .child(Button::secondary("Secondary", theme)),
            )
            .child(
                Row::new()
                    .gap(space::SM)
                    .child(Button::ghost("Ghost", theme))
                    .child(Button::destructive("Delete", theme)),
            )
            .child(Button::secondary("Mini", theme).mini()),
    )
}

pub(crate) fn checkbox(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        Column::new()
            .gap(space::SM)
            .child(Checkbox::new("Bold headings", theme).checked(true))
            .child(Checkbox::new("Wrap long lines", theme)),
    )
}

pub(crate) fn switch(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        Column::new()
            .gap(space::SM)
            .child(Switch::new(theme).label("Sync").on(true))
            .child(Switch::new(theme).label("Notifications")),
    )
}

pub(crate) fn resize(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let left = NodeRef::new();
    let width = Rc::new(Cell::new(110.0));
    card.child(
        Row::new()
            .gap(0.0)
            .min_size(0.0, 120.0)
            .child(
                Panel::new()
                    .color(theme.palette().accent)
                    .flat()
                    .min_size(0.0, 120.0)
                    .basis(SizeBasis::Px(width.get()))
                    .shrink(0.0)
                    .ref_(&left),
            )
            .child(
                ResizeHandle::vertical(theme)
                    .target(left.clone())
                    .width(width.clone())
                    .min(64.0)
                    .max(220.0),
            )
            .child(
                Panel::new()
                    .color(theme.palette().surface_hover)
                    .flat()
                    .min_size(0.0, 120.0)
                    .grow(1.0),
            ),
    )
}

pub(crate) fn text_input(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        Column::new()
            .gap(space::SM)
            .child(
                TextInput::new(theme)
                    .placeholder("Search…")
                    .min_width(220.0),
            )
            .child(
                TextInput::new(theme)
                    .value("secret")
                    .masked(true)
                    .min_width(220.0),
            ),
    )
}

pub(crate) fn text_area(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        TextArea::new(theme)
            .placeholder("Write a note…")
            .rows(3)
            .min_width(220.0),
    )
}

pub(crate) fn list(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let count = Rc::new(Cell::new(128usize));
    let list = List::new(theme, theme.row_height(), move |index| {
        vec![format!("Row {index}"), format!("{} KB", index * 2)]
    })
    .columns(vec![ListColumn::flexible(), ListColumn::fixed(64.0)])
    .count(count);
    ctx.lists.push(list.state());
    card.child(list.min_size(0.0, 168.0))
}

pub(crate) fn scroll_view(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let content = Column::new().gap(space::SM).children((0..14).map(|index| {
        Text::small(format!("Line {index} of a tall, clipped column"), theme).tone(Tone::Muted)
    }));
    let view = ScrollView::new(theme).child(content).min_size(0.0, 150.0);
    ctx.scrolls.push(view.state());
    card.child(view)
}

pub(crate) fn router(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let route = Rc::new(Cell::new(0));
    let first = Column::new()
        .gap(space::XXS)
        .child(Text::small("View A", theme))
        .child(Text::caption("The first routed view.", theme).tone(Tone::Muted));
    let second = Column::new()
        .gap(space::XXS)
        .child(Text::small("View B", theme))
        .child(Text::caption("The second routed view.", theme).tone(Tone::Muted));
    let buttons = Row::new()
        .gap(space::SM)
        .child(Button::secondary("View A", theme).on_click({
            let route = route.clone();
            move || route.set(0)
        }))
        .child(Button::secondary("View B", theme).on_click({
            let route = route.clone();
            move || route.set(1)
        }));
    card.child(
        Column::new()
            .gap(space::SM)
            .child(RouterDemo::new(
                theme,
                route,
                first,
                second,
                Rc::clone(ctx.routers),
            ))
            .child(buttons),
    )
}

/// A tiny container that mounts two views behind a [`Router`] and hands the
/// router back so the app can apply route changes each frame.
struct RouterDemo {
    spec: Spec,
    theme: &'static dyn Theme,
    route: Rc<Cell<usize>>,
    first: Column,
    second: Column,
    out: Rc<RefCell<Vec<Router>>>,
}

impl RouterDemo {
    fn new(
        theme: &'static dyn Theme,
        route: Rc<Cell<usize>>,
        first: Column,
        second: Column,
        out: Rc<RefCell<Vec<Router>>>,
    ) -> Self {
        let mut spec = Spec::default();
        spec.data.min_size = Size::new(0.0, 56.0);
        Self {
            spec,
            theme,
            route,
            first,
            second,
            out,
        }
    }
}

impl Component for RouterDemo {
    fn spec(&mut self) -> &mut Spec {
        &mut self.spec
    }

    fn name(&self) -> &'static str {
        "Router"
    }

    fn content(&self) -> Option<ContentRef> {
        Some(Box::new(PanelContent {
            color: self.theme.palette().surface_raised,
            border: Some(self.theme.palette().border),
        }))
    }

    fn build(mut self, tree: &mut SceneTree, parent: NodeId) -> NodeId {
        self.prepare();
        let spec = std::mem::take(self.spec());
        let root = tree.add_control(parent, self.name());
        tree.set_data(
            root,
            Control::new(spec.data, self.container(), self.content()),
        );

        let first = tree.add_child(root, self.first);
        let second = tree.add_child(root, self.second);
        let mut router = Router::with_route(root, self.route.clone());
        router.add_node(first);
        router.add_node(second);
        router.sync(tree);
        self.out.borrow_mut().push(router);

        apply_spec(tree, root, spec);
        root
    }
}

impl SceneChild for RouterDemo {
    fn attach(self, tree: &mut SceneTree, parent: NodeId) -> NodeId {
        <Self as Component>::build(self, tree, parent)
    }
}

pub(crate) fn menu(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let request = ctx.state.menu_request.clone();
    card.child(
        Row::new().child(
            Button::secondary("Open menu", theme)
                .on_click(move || request.set(true))
                .ref_(&ctx.state.menu_anchor),
        ),
    )
}

pub(crate) fn confirm(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let request = ctx.state.confirm_request.clone();
    card.child(Button::secondary("Delete item…", theme).on_click(move || request.set(true)))
}

pub(crate) fn message(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    let request = ctx.state.message_request.clone();
    card.child(Button::secondary("Show toast", theme).on_click(move || request.set(true)))
}

/// The menu opened by the preview's button (built by [`crate::DemoApp::update`]).
pub fn menu_content(tree: &mut SceneTree, node: NodeId, theme: &'static dyn Theme) {
    tree.add_child(
        node,
        Menu::new(theme)
            .item(MenuItem::action("New file", "Ctrl+N", theme))
            .item(MenuItem::new("Open…", theme))
            .separator()
            .item(MenuItem::new("Delete", theme).destructive()),
    );
}

/// A row of buttons whose arrow keys follow `focus_name` links, not the grid:
/// right from `Home` jumps to `Next`, skipping the `Play` in between. The
/// focused button gets a ring; the last one activated (Enter / click) a dot.
pub(crate) fn focus_navigation(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    // Index of the last activated button (0 = none); shared with the click
    // callbacks and read by the foreground at paint time.
    let active = Rc::new(Cell::new(0u8));
    let activate = |index: u8| {
        let active = active.clone();
        move || active.set(index)
    };
    card.child(
        Column::new()
            .gap(space::SM)
            .child(
                Text::caption(
                    "Click a button, then use the arrow keys; Enter activates.",
                    theme,
                )
                .tone(Tone::Muted),
            )
            .child(
                Row::new()
                    .gap(space::SM)
                    .child(
                        Button::primary("Home", theme)
                            .focus_name("home")
                            .focus_neighbor_right("next")
                            .foreground(button_chrome(theme, 1, active.clone()))
                            .on_click(activate(1)),
                    )
                    .child(
                        Button::secondary("Play", theme)
                            .focus_name("play")
                            .focus_neighbor_left("home")
                            .foreground(button_chrome(theme, 2, active.clone()))
                            .on_click(activate(2)),
                    )
                    .child(
                        Button::secondary("Next", theme)
                            .focus_name("next")
                            .focus_neighbor_left("home")
                            .foreground(button_chrome(theme, 3, active.clone()))
                            .on_click(activate(3)),
                    ),
            )
            .child(
                Text::caption(
                    "Right from Home reaches Next — Play is skipped; a dot marks the last activation.",
                    theme,
                )
                .tone(Tone::Subtle),
            ),
    )
}

/// A surface that lights up while its button is hovered (`group` /
/// `group_hover`).
pub(crate) fn group_hover(card: Card, ctx: &mut Ctx) -> Card {
    let theme = ctx.theme;
    card.child(
        Flex::column()
            .gap(space::SM)
            .padding(Edges::all(space::MD))
            .group_hover("gallery-card")
            .dynamic_background(move |state| {
                let palette = theme.palette();
                let (fill, border) = if state.group_hovered {
                    (palette.surface_hover, palette.accent)
                } else {
                    (palette.surface_raised, palette.border)
                };
                SurfaceStyle::new(fill).border(border).radius(radius::MD)
            })
            .child(Text::small("Hover the button", theme))
            .child(Button::secondary("Hover me", theme).group("gallery-card")),
    )
}

/// Focus ring plus an "activated" dot, drawn over a focus-navigation button
/// (the components paint no focus / activation state of their own).
fn button_chrome(
    theme: &'static dyn Theme,
    index: u8,
    active: Rc<Cell<u8>>,
) -> impl Fn(&mut PaintContext, Rect, InteractState) {
    let ring = theme.palette().focus_ring;
    let dot = theme.palette().success;
    move |ctx, rect, state| {
        if state.focused {
            igui_ui::surface(
                ctx,
                rect,
                &SurfaceStyle::new(Color::TRANSPARENT)
                    .border(ring)
                    .border_width(2.0)
                    .radius(radius::MD),
            );
        }
        if active.get() == index {
            ctx.fill_circle(Vec2::new(rect.right() - 6.0, rect.top() + 6.0), 3.0, dot);
        }
    }
}
