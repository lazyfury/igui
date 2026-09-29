//! The one callback shape every node-triggered behaviour uses.
//!
//! A node behaviour is a **tree-aware closure**: it receives the owning tree,
//! the node it was dispatched for (Godot's `self`), and its event argument.
//! Callbacks are shared handles (`Rc<RefCell<…>>`) so a dispatcher can clone the
//! handle out of the node, release the node borrow, and then call it with
//! `&mut SceneTree` — so a callback may freely mutate the tree (spawn, remove,
//! move nodes). `Box<dyn FnMut>` is used instead where the container is
//! host-owned and never borrowed from the tree (`Timers` / `Areas` / `Signal`);
//! the signature stays the same. See `docs/viewport-model.md` §5.
//!
//! Dispatch snapshot rule: a node removed before its turn is skipped, a node
//! added during a dispatch runs next frame, and a nested
//! `process`/`physics_process`/`handle_input` is a debug-asserted error.

use std::cell::RefCell;
use std::rc::Rc;

use igui_core::{EventResult, InputEvent, NodeId};

use crate::SceneTree;

/// A per-frame lifecycle callback (Godot `_process` / `_physics_process`).
pub type LifecycleCallback = Rc<RefCell<dyn FnMut(&mut SceneTree, NodeId, f32)>>;

/// An input-phase callback (Godot `_input` / `_input_event` /
/// `_unhandled_input`).
pub type InputCallback = Rc<RefCell<dyn FnMut(&mut SceneTree, NodeId, &InputEvent) -> EventResult>>;
