//! Components: node-building values composed with `.child()`.
//!
//! A component builds exactly one primary control node; nesting is expressed by
//! chaining [`Component::child`] or by attaching the component to the scene
//! with [`SceneTree::add_child`](igui_scene::SceneTree::add_child):
//!
//! ```ignore
//! let root = tree.add_child(tree.root(), Flex::column()
//!     .gap(12.0)
//!     .child(Label::new("Hello"))
//!     .child(Button::new("Save").on_click(|| { /* ... */ })));
//! ```
//!
//! Every component carries a [`Spec`] with the layout inputs, background,
//! foreground, click callback and children. The theme is never stored here: a
//! component receives the concrete colors it paints.

use std::cell::RefCell;
use std::rc::Rc;

use igui_core::{
    Color, Cursor, Edges, EventResult, ImeEvent, Key, Modifiers, NodeId, Rect, Size, Vec2,
};
use igui_render::PaintContext;
use igui_scene::SceneTree;
use igui_ui::layout::{FlexDirection, FlexStyle, GridStyle, SizeBasis, Track};
use igui_ui::{
    ButtonContent, Chrome, Container, ContentRef, Control, ControlData, DragPhase, FocusNav,
    InteractState, MouseFilter, PanelContent, SurfaceStyle, TextContent,
};

use crate::node_ref::{NodeRef, Ref};

/// A child builder stored on a [`Spec`].
pub type ChildFn = Box<dyn FnOnce(&mut SceneTree, NodeId)>;

/// A foreground painter: the paint context, the node rect and its interaction
/// state.
pub type ForegroundFn = Box<dyn Fn(&mut PaintContext, Rect, InteractState)>;

/// A drag callback: the tree, the drag phase and the pointer delta.
pub type DragFn = Box<dyn FnMut(&mut SceneTree, DragPhase, Vec2)>;

/// A key callback: the tree, the key, pressed/released, and the modifiers.
pub type KeyFn = Box<dyn FnMut(&mut SceneTree, Key, bool, Modifiers) -> EventResult>;

/// A committed-text callback (typing / IME commit).
pub type TextFn = Box<dyn FnMut(&mut SceneTree, &str)>;

/// An IME composition callback.
pub type ImeFn = Box<dyn FnMut(&mut SceneTree, &ImeEvent)>;

/// A caret-rectangle provider (for platform IME window placement).
pub type CaretFn = Box<dyn Fn() -> Option<Rect>>;

/// The common node state every component carries.
///
/// Layout fields mirror [`ControlData`]; the rest are the decorators, click
/// callback and children applied when the component is built.
pub struct Spec {
    pub data: ControlData,
    pub background: Option<Box<dyn Fn(InteractState) -> SurfaceStyle>>,
    pub foreground: Option<ForegroundFn>,
    pub on_click: Option<Box<dyn FnMut()>>,
    /// Secondary (right) click callback: the pointer position (context menus).
    pub on_secondary: Option<Box<dyn FnMut(Vec2)>>,
    pub on_drag: Option<DragFn>,
    /// Absolute-position pointer callback: press + move while held, with the
    /// control's rect and the pointer position (sliders / pickers).
    pub on_pointer: Option<Box<dyn FnMut(Rect, Vec2)>>,
    /// Tree-aware absolute-position pointer callback (text fields: place the
    /// caret and repaint). Dispatched before [`Spec::on_pointer`].
    pub on_pointer_tree: Option<Box<dyn FnMut(&mut SceneTree, igui_ui::PointerPhase, Rect, Vec2)>>,
    pub on_scroll: Option<Box<dyn FnMut(Vec2)>>,
    pub cursor_provider: Option<Box<dyn Fn() -> Cursor>>,
    /// Whether the control accepts focused key / text / IME input.
    pub focusable: bool,
    /// Whether the control takes keyboard focus as soon as it is mounted.
    pub focus_on_mount: bool,
    pub on_key: Option<KeyFn>,
    pub on_text: Option<TextFn>,
    pub on_ime: Option<ImeFn>,
    pub caret_provider: Option<CaretFn>,
    /// Keyboard-focus wiring (named directional neighbors + tab index). See
    /// [`Component::focus_neighbor_up`] and friends.
    pub focus: FocusNav,
    /// Name this control's hover drives for group-hover styling.
    pub group: Option<String>,
    /// Name of the group this control reacts to while it is hovered.
    pub group_hover: Option<String>,
    pub children: Vec<ChildFn>,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            data: ControlData::fill_parent(),
            background: None,
            foreground: None,
            on_click: None,
            on_secondary: None,
            on_drag: None,
            on_pointer: None,
            on_pointer_tree: None,
            on_scroll: None,
            cursor_provider: None,
            focusable: false,
            focus_on_mount: false,
            on_key: None,
            on_text: None,
            on_ime: None,
            caret_provider: None,
            focus: FocusNav::default(),
            group: None,
            group_hover: None,
            children: Vec::new(),
        }
    }
}

