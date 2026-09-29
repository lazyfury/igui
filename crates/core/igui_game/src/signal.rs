//! Lightweight typed signals: a one-to-many callback list.
//!
//! Not an ECS and not a node system — just a small value a game can emit from
//! (for example `on_died`, `on_score`) and listeners can subscribe to. Cloning a
//! signal shares its handler list, so a node can own one and hand clones around.
//!
//! Handlers are tree-aware (`FnMut(&mut SceneTree, &T)`), so a listener may
//! mutate the world. [`Signal::connect`] returns a [`Connection`] for a
//! single-handler [`Signal::disconnect`]. Each handler is a shared handle, so a
//! handler may `connect` / `disconnect` during an `emit` (the change takes
//! effect on the next emit) without a `RefCell` re-entry panic.

use std::cell::RefCell;
use std::rc::Rc;

use igui_scene::SceneTree;

/// A handle to one connected handler, for [`Signal::disconnect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Connection(u64);

type Handler<T> = Rc<RefCell<dyn FnMut(&mut SceneTree, &T)>>;

struct Inner<T> {
    handlers: Vec<(u64, Handler<T>)>,
    next_id: u64,
}

/// A typed, one-to-many callback list.
pub struct Signal<T> {
    inner: Rc<RefCell<Inner<T>>>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T: 'static> Default for Signal<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> Signal<T> {
    /// Creates an empty signal.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(Inner {
                handlers: Vec::new(),
                next_id: 1,
            })),
        }
    }

    /// Adds a handler, called in registration order on every [`Signal::emit`].
    ///
    /// Returns a [`Connection`] that removes just this handler. A handler may
    /// safely `connect` / `disconnect` while the signal is emitting; the change
    /// takes effect on the next emit.
    pub fn connect(&self, handler: impl FnMut(&mut SceneTree, &T) + 'static) -> Connection {
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_id;
        inner.next_id += 1;
        inner.handlers.push((id, Rc::new(RefCell::new(handler))));
        Connection(id)
    }

    /// Removes a single handler. Returns whether it was connected.
    pub fn disconnect(&self, connection: Connection) -> bool {
        let mut inner = self.inner.borrow_mut();
        let before = inner.handlers.len();
        inner.handlers.retain(|(id, _)| *id != connection.0);
        inner.handlers.len() != before
    }

    /// Calls every handler with `value`.
    ///
    /// The handler list is snapshotted first, so a handler may `connect` /
    /// `disconnect` during the emit (a handler added mid-emit runs next time; a
    /// handler disconnected mid-emit still runs this time).
    pub fn emit(&self, tree: &mut SceneTree, value: &T) {
        let handlers: Vec<Handler<T>> = self
            .inner
            .borrow()
            .handlers
            .iter()
            .map(|(_, handler)| handler.clone())
            .collect();
        for handler in handlers {
            (handler.borrow_mut())(tree, value);
        }
    }

    /// Number of connected handlers.
    pub fn handler_count(&self) -> usize {
        self.inner.borrow().handlers.len()
    }

    /// Removes every handler.
    pub fn disconnect_all(&self) {
        self.inner.borrow_mut().handlers.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use igui_scene::SceneTree;

    #[test]
    fn emit_calls_every_handler_in_order() {
        let mut tree = SceneTree::new();
        let signal = Signal::<i32>::new();
        let first = Rc::new(Cell::new(0));
        let second = Rc::new(Cell::new(0));
        let (a, b) = (first.clone(), second.clone());

        signal.connect(move |_tree, value| a.set(*value));
        signal.connect(move |_tree, value| b.set(b.get() + *value));
        assert_eq!(signal.handler_count(), 2);

        signal.emit(&mut tree, &7);
        assert_eq!(first.get(), 7);
        assert_eq!(second.get(), 7);
    }

    #[test]
    fn clones_share_the_same_handler_list() {
        let mut tree = SceneTree::new();
        let signal = Signal::<i32>::new();
        let clone = signal.clone();
        let hits = Rc::new(Cell::new(0));
        let counter = hits.clone();

        clone.connect(move |_tree, _| counter.set(counter.get() + 1));
        assert_eq!(signal.handler_count(), 1);
        signal.emit(&mut tree, &0);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn disconnect_all_clears_handlers() {
        let signal = Signal::<()>::new();
        signal.connect(|_tree, _| {});
        signal.disconnect_all();
        assert_eq!(signal.handler_count(), 0);
    }

    #[test]
    fn a_connection_disconnects_one_handler() {
        let mut tree = SceneTree::new();
        let signal = Signal::<i32>::new();
        let kept = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(0));
        let (a, b) = (kept.clone(), dropped.clone());

        let connection = signal.connect(move |_tree, value| b.set(*value));
        signal.connect(move |_tree, value| a.set(a.get() + *value));
        assert!(signal.disconnect(connection));

        signal.emit(&mut tree, &5);
        assert_eq!(kept.get(), 5);
        assert_eq!(dropped.get(), 0, "the disconnected handler did not run");
        assert!(!signal.disconnect(connection), "already gone");
    }

    #[test]
    fn a_handler_can_mutate_the_tree() {
        let mut tree = SceneTree::new();
        let root = tree.root();
        let signal = Signal::<()>::new();
        signal.connect(move |tree, _| {
            tree.add_node2d(root, "spawned");
        });
        signal.emit(&mut tree, &());
        assert_eq!(tree.children(root).map(|c| c.len()), Some(1));
    }
}
