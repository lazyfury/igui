//! Shared layout, painting and hit-testing for editable text fields.
//!
//! [`TextInput`](super::TextInput) and [`TextArea`](super::TextArea) are the
//! same editor at two sizes: both own a [`TextEdit`] state handle, draw their
//! text in a foreground decorator (so the caret and selection share the exact
//! layout the text is drawn with), and route keys/text/IME through the
//! callbacks bound by [`bind`].
//!
//! This module is deliberately render-only: it never touches the tree. A
//! component supplies the tree-side wiring; everything here maps between
//! *bytes* (the [`TextEdit`] model) and *pixels* (the field rectangle).

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use draw_core::{Cursor, EventResult, FontWeight, ImeEvent, Key, Rect, Size, Vec2};
use draw_render::{PaintContext, TextAlign};
use draw_theme::{ControlSize, TextSize, Theme};
use draw_ui::{InteractState, PointerPhase, TextEdit, TextMeasurer};

use crate::base::Spec;

/// The shared, tree-independent state of one editable field.
#[derive(Clone)]
pub(crate) struct FieldState {
    /// The content + caret + selection + preedit.
    pub edit: Rc<RefCell<TextEdit>>,
    /// Vertical scroll offset (multiline only), in logical pixels.
    pub scroll: Rc<Cell<f32>>,
    /// The field's rectangle from the last paint, so callbacks that run before
    /// a layout (pointer, caret provider) still know where the field is.
    pub last_rect: Rc<Cell<Rect>>,
    /// Whether a pointer drag (text selection) is in progress.
    pub dragging: Rc<Cell<bool>>,
}

impl FieldState {
    pub(crate) fn new(text: impl Into<String>, multiline: bool) -> Self {
        Self {
            edit: Rc::new(RefCell::new(TextEdit::new(text).multiline(multiline))),
            scroll: Rc::new(Cell::new(0.0)),
            last_rect: Rc::new(Cell::new(Rect::ZERO)),
            dragging: Rc::new(Cell::new(false)),
        }
    }
}

/// Resolved text metrics for one field.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub font_size: f32,
    pub weight: FontWeight,
    pub line_height: f32,
    pub ascent: f32,
    pub pad_x: f32,
    pub pad_y: f32,
}

pub(crate) fn metrics(theme: &dyn Theme, measurer: &dyn TextMeasurer) -> Metrics {
    let font_size = theme.font_size(TextSize::Small);
    Metrics {
        font_size,
        weight: theme.font_weight(TextSize::Small),
        line_height: measurer.line_height(font_size),
        ascent: measurer.ascent(font_size),
        pad_x: theme.control_padding_x(),
        pad_y: theme.control_padding_y().max(2.0),
    }
}

/// A field's text as it is drawn: masked for passwords, with the active IME
/// preedit spliced in at the caret. All offsets are **byte** offsets into
/// `text`.
struct Display {
    text: String,
    /// Caret offset (the start of the preedit when one is active).
    caret: usize,
    /// Selection range, shifted past an inserted preedit.
    selection: Option<(usize, usize)>,
    /// The preedit range, underlined while composing.
    preedit: Option<(usize, usize)>,
}

fn display(edit: &TextEdit, masked: bool) -> Display {
    let committed = edit.text();
    let caret = edit.caret();
    if masked {
        let bullet = '\u{2022}';
        let text: String = committed.chars().map(|_| bullet).collect();
        let bullet_len = bullet.len_utf8();
        let selection = edit.selection().map(|(start, end)| {
            (
                committed[..start].chars().count() * bullet_len,
                committed[..end].chars().count() * bullet_len,
            )
        });
        return Display {
            text,
            caret: committed[..caret].chars().count() * bullet_len,
            selection,
            preedit: None,
        };
    }

    let mut text = String::with_capacity(committed.len() + 16);
    text.push_str(&committed[..caret]);
    let mut preedit = None;
    if let Some(active) = edit.preedit() {
        let start = text.len();
        text.push_str(&active.text);
        preedit = Some((start, text.len()));
    }
    text.push_str(&committed[caret..]);

    let inserted = preedit.map_or(0, |(start, end)| end - start);
    let shift = |byte: usize| {
        if byte <= caret {
            byte
        } else {
            byte + inserted
        }
    };
    Display {
        text,
        caret,
        selection: edit
            .selection()
            .map(|(start, end)| (shift(start), shift(end))),
        preedit,
    }
}

