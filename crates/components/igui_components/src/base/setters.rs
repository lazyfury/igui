use super::*;

/// Borrows a control's runtime from the node's extension slot.
pub fn control_mut(tree: &mut SceneTree, id: NodeId) -> Option<&mut Control> {
    tree.data_mut::<Control>(id)
}

/// Registers a click callback on `id`.
pub fn set_on_click<F>(tree: &mut SceneTree, id: NodeId, callback: F) -> bool
where
    F: FnMut(&mut SceneTree, NodeId) + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.callback = Some(Rc::new(RefCell::new(callback)));
            true
        }
        None => false,
    }
}

/// Registers a secondary (right) click callback on `id`.
pub fn set_on_secondary<F>(tree: &mut SceneTree, id: NodeId, callback: F) -> bool
where
    F: FnMut(&mut SceneTree, NodeId, Vec2) + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.secondary_callback = Some(Rc::new(RefCell::new(callback)));
            true
        }
        None => false,
    }
}

/// Registers a pointer-drag callback on `id` (pointer capture while held).
pub fn set_on_drag<F>(tree: &mut SceneTree, id: NodeId, callback: F) -> bool
where
    F: FnMut(&mut SceneTree, NodeId, DragPhase, Vec2) + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.drag_callback = Some(Rc::new(RefCell::new(callback)));
            true
        }
        None => false,
    }
}

/// Registers an absolute-position pointer callback on `id`.
pub fn set_pointer_callback<F>(tree: &mut SceneTree, id: NodeId, callback: F) -> bool
where
    F: FnMut(&mut SceneTree, NodeId, igui_ui::PointerPhase, Rect, Vec2) + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.pointer_callback = Some(Rc::new(RefCell::new(callback)));
            true
        }
        None => false,
    }
}

/// Registers a wheel callback on `id`, so it (and its subtree) owns scrolling.
pub fn set_on_scroll<F>(tree: &mut SceneTree, id: NodeId, callback: F) -> bool
where
    F: FnMut(&mut SceneTree, NodeId, Vec2) + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.scroll_callback = Some(Rc::new(RefCell::new(callback)));
            true
        }
        None => false,
    }
}

/// Registers a dynamic cursor provider on `id`, evaluated while it is hovered.
pub fn set_cursor_provider<F>(tree: &mut SceneTree, id: NodeId, provider: F) -> bool
where
    F: Fn() -> Cursor + 'static,
{
    match tree.data_mut::<Control>(id) {
        Some(control) => {
            control.cursor_provider = Some(Rc::new(provider));
            true
        }
        None => false,
    }
}

/// Replaces a control's text, marking layout dirty only when it changed.
pub fn set_text(tree: &mut SceneTree, id: NodeId, text: impl Into<String>) -> bool {
    let text = text.into();
    let changed = match tree.data_mut::<Control>(id) {
        Some(control) => control
            .content
            .as_mut()
            .is_some_and(|content| content.set_text(&text)),
        None => return false,
    };
    if changed {
        igui_ui::mark_dirty(tree, id);
    }
    true
}

/// Replaces a text control's color (no-op on content without a color).
///
/// Color does not affect layout, so the tree is not marked dirty: the paint
/// stage reads the content's color every frame.
pub fn set_text_color(tree: &mut SceneTree, id: NodeId, color: Color) -> bool {
    let Some(control) = tree.data_mut::<Control>(id) else {
        return false;
    };
    if let Some(content) = control.content.as_mut() {
        content.set_color(color);
    }
    true
}

/// Mutates a control's layout data and marks the tree dirty.
pub fn update_control(tree: &mut SceneTree, id: NodeId, f: impl FnOnce(&mut ControlData)) -> bool {
    let changed = match tree.data_mut::<Control>(id) {
        Some(control) => {
            f(&mut control.data);
            true
        }
        None => false,
    };
    if changed {
        igui_ui::mark_dirty(tree, id);
    }
    changed
}
