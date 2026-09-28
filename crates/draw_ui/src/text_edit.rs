//! A dependency-free text-editing state machine.
//!
//! [`TextEdit`] owns the *content* of an editable field — the text, the caret
//! and selection as byte offsets, and the active IME preedit — but nothing
//! about layout or pixels. A component renders it by turning the byte indices
//! into pixel positions with the tree's [`TextMeasurer`](crate::TextMeasurer);
//! a host routes keys, committed text and IME events into it.
//!
//! Movement is on `char` boundaries (`prev_boundary` / `next_boundary`), which
//! is what a field needs; grapheme clusters (combining marks, emoji ZWJ
//! sequences) are handled as their individual `char`s for now. Word movement
//! treats Unicode alphanumerics as word characters, so CJK runs move as words.
//!
//! The IME preedit is *not* part of [`TextEdit::text`]: it is a transient
//! composition the platform updates until [`TextEdit::commit_text`] folds it
//! into the committed text. Rendering shows `text` + `preedit` concatenated.

/// An active input-method composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preedit {
    /// The in-progress composition text.
    pub text: String,
    /// Optional selection within `text` as `(start, end)` byte offsets.
    pub cursor: Option<(usize, usize)>,
}

/// The content state of one editable field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    text: String,
    /// Caret position, a byte index on a `char` boundary.
    caret: usize,
    /// Selection anchor: the other end of the selection. Equal to `caret` when
    /// there is no selection.
    anchor: usize,
    preedit: Option<Preedit>,
    multiline: bool,
}

impl TextEdit {
    /// A new field with an optional committed value.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        Self {
            text,
            caret,
            anchor: caret,
            preedit: None,
            multiline: false,
        }
    }

    /// Whether the field accepts newlines.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Whether the field accepts newlines.
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// The committed text (the preedit is not included).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the committed text; the caret moves to the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.caret = self.text.len();
        self.anchor = self.caret;
        self.preedit = None;
    }

    /// The field is empty of committed text (a preedit may still be active).
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    // -- caret / selection -------------------------------------------------

    /// The caret, a byte index.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selection anchor, a byte index.
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The selection as `(start, end)` with `start <= end`, or `None` when the
    /// caret and anchor coincide.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let (start, end) = (self.caret.min(self.anchor), self.caret.max(self.anchor));
        (start != end).then_some((start, end))
    }

    /// Whether a selection exists.
    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    /// Moves the caret to `byte`, clamping it to a `char` boundary, and clears
    /// the selection.
    pub fn set_caret(&mut self, byte: usize) {
        self.caret = floor_boundary(&self.text, byte);
        self.anchor = self.caret;
    }

    /// Selects the whole committed text.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }

    /// Collapses the selection to the caret end (no-op when there is none).
    pub fn clear_selection(&mut self) {
        self.anchor = self.caret;
    }

    // -- preedit (IME) -----------------------------------------------------

    /// The active preedit, if any.
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// Whether a composition is in progress.
    pub fn has_preedit(&self) -> bool {
        self.preedit.is_some()
    }

    /// Replaces the active preedit (an empty text clears it).
    pub fn set_preedit(&mut self, text: impl Into<String>, cursor: Option<(usize, usize)>) {
        let text = text.into();
        if text.is_empty() {
            self.preedit = None;
        } else {
            self.preedit = Some(Preedit { text, cursor });
        }
    }

    /// Drops the active preedit without inserting it.
    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    /// Commits `text`: the preedit clears, any selection is replaced, and the
    /// text is inserted at the caret.
    pub fn commit_text(&mut self, text: &str) {
        self.preedit = None;
        self.insert(text);
    }

    // -- operations --------------------------------------------------------

    /// Inserts `text` at the caret, replacing the selection and folding away
    /// any active preedit. Newlines are dropped in a single-line field.
    pub fn insert(&mut self, text: &str) {
        self.delete_selection();
        let text = if self.multiline {
            text.to_string()
        } else {
            text.replace(['\n', '\r'], "")
        };
        self.text.insert_str(self.caret, &text);
        self.caret += text.len();
        self.anchor = self.caret;
    }

    /// Inserts a line break (no-op in a single-line field).
    pub fn insert_newline(&mut self) {
        if self.multiline {
            self.insert("\n");
        }
    }

    /// Deletes the selection, returning whether anything was removed.
    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            return false;
        };
        self.text.replace_range(start..end, "");
        self.caret = start;
        self.anchor = start;
        true
    }

    /// Backspace: delete the selection, or the `char` before the caret (or
    /// clear the preedit first).
    pub fn backspace(&mut self) {
        if self.preedit.is_some() {
            self.preedit = None;
            return;
        }
        if self.delete_selection() {
            return;
        }
        if self.caret == 0 {
            return;
        }
        let previous = prev_boundary(&self.text, self.caret);
        self.text.replace_range(previous..self.caret, "");
        self.caret = previous;
        self.anchor = previous;
    }

    /// Forward delete: delete the selection, or the `char` after the caret.
    pub fn delete(&mut self) {
        if self.preedit.is_some() {
            self.preedit = None;
            return;
        }
        if self.delete_selection() {
            return;
        }
        if self.caret >= self.text.len() {
            return;
        }
        let next = next_boundary(&self.text, self.caret);
        self.text.replace_range(self.caret..next, "");
    }

    /// Move the caret left by one `char` (or one word with `word`).
    pub fn move_left(&mut self, select: bool, word: bool) {
        let target = if word {
            prev_word_boundary(&self.text, self.caret)
        } else {
            prev_boundary(&self.text, self.caret)
        };
        self.move_to(target, select);
    }

    /// Move the caret right by one `char` (or one word with `word`).
    pub fn move_right(&mut self, select: bool, word: bool) {
        let target = if word {
            next_word_boundary(&self.text, self.caret)
        } else {
            next_boundary(&self.text, self.caret)
        };
        self.move_to(target, select);
    }

    /// Move the caret to the start of the current visual line (logical line
    /// for a single-line field).
    pub fn move_home(&mut self, select: bool) {
        let target = line_start(&self.text, self.caret);
        self.move_to(target, select);
    }

    /// Move the caret to the end of the current line.
    pub fn move_end(&mut self, select: bool) {
        let target = line_end(&self.text, self.caret);
        self.move_to(target, select);
    }

    /// Move the caret up one logical line, keeping the column. No-op in a
    /// single-line field or on the first line.
    pub fn move_up(&mut self, select: bool) {
        if !self.multiline {
            return;
        }
        let start = line_start(&self.text, self.caret);
        if start == 0 {
            return;
        }
        let column = self.text[start..self.caret].chars().count();
        let prev_end = start - 1;
        let prev_start = line_start(&self.text, prev_end);
        let target = byte_at_column(&self.text, prev_start, prev_end, column);
        self.move_to(target, select);
    }

    /// Move the caret down one logical line, keeping the column.
    pub fn move_down(&mut self, select: bool) {
        if !self.multiline {
            return;
        }
        let end = line_end(&self.text, self.caret);
        if end >= self.text.len() {
            return;
        }
        let start = line_start(&self.text, self.caret);
        let column = self.text[start..self.caret].chars().count();
        let next_start = end + 1;
        let next_end = line_end(&self.text, next_start);
        let target = byte_at_column(&self.text, next_start, next_end, column);
        self.move_to(target, select);
    }

    /// The 0-based index of the logical line containing the caret.
    pub fn line_index(&self) -> usize {
        self.text[..self.caret].matches('\n').count()
    }

    /// The number of logical lines (`1` for empty text).
    pub fn line_count(&self) -> usize {
        self.text.split('\n').count()
    }

    fn move_to(&mut self, target: usize, select: bool) {
        self.caret = floor_boundary(&self.text, target);
        if !select {
            self.anchor = self.caret;
        }
    }
}