impl Spec {
    /// A spec with leaf defaults (top-left anchors, so it sizes to contents).
    pub fn leaf() -> Self {
        Self {
            data: ControlData::default(),
            ..Self::default()
        }
    }

    /// Records `child` to be built under this spec's node at mount time.
    ///
    /// This is the `prepare(&mut self)`-friendly counterpart to
    /// [`Component::child`], which requires `self` by value.
    pub fn child<C: Component + 'static>(&mut self, child: C) {
        self.children.push(Box::new(move |tree, parent| {
            child.build(tree, parent);
        }));
    }

    /// Records several children (equivalent to repeated [`Spec::child`]).
    pub fn children<I, C>(&mut self, children: I)
    where
        I: IntoIterator<Item = C>,
        C: Component + 'static,
    {
        for child in children {
            self.child(child);
        }
    }

    /// Composes this spec's `background` / `foreground` chrome around `inner`,
    /// returning the single [`ControlContent`] stored on the control.
    ///
    /// A control has one self-draw value, so the themed surface and foreground
    /// are wrapped around the component's own content here instead of being a
    /// second, separately-stored layer.
    pub fn chrome(&mut self, inner: Option<ContentRef>) -> Option<ContentRef> {
        let background = self.background.take();
        let foreground = self.foreground.take();
        if background.is_none() && foreground.is_none() {
            return inner;
        }
        let mut chrome = Chrome::new(inner);
        if let Some(resolve) = background {
            chrome = chrome.background(resolve);
        }
        if let Some(draw) = foreground {
            chrome = chrome.foreground(move |env, rect, state| draw(env.ctx, rect, state));
        }
        Some(Box::new(chrome))
    }
}

/// A value that builds one primary control node into a [`SceneTree`].
///
/// Implementors embed a [`Spec`], return it from [`Component::spec`], and
/// describe their visual with [`Component::container`] / [`Component::content`].
/// The default [`build`] creates the node and applies the spec, so components
/// compose natively with `.child()`, `.background()`, `.grow()` and friends.
///
/// [`build`]: Component::build
pub trait Component: Sized {
    /// The component's common node state.
    fn spec(&mut self) -> &mut Spec;