/// Splits `text` into painted lines as byte ranges (excluding the `\n` that
/// separates explicit lines). Single-line fields never wrap; multiline fields
/// break greedily at `char` boundaries that fit `width`.
pub(crate) fn wrap(
    measurer: &dyn TextMeasurer,
    text: &str,
    font_size: f32,
    weight: FontWeight,
    width: f32,
    multiline: bool,
) -> Vec<Range<usize>> {
    if !multiline || width <= 0.0 {
        return vec![0..text.len()];
    }
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut line_width = 0.0f32;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            lines.push(line_start..index);
            line_start = index + 1;
            line_width = 0.0;
            continue;
        }
        let advance = measurer.advance_weighted(ch, font_size, weight);
        if line_width + advance > width && index > line_start {
            lines.push(line_start..index);
            line_start = index;
            line_width = 0.0;
        }
        line_width += advance;
    }
    lines.push(line_start..text.len());
    lines
}

/// The index of the line containing `byte` (the first line whose end reaches
/// it, so a caret at a soft-wrap boundary stays at the end of the previous
/// line).
fn line_of(lines: &[Range<usize>], byte: usize) -> usize {
    lines
        .iter()
        .position(|line| byte <= line.end)
        .unwrap_or(lines.len().saturating_sub(1))
}

fn prefix_width(
    measurer: &dyn TextMeasurer,
    text: &str,
    line_start: usize,
    byte: usize,
    metrics: Metrics,
) -> f32 {
    let end = byte.clamp(line_start, text.len());
    if end <= line_start {
        return 0.0;
    }
    measurer.measure_run_weighted(&text[line_start..end], metrics.font_size, metrics.weight)
}

/// Where the content block starts (top-left of the first line's line box).
fn content_origin(rect: Rect, metrics: Metrics, multiline: bool, scroll: f32) -> Vec2 {
    if multiline {
        Vec2::new(
            rect.left() + metrics.pad_x,
            rect.top() + metrics.pad_y - scroll,
        )
    } else {
        Vec2::new(
            rect.left() + metrics.pad_x,
            rect.center().y - metrics.line_height / 2.0,
        )
    }
}

/// Paints the field: selection, text, preedit underline and caret.
pub(crate) fn paint(
    ctx: &mut PaintContext,
    rect: Rect,
    state: InteractState,
    theme: &dyn Theme,
    measurer: &dyn TextMeasurer,
    field: &FieldState,
    size: ControlSize,
    masked: bool,
    placeholder: &str,
    multiline: bool,
) {
    let _ = size;
    field.last_rect.set(rect);
    let edit = field.edit.borrow();
    let metrics = metrics(theme, measurer);
    let width = (rect.size.width - 2.0 * metrics.pad_x).max(0.0);
    let display = display(&edit, masked);
    let lines = wrap(
        measurer,
        &display.text,
        metrics.font_size,
        metrics.weight,
        width,
        multiline,
    );
    let scroll = if multiline { field.scroll.get() } else { 0.0 };
    let origin = content_origin(rect, metrics, multiline, scroll);

    let palette = theme.palette();

    // Placeholder (muted) when there is nothing to show yet.
    if display.text.is_empty() && display.preedit.is_none() && !placeholder.is_empty() {
        ctx.draw_text_weighted(
            placeholder,
            Vec2::new(origin.x, origin.y + metrics.ascent),
            metrics.font_size,
            metrics.weight,
            TextAlign::Left,
            palette.muted,
        );
    }

    // Selection highlight, clipped to each line.
    if let Some((sel_start, sel_end)) = display.selection {
        for (index, line) in lines.iter().enumerate() {
            let start = line.start.max(sel_start).min(line.end);
            let end = line.end.min(sel_end).max(start);
            if start >= end {
                continue;
            }
            let x0 = origin.x + prefix_width(measurer, &display.text, line.start, start, metrics);
            let x1 = origin.x + prefix_width(measurer, &display.text, line.start, end, metrics);
            let y = origin.y + index as f32 * metrics.line_height;
            ctx.fill_rect(
                Rect::from_min_size(
                    Vec2::new(x0, y),
                    Size::new((x1 - x0).max(1.0), metrics.line_height),
                ),
                palette.selection,
            );
        }
    }

    // The text itself, line by line.
    for (index, line) in lines.iter().enumerate() {
        if line.start >= line.end {
            continue;
        }
        let baseline = origin.y + index as f32 * metrics.line_height + metrics.ascent;
        ctx.draw_text_weighted(
            &display.text[line.clone()],
            Vec2::new(origin.x, baseline),
            metrics.font_size,
            metrics.weight,
            TextAlign::Left,
            palette.foreground,
        );
    }

    // Underline the IME preedit.
    if let Some((pre_start, pre_end)) = display.preedit {
        let index = line_of(&lines, pre_start);
        let line = &lines[index];
        let x0 = origin.x
            + prefix_width(
                measurer,
                &display.text,
                line.start,
                pre_start.max(line.start),
                metrics,
            );
        let x1 = origin.x
            + prefix_width(
                measurer,
                &display.text,
                line.start,
                pre_end.min(line.end).max(line.start),
                metrics,
            );
        let y = origin.y + (index as f32 + 1.0) * metrics.line_height - 1.5;
        ctx.fill_rect(
            Rect::from_min_size(Vec2::new(x0, y), Size::new((x1 - x0).max(1.0), 1.5)),
            palette.foreground,
        );
    }

    // Caret.
    if state.focused && display.preedit.is_none() {
        let index = line_of(&lines, display.caret);
        let line = &lines[index];
        let x =
            origin.x + prefix_width(measurer, &display.text, line.start, display.caret, metrics);
        let y = origin.y + index as f32 * metrics.line_height;
        ctx.fill_rect(
            Rect::from_min_size(Vec2::new(x, y), Size::new(1.5, metrics.line_height)),
            palette.accent,
        );
    }
}