/// Clamps `byte` down to the nearest `char` boundary (a `String` slice
/// boundary).
fn floor_boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// The start of the `char` before `byte`.
fn prev_boundary(text: &str, byte: usize) -> usize {
    let byte = floor_boundary(text, byte);
    if byte == 0 {
        return 0;
    }
    let mut previous = byte - 1;
    while !text.is_char_boundary(previous) {
        previous -= 1;
    }
    previous
}

/// The start of the `char` after `byte`.
fn next_boundary(text: &str, byte: usize) -> usize {
    let byte = floor_boundary(text, byte);
    if byte >= text.len() {
        return text.len();
    }
    byte + text[byte..].chars().next().map_or(0, char::len_utf8)
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// The start of the word before `byte` (skipping separators first).
fn prev_word_boundary(text: &str, byte: usize) -> usize {
    let mut byte = floor_boundary(text, byte);
    while byte > 0 {
        let previous = prev_boundary(text, byte);
        let Some(ch) = text[previous..byte].chars().next() else {
            break;
        };
        if is_word_char(ch) {
            break;
        }
        byte = previous;
    }
    while byte > 0 {
        let previous = prev_boundary(text, byte);
        let Some(ch) = text[previous..byte].chars().next() else {
            break;
        };
        if !is_word_char(ch) {
            break;
        }
        byte = previous;
    }
    byte
}

/// The start of the word after `byte` (skipping separators first).
fn next_word_boundary(text: &str, byte: usize) -> usize {
    let mut byte = floor_boundary(text, byte);
    while byte < text.len() {
        let Some(ch) = text[byte..].chars().next() else {
            break;
        };
        if is_word_char(ch) {
            break;
        }
        byte = next_boundary(text, byte);
    }
    while byte < text.len() {
        let Some(ch) = text[byte..].chars().next() else {
            break;
        };
        if !is_word_char(ch) {
            break;
        }
        byte = next_boundary(text, byte);
    }
    byte
}

fn line_start(text: &str, byte: usize) -> usize {
    let byte = floor_boundary(text, byte);
    text[..byte].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(text: &str, byte: usize) -> usize {
    let byte = floor_boundary(text, byte);
    text[byte..]
        .find('\n')
        .map_or(text.len(), |index| byte + index)
}

/// The byte offset `column` chars into `[start, end)` (clamped to `end`).
fn byte_at_column(text: &str, start: usize, end: usize, column: usize) -> usize {
    let mut byte = start;
    let mut remaining = column;
    while remaining > 0 && byte < end {
        byte = next_boundary(text, byte);
        remaining -= 1;
    }
    byte.min(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserting_replaces_the_selection() {
        let mut edit = TextEdit::new("hello world");
        edit.set_caret(6); // before "w"
        edit.move_right(true, false); // select "w"
        assert_eq!(edit.selection(), Some((6, 7)));
        edit.insert("W");
        assert_eq!(edit.text(), "hello World");
        assert_eq!(edit.caret(), 7);
        assert!(!edit.has_selection());
    }

    #[test]
    fn movement_respects_char_boundaries() {
        let mut edit = TextEdit::new("你好a");
        edit.set_caret(0);
        edit.move_right(false, false);
        assert_eq!(edit.caret(), 3); // one CJK char is 3 bytes
        edit.move_left(false, false);
        assert_eq!(edit.caret(), 0);
    }

    #[test]
    fn word_movement_skips_separators() {
        let mut edit = TextEdit::new("foo bar  baz");
        edit.set_caret(0);
        edit.move_right(false, true);
        assert_eq!(&edit.text()[..edit.caret()], "foo");
        edit.move_right(false, true);
        assert_eq!(&edit.text()[..edit.caret()], "foo bar");
        edit.move_left(false, true);
        assert_eq!(&edit.text()[..edit.caret()], "foo ");
    }

    #[test]
    fn a_single_line_field_drops_newlines() {
        let mut edit = TextEdit::new("");
        edit.insert("a\nb");
        assert_eq!(edit.text(), "ab");
        edit.insert_newline();
        assert_eq!(edit.text(), "ab");
    }

    #[test]
    fn multiline_up_down_keeps_the_column() {
        let mut edit = TextEdit::new("hello\nhi\nworld").multiline(true);
        edit.set_caret(0);
        edit.move_right(false, false);
        edit.move_right(false, false);
        assert_eq!(&edit.text()[..edit.caret()], "he");
        edit.move_down(false);
        assert_eq!(&edit.text()[..edit.caret()], "hello\nhi");
        edit.move_down(false);
        assert_eq!(&edit.text()[..edit.caret()], "hello\nhi\nwo");
        edit.move_up(false);
        assert_eq!(&edit.text()[..edit.caret()], "hello\nhi");
    }

    #[test]
    fn backspace_and_delete_remove_one_char() {
        let mut edit = TextEdit::new("ab界");
        edit.set_caret(2); // before '界'
        edit.delete();
        assert_eq!(edit.text(), "ab");
        edit.backspace();
        assert_eq!(edit.text(), "a");
        edit.backspace();
        assert_eq!(edit.text(), "");
        edit.backspace(); // no-op at the start
        assert_eq!(edit.text(), "");
    }

    #[test]
    fn a_preedit_is_separate_until_committed() {
        let mut edit = TextEdit::new("");
        edit.set_preedit("ni", Some((2, 2)));
        assert_eq!(edit.text(), "");
        assert!(edit.has_preedit());
        edit.commit_text("你");
        assert_eq!(edit.text(), "你");
        assert!(!edit.has_preedit());
        assert_eq!(edit.caret(), 3);
    }

    #[test]
    fn select_all_then_insert_replaces_everything() {
        let mut edit = TextEdit::new("old");
        edit.select_all();
        assert_eq!(edit.selection(), Some((0, 3)));
        edit.insert("new");
        assert_eq!(edit.text(), "new");
    }

    #[test]
    fn line_helpers_find_logical_lines() {
        let mut edit = TextEdit::new("a\nbb\nc").multiline(true);
        edit.set_caret(4); // inside "bb"
        assert_eq!(edit.line_index(), 1);
        assert_eq!(edit.line_count(), 3);
        edit.move_home(false);
        assert_eq!(edit.caret(), 2);
        edit.move_end(false);
        assert_eq!(edit.caret(), 4);
    }
}