    /// Node name shown in debug overlays.
    fn name(&self) -> &'static str {
        "Control"
    }

    /// How this component lays out its children. The default is a leaf, which
    /// resolves children with the anchor/offset model.
    fn container(&self) -> Container {
        Container::Leaf
    }

    /// The component's own intrinsic size and self-draw. The default is `None`:
    /// the control paints nothing on its own and takes no intrinsic size, which
    /// is what a pure container or a chrome-only surface wants.
    fn content(&self) -> Option<ContentRef> {
        None
    }

    /// Runs before [`prepare`](Component::prepare) with the mounting tree, so a
    /// component can capture tree-scoped services (e.g. the tree's
    /// [`TextMeasurer`](igui_ui::TextMeasurer)) into the decorators and
    /// callbacks it is about to build. The default does nothing.
    fn bind(&mut self, _tree: &mut SceneTree) {}

    /// Finalizes the spec from the component's fields (surface styles, child
    /// closures) after every builder method has run.
    ///
    /// The default does nothing. Override it when a decorator or an internal
    /// child depends on more than one builder value.
    fn prepare(&mut self) {}

    /// Builds this component's node under `parent` and returns its id.
    ///
    /// The default runs [`prepare`](Component::prepare), creates a `Control`
    /// node, installs its container and self-draw content, and applies the spec
    /// (layout, decorators, click callback, children).
    fn build(mut self, tree: &mut SceneTree, parent: NodeId) -> NodeId {
        self.bind(tree);
        self.prepare();
        let mut spec = std::mem::take(self.spec());
        let content = spec.chrome(self.content());
        let id = tree.add_control(parent, self.name());
        tree.set_data(id, Control::new(spec.data, self.container(), content));
        apply_spec(tree, id, spec);
        id
    }

    /// Adds one child component.
    fn child<C: Component + 'static>(mut self, child: C) -> Self {
        self.spec().children.push(Box::new(move |tree, parent| {
            child.build(tree, parent);
        }));
        self
    }

    /// Adds several child components.
    fn children<I, C>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Component + 'static,
    {
        for child in children {
            self = self.child(child);
        }
        self
    }

    /// Wraps `self` so its mounted `NodeId` is written to `slot`.
    ///
    /// This is the component equivalent of Godot holding a `Node*` from `new()`
    /// / React's `ref` callback: the slot exists before mount and is read after.
    /// It works identically through
    /// [`SceneTree::add_child`](igui_scene::SceneTree::add_child) and
    /// [`Component::child`], because both mount paths run [`build`]
    /// ([`Component::build`]).
    fn ref_(self, slot: &NodeRef) -> Ref<Self> {
        Ref::new(self, {
            let slot = slot.clone();
            move |id| slot.fill(id)
        })
    }

    /// Like [`ref_`](Component::ref_), but reports the mounted id to a callback.
    fn with_ref(self, on_mount: impl FnOnce(NodeId) + 'static) -> Ref<Self> {
        Ref::new(self, on_mount)
    }

    /// Paints a rounded surface behind the node.
    fn background(mut self, color: Color) -> Self {
        self = self.surface(SurfaceStyle::new(color));
        self
    }

    /// Paints an explicit surface style behind the node.
    fn surface(mut self, style: SurfaceStyle) -> Self {
        self.spec().background = Some(Box::new(move |_| style));
        self
    }

    /// A surface whose style is resolved from the interaction state each frame.
    fn dynamic_background(
        mut self,
        resolve: impl Fn(InteractState) -> SurfaceStyle + 'static,
    ) -> Self {
        self.spec().background = Some(Box::new(resolve));
        self
    }

    /// Paints arbitrary chrome in front of the node.
    fn foreground(
        mut self,
        draw: impl Fn(&mut PaintContext, Rect, InteractState) + 'static,
    ) -> Self {
        self.spec().foreground = Some(Box::new(draw));
        self
    }

    /// Runs `callback` when the node is clicked or activated.
    fn on_click(mut self, callback: impl FnMut() + 'static) -> Self {
        self.spec().on_click = Some(Box::new(callback));
        self
    }

    /// Runs `callback` on a secondary (right) click, with the pointer position
    /// in viewport coordinates (used to open a context menu at the cursor).
    fn on_secondary_click(mut self, callback: impl FnMut(Vec2) + 'static) -> Self {
        self.spec().on_secondary = Some(Box::new(callback));
        self
    }

    /// Runs `callback` on drag start/move/end while the node is held, with the
    /// delta since the previous event. Gives the node pointer capture.
    fn on_drag(mut self, callback: impl FnMut(&mut SceneTree, DragPhase, Vec2) + 'static) -> Self {
        self.spec().on_drag = Some(Box::new(callback));
        self
    }

    /// Runs `callback(rect, pointer)` on press and on every move while the
    /// pointer is held on this node, so a component can map the pointer onto
    /// its own rectangle (sliders, colour pickers).
    fn on_pointer(mut self, callback: impl FnMut(Rect, Vec2) + 'static) -> Self {
        self.spec().on_pointer = Some(Box::new(callback));
        self
    }

    /// Like [`on_pointer`](Component::on_pointer), but the callback also
    /// receives the tree, so it can mark the UI for repaint after the edit
    /// (text fields placing their caret).
    fn on_pointer_tree(
        mut self,
        callback: impl FnMut(&mut SceneTree, igui_ui::PointerPhase, Rect, Vec2) + 'static,
    ) -> Self {
        self.spec().on_pointer_tree = Some(Box::new(callback));
        self
    }

    /// Sends keys to this component while it is focused.
    fn on_key(
        mut self,
        callback: impl FnMut(&mut SceneTree, Key, bool, Modifiers) -> EventResult + 'static,
    ) -> Self {
        self.spec().on_key = Some(Box::new(callback));
        self
    }

    /// Sends committed text (typing / IME commit) to this component while it is
    /// focused.
    fn on_text(mut self, callback: impl FnMut(&mut SceneTree, &str) + 'static) -> Self {
        self.spec().on_text = Some(Box::new(callback));
        self
    }

    /// Sends IME composition events to this component while it is focused.
    fn on_ime(mut self, callback: impl FnMut(&mut SceneTree, &ImeEvent) + 'static) -> Self {
        self.spec().on_ime = Some(Box::new(callback));
        self
    }

    /// Reports the caret rectangle so a host can place the platform IME window.
    fn caret_rect(mut self, provider: impl Fn() -> Option<Rect> + 'static) -> Self {
        self.spec().caret_provider = Some(Box::new(provider));
        self
    }

    /// Lets this component receive focused key / text / IME input.
    fn focusable(mut self, focusable: bool) -> Self {
        self.spec().focusable = focusable;
        self
    }

    /// Takes keyboard focus as soon as the node is mounted.
    ///
    /// The focus is written into the mounting tree's `GuiState`, so it works for
    /// the window tree and for an overlay's own tree alike (both mount through
    /// [`apply_spec`]). A deeper [`autofocus`](Self::autofocus) child wins,
    /// because focus is applied before the children are built.
    fn autofocus(mut self, autofocus: bool) -> Self {
        self.spec().focus_on_mount = autofocus;
        self
    }

    /// Names this component for explicit focus neighbors.
    ///
    /// Another component reaches it with [`focus_neighbor_up`](Self::focus_neighbor_up)
    /// and friends; the name need not match the node's scene name.
    fn focus_name(mut self, name: impl Into<String>) -> Self {
        self.spec().focus.name = Some(name.into());
        self
    }

    /// Names the component the up arrow moves to (overrides the spatial search).
    fn focus_neighbor_up(mut self, name: impl Into<String>) -> Self {
        self.spec().focus.up = Some(name.into());
        self
    }

    /// Names the component the down arrow moves to (overrides the spatial search).
    fn focus_neighbor_down(mut self, name: impl Into<String>) -> Self {
        self.spec().focus.down = Some(name.into());
        self
    }

    /// Names the component the left arrow moves to (overrides the spatial search).
    fn focus_neighbor_left(mut self, name: impl Into<String>) -> Self {
        self.spec().focus.left = Some(name.into());
        self
    }

    /// Names the component the right arrow moves to (overrides the spatial search).
    fn focus_neighbor_right(mut self, name: impl Into<String>) -> Self {
        self.spec().focus.right = Some(name.into());
        self
    }

    /// Overrides this component's Tab order (lower sorts first; components
    /// without a value follow, in tree order).
    fn tab_index(mut self, index: i32) -> Self {
        self.spec().focus.tab_index = Some(index);
        self
    }

    /// Drives the named group's hover state from this component's hover.
    ///
    /// A control declaring [`group_hover`](Self::group_hover) with the same
    /// name lights up while this component (or one of its descendants) is
    /// hovered.
    fn group(mut self, name: impl Into<String>) -> Self {
        self.spec().group = Some(name.into());
        self
    }

    /// Reacts to another component's hover through a named group: the
    /// [`InteractState`] this component's background / foreground sees reports
    /// [`group_hovered`](InteractState::group_hovered).
    fn group_hover(mut self, name: impl Into<String>) -> Self {
        self.spec().group_hover = Some(name.into());
        self
    }

    /// Runs `callback` when a wheel event lands on this node or one of its
    /// descendants, with the scroll delta in logical pixels.
    ///
    /// The nearest ancestor with a scroll callback owns the event, so a list
    /// can scroll itself and everything else stays unhandled.
    fn on_scroll(mut self, callback: impl FnMut(Vec2) + 'static) -> Self {
        self.spec().on_scroll = Some(Box::new(callback));
        self
    }

    /// Clips this component's subtree to its own rectangle.
    fn clip(mut self, clip: bool) -> Self {
        self.spec().data.clip = clip;
        self
    }

    /// A cursor resolved from the component's own state each frame while
    /// hovered (overrides [`Component::cursor`] when non-default).
    fn dynamic_cursor(mut self, cursor: impl Fn() -> Cursor + 'static) -> Self {
        self.spec().cursor_provider = Some(Box::new(cursor));
        self
    }

    /// Flex grow factor.
    fn grow(mut self, grow: f32) -> Self {
        self.spec().data.layout.grow = grow;
        self
    }

    /// Flex shrink factor.
    fn shrink(mut self, shrink: f32) -> Self {
        self.spec().data.layout.shrink = shrink;
        self
    }

    /// Flex basis.
    fn basis(mut self, basis: SizeBasis) -> Self {
        self.spec().data.layout.basis = basis;
        self
    }

    /// Minimum intrinsic size.
    fn min_size(mut self, width: f32, height: f32) -> Self {
        self.spec().data.min_size = Size::new(width, height);
        self
    }

    /// Layout order within the parent.
    fn order(mut self, order: i32) -> Self {
        self.spec().data.layout.order = order;
        self
    }

    /// Anchor edges (`0` = parent start, `1` = parent end).
    fn anchors(mut self, anchors: Edges) -> Self {
        self.spec().data.anchors = anchors;
        self
    }

    /// Offset edges, in the same order as [`Edges`].
    fn offsets(mut self, offsets: Edges) -> Self {
        self.spec().data.offsets = offsets;
        self
    }

    /// Pointer hit-test behaviour.
    fn mouse_filter(mut self, filter: MouseFilter) -> Self {
        self.spec().data.mouse_filter = filter;
        self
    }

    /// Cursor the host shows while the pointer is over the node.
    fn cursor(mut self, cursor: igui_core::Cursor) -> Self {
        self.spec().data.cursor = cursor;
        self
    }
}