/// The caret rectangle in the field's own coordinate space (for
/// `set_ime_cursor_area`).
pub(crate) fn caret_rect(
    theme: &dyn Theme,
    measurer: &dyn TextMeasurer,
    field: &FieldState,
    masked: bool,
    multiline: bool,
) -> Option<Rect> {
    let rect = field.last_rect.get();
    if rect.size.width <= 0.0 {
        return None;
    }
    let edit = field.edit.borrow();
    let metrics = metrics(theme, measurer);
    let width = (rect.size.width - 2.0 * metrics.pad_x).max(0.0);
    let display = display(&edit, masked);
    let lines = wrap(
        measurer,
        &display.text,
        metrics.font_size,
        metrics.weight,
        width,
        multiline,
    );
    let scroll = if multiline { field.scroll.get() } else { 0.0 };
    let origin = content_origin(rect, metrics, multiline, scroll);
    let index = line_of(&lines, display.caret);
    let line = &lines[index];
    let x = origin.x + prefix_width(measurer, &display.text, line.start, display.caret, metrics);
    let y = origin.y + index as f32 * metrics.line_height;
    Some(Rect::from_min_size(
        Vec2::new(x, y),
        Size::new(1.5, metrics.line_height),
    ))
}

/// The byte offset closest to `point` (in field coordinates).
pub(crate) fn caret_from_point(
    theme: &dyn Theme,
    measurer: &dyn TextMeasurer,
    field: &FieldState,
    rect: Rect,
    point: Vec2,
    masked: bool,
    multiline: bool,
) -> usize {
    let edit = field.edit.borrow();
    let metrics = metrics(theme, measurer);
    let width = (rect.size.width - 2.0 * metrics.pad_x).max(0.0);
    let display = display(&edit, masked);
    let lines = wrap(
        measurer,
        &display.text,
        metrics.font_size,
        metrics.weight,
        width,
        multiline,
    );
    let scroll = if multiline { field.scroll.get() } else { 0.0 };
    let origin = content_origin(rect, metrics, multiline, scroll);
    let row = if multiline {
        (((point.y - origin.y) / metrics.line_height).floor()).max(0.0) as usize
    } else {
        0
    };
    let index = row.min(lines.len().saturating_sub(1));
    let line = &lines[index];
    let local_x = (point.x - origin.x).max(0.0);
    let mut width_so_far = 0.0;
    let mut byte = line.start;
    for (offset, ch) in display.text[line.clone()].char_indices() {
        let advance = measurer.advance_weighted(ch, metrics.font_size, metrics.weight);
        if width_so_far + advance / 2.0 > local_x {
            break;
        }
        width_so_far += advance;
        byte = line.start + offset + ch.len_utf8();
    }
    // Translate a display offset back to a committed-text offset.
    if masked {
        let char_index = display.text[..byte].chars().count();
        return edit
            .text()
            .char_indices()
            .nth(char_index)
            .map_or(edit.text().len(), |(index, _)| index);
    }
    byte.min(edit.text().len())
}

