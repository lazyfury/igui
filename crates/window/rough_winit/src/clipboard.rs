//! The system clipboard, with an in-process fallback.
//!
//! `arboard` may fail to open a clipboard (headless CI, a sandboxed process,
//! a platform without one); the wrapper then keeps copy / paste working within
//! the process instead of dropping the operation.

use std::cell::RefCell;
use std::rc::Rc;

use cobbled_app::{AppBuilder, Plugin};
use cobbled_ui::{Clipboard, MemoryClipboard};

/// Installs a [`Clipboard`] service (the system clipboard with a fallback) so a
/// UI can copy / cut / paste.
#[derive(Default)]
pub struct ClipboardPlugin;

impl Plugin for ClipboardPlugin {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn build(&self, app: &mut AppBuilder) {
        let clipboard: Rc<RefCell<dyn Clipboard>> = Rc::new(RefCell::new(SystemClipboard::new()));
        app.insert_service(clipboard);
    }
}

/// A [`Clipboard`] backed by the OS clipboard when available.
pub struct SystemClipboard {
    /// `arboard` needs `&mut` even to read, so the handle is behind a `RefCell`
    /// and [`Clipboard::get`] stays `&self`.
    inner: RefCell<Option<arboard::Clipboard>>,
    fallback: MemoryClipboard,
}

impl SystemClipboard {
    /// Opens the system clipboard; on failure only the in-process fallback is
    /// used.
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(arboard::Clipboard::new().ok()),
            fallback: MemoryClipboard::default(),
        }
    }

    /// Whether the OS clipboard is available (otherwise the fallback is used).
    pub fn is_system(&self) -> bool {
        self.inner.borrow().is_some()
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for SystemClipboard {
    fn get(&self) -> Option<String> {
        if let Some(clipboard) = self.inner.borrow_mut().as_mut() {
            if let Ok(text) = clipboard.get_text() {
                return Some(text);
            }
        }
        self.fallback.get()
    }

    fn set(&mut self, text: &str) {
        // Always keep the fallback current, so a later `get` still works if the
        // system clipboard write fails.
        self.fallback.set(text);
        if let Some(clipboard) = self.inner.borrow_mut().as_mut() {
            let _ = clipboard.set_text(text.to_string());
        }
    }
}