/// Implements [`SceneChild`](igui_scene::SceneChild) for component types.
///
/// The trait lives in `igui_scene` (so `SceneTree::add_child` stays UI-neutral),
/// so each component type needs its own impl; the macro keeps that to one line
/// per type without introducing a forwarding layer.
#[macro_export]
macro_rules! impl_scene_child {
    ($($t:ty),* $(,)?) => {$(
        impl igui_scene::SceneChild for $t {
            fn attach(
                self,
                tree: &mut igui_scene::SceneTree,
                parent: igui_core::NodeId,
            ) -> igui_core::NodeId {
                <Self as $crate::Component>::build(self, tree, parent)
            }
        }
    )*};
}

impl_scene_child!(Panel, Label, Button, VBox, HBox, Flex, Column, Row, Grid);

/// Applies a spec to an already-created node (decorators, callback, children).
pub fn apply_spec(tree: &mut SceneTree, id: NodeId, spec: Spec) {
    if let Some(callback) = spec.on_click {
        set_on_click(tree, id, callback);
    }
    if let Some(callback) = spec.on_secondary {
        set_on_secondary(tree, id, callback);
    }
    if let Some(callback) = spec.on_drag {
        set_on_drag(tree, id, callback);
    }
    if let Some(callback) = spec.on_pointer {
        set_pointer_callback(tree, id, callback);
    }
    if let Some(callback) = spec.on_pointer_tree {
        igui_ui::set_pointer_tree_callback(tree, id, callback);
    }
    if let Some(callback) = spec.on_scroll {
        set_on_scroll(tree, id, callback);
    }
    if let Some(provider) = spec.cursor_provider {
        set_cursor_provider(tree, id, provider);
    }
    if spec.focusable {
        if let Some(control) = tree.data_mut::<Control>(id) {
            control.focusable = true;
        }
    }
    if !spec.focus.is_empty() {
        igui_ui::set_focus_nav(tree, id, spec.focus.clone());
    }
    if let Some(group) = spec.group {
        if let Some(control) = tree.data_mut::<Control>(id) {
            control.group = Some(group);
        }
    }
    if let Some(group) = spec.group_hover {
        if let Some(control) = tree.data_mut::<Control>(id) {
            control.group_hover = Some(group);
        }
    }
    if let Some(callback) = spec.on_key {
        igui_ui::set_key_callback(tree, id, callback);
    }
    if let Some(callback) = spec.on_text {
        igui_ui::set_text_callback(tree, id, callback);
    }
    if let Some(callback) = spec.on_ime {
        igui_ui::set_ime_callback(tree, id, callback);
    }
    if let Some(provider) = spec.caret_provider {
        igui_ui::set_caret_provider(tree, id, provider);
    }
    if spec.focus_on_mount {
        // Mount-time focus (a text field opened in a dialog). Applied before the
        // children so a deeper autofocus child wins.
        igui_ui::gui_state_mut(tree).focused = Some(id);
    }
    for child in spec.children {
        child(tree, id);
    }
}

mod containers;
mod primitives;

pub use containers::{Column, Flex, Grid, HBox, Row, VBox};
pub use primitives::{Button, Label, Panel};
mod setters;

pub use setters::*;