/// Wires a [`Spec`] so the field behaves as an editor: text/caret painting,
/// focus, keys, committed text, IME and pointer-to-caret.
pub(crate) fn wire(
    spec: &mut Spec,
    theme: &'static dyn Theme,
    measurer: Rc<RefCell<Rc<dyn TextMeasurer>>>,
    field: FieldState,
    masked: bool,
    multiline: bool,
    placeholder: String,
) {
    spec.focusable = true;
    spec.data.clip = true;
    spec.cursor_provider = Some(Box::new(|| Cursor::Text));

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.foreground = Some(Box::new(move |ctx, rect, state| {
            // Resolve the measurer per paint: the host usually installs the
            // real font metrics *after* the tree was built.
            let measurer = measurer.borrow().clone();
            paint(
                ctx,
                rect,
                state,
                theme,
                measurer.as_ref(),
                &field,
                ControlSize::Regular,
                masked,
                &placeholder,
                multiline,
            );
        }));
    }

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.on_key = Some(Box::new(move |tree, key, pressed, modifiers| {
            if !pressed {
                return EventResult::Ignored;
            }
            let select = modifiers.shift;
            let word = modifiers.ctrl || modifiers.alt;
            let mut edit = field.edit.borrow_mut();
            let handled = match key {
                Key::Backspace => {
                    edit.backspace();
                    true
                }
                Key::Delete => {
                    edit.delete();
                    true
                }
                Key::ArrowLeft => {
                    edit.move_left(select, word);
                    true
                }
                Key::ArrowRight => {
                    edit.move_right(select, word);
                    true
                }
                Key::Home => {
                    edit.move_home(select);
                    true
                }
                Key::End => {
                    edit.move_end(select);
                    true
                }
                Key::ArrowUp if multiline => {
                    edit.move_up(select);
                    true
                }
                Key::ArrowDown if multiline => {
                    edit.move_down(select);
                    true
                }
                Key::Enter if multiline => {
                    edit.insert_newline();
                    true
                }
                Key::Space if !edit.has_preedit() => {
                    edit.insert(" ");
                    true
                }
                Key::Tab if !edit.has_preedit() => {
                    edit.insert("\t");
                    true
                }
                Key::Character('a') if modifiers.ctrl || modifiers.meta => {
                    edit.select_all();
                    true
                }
                Key::Character('c') if modifiers.ctrl || modifiers.meta => {
                    if let Some(selected) = edit.selected_text().map(str::to_string) {
                        if let Some(clipboard) = draw_ui::clipboard(tree) {
                            clipboard.borrow_mut().set(&selected);
                        }
                    }
                    true
                }
                Key::Character('x') if modifiers.ctrl || modifiers.meta => {
                    if let Some(selected) = edit.selected_text().map(str::to_string) {
                        if let Some(clipboard) = draw_ui::clipboard(tree) {
                            clipboard.borrow_mut().set(&selected);
                        }
                        edit.delete_selection();
                    }
                    true
                }
                Key::Character('v') if modifiers.ctrl || modifiers.meta => {
                    if let Some(clipboard) = draw_ui::clipboard(tree) {
                        if let Some(text) = clipboard.borrow().get() {
                            edit.commit_text(&text);
                        }
                    }
                    true
                }
                // Printable characters are committed through `TextInput`;
                // consume them here so a host shortcut cannot also fire while
                // the field is focused.
                Key::Character(_) => true,
                _ => false,
            };
            drop(edit);
            if handled {
                let measurer = measurer.borrow().clone();
                ensure_caret_visible(theme, measurer.as_ref(), &field, masked, multiline);
                draw_ui::request_paint(tree);
                EventResult::Handled
            } else {
                EventResult::Ignored
            }
        }));
    }

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.on_text = Some(Box::new(move |tree, text| {
            field.edit.borrow_mut().commit_text(text);
            let measurer = measurer.borrow().clone();
            ensure_caret_visible(theme, measurer.as_ref(), &field, masked, multiline);
            draw_ui::request_paint(tree);
        }));
    }

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.on_ime = Some(Box::new(move |tree, ime| {
            match ime {
                ImeEvent::Preedit { text, cursor } => {
                    field.edit.borrow_mut().set_preedit(text.clone(), *cursor)
                }
                ImeEvent::Commit(text) => field.edit.borrow_mut().commit_text(text),
                ImeEvent::Disabled => field.edit.borrow_mut().clear_preedit(),
                ImeEvent::Enabled => {}
            }
            let measurer = measurer.borrow().clone();
            ensure_caret_visible(theme, measurer.as_ref(), &field, masked, multiline);
            draw_ui::request_paint(tree);
        }));
    }

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.on_pointer_tree = Some(Box::new(move |tree, phase, rect, point| {
            match phase {
                PointerPhase::Down => {
                    let measurer = measurer.borrow().clone();
                    let byte = caret_from_point(
                        theme,
                        measurer.as_ref(),
                        &field,
                        rect,
                        point,
                        masked,
                        multiline,
                    );
                    field.edit.borrow_mut().set_caret(byte);
                    field.dragging.set(true);
                }
                PointerPhase::Move => {
                    if !field.dragging.get() {
                        return;
                    }
                    let measurer = measurer.borrow().clone();
                    drag_scroll(theme, measurer.as_ref(), &field, rect, point, multiline);
                    let byte = caret_from_point(
                        theme,
                        measurer.as_ref(),
                        &field,
                        rect,
                        point,
                        masked,
                        multiline,
                    );
                    field.edit.borrow_mut().extend_to(byte);
                }
                PointerPhase::Up => field.dragging.set(false),
                PointerPhase::DoubleClick => {
                    let measurer = measurer.borrow().clone();
                    let byte = caret_from_point(
                        theme,
                        measurer.as_ref(),
                        &field,
                        rect,
                        point,
                        masked,
                        multiline,
                    );
                    field.edit.borrow_mut().select_word_at(byte);
                    field.dragging.set(false);
                }
            }
            let measurer = measurer.borrow().clone();
            ensure_caret_visible(theme, measurer.as_ref(), &field, masked, multiline);
            draw_ui::request_paint(tree);
        }));
    }

    {
        let theme = theme;
        let measurer = measurer.clone();
        let field = field.clone();
        spec.caret_provider = Some(Box::new(move || {
            let measurer = measurer.borrow().clone();
            caret_rect(theme, measurer.as_ref(), &field, masked, multiline)
        }));
    }
}

