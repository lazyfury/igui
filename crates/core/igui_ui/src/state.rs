//! Interaction state resolved per control, shared by content and chrome.

/// Hover/pressed/focused state of a control, resolved against its ancestors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractState {
    /// The control is hovered.
    pub hovered: bool,
    /// The control is held down.
    pub pressed: bool,
    /// The control (or a descendant) has keyboard focus.
    pub focused: bool,
    /// The control (or an ancestor) is disabled, so hover / press are ignored.
    pub disabled: bool,
    /// The group this control reacts to (declared with `Component::group_hover`)
    /// is currently hovered. `false` when the control declares no group.
    pub group_hovered: bool,
}