/// While dragging a selection, scrolls a multiline field when the pointer goes
/// above or below its content area, so the selection can extend past the view.
fn drag_scroll(
    theme: &dyn Theme,
    measurer: &dyn TextMeasurer,
    field: &FieldState,
    rect: Rect,
    point: Vec2,
    multiline: bool,
) {
    if !multiline {
        return;
    }
    let metrics = metrics(theme, measurer);
    let top = rect.top() + metrics.pad_y;
    let bottom = rect.top() + rect.size.height - metrics.pad_y;
    let scroll = field.scroll.get();
    let next = if point.y < top {
        scroll - metrics.line_height
    } else if point.y > bottom {
        scroll + metrics.line_height
    } else {
        return;
    };
    field.scroll.set(next.max(0.0));
}

/// Scrolls a multiline field so the caret's line is inside the viewport.
pub(crate) fn ensure_caret_visible(
    theme: &dyn Theme,
    measurer: &dyn TextMeasurer,
    field: &FieldState,
    masked: bool,
    multiline: bool,
) {
    if !multiline {
        return;
    }
    let rect = field.last_rect.get();
    if rect.size.height <= 0.0 {
        return;
    }
    let edit = field.edit.borrow();
    let metrics = metrics(theme, measurer);
    let width = (rect.size.width - 2.0 * metrics.pad_x).max(0.0);
    let display = display(&edit, masked);
    let lines = wrap(
        measurer,
        &display.text,
        metrics.font_size,
        metrics.weight,
        width,
        multiline,
    );
    let index = line_of(&lines, display.caret);
    let line_top = index as f32 * metrics.line_height;
    let view_height = (rect.size.height - 2.0 * metrics.pad_y).max(metrics.line_height);
    let scroll = field.scroll.get();
    let next = if line_top < scroll {
        line_top
    } else if line_top + metrics.line_height > scroll + view_height {
        line_top + metrics.line_height - view_height
    } else {
        scroll
    };
    field.scroll.set(next.max(0.0));
}
