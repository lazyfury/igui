# Start here

From zero to a running, good-looking UI — then a map of where to go next. Every
code block below compiles as written; the ones that only make sense as a
fragment say so.

**Pick your entry point**

| You are | Read | Then |
|---|---|---|
| building an app | §1 → §2 → §3 | [`ui-guide.md`](ui-guide.md), [`components.md`](components.md) |
| an agent asked to "make a UI" | §1 → §3 → §6 | [`ui-guide.md`](ui-guide.md) |
| writing a backend | §2 → [`backend.md`](backend.md) | [`../internals/architecture.md`](../internals/architecture.md) |
| reviewing / contributing | [`../internals/architecture.md`](../internals/architecture.md) | [`../internals/testing.md`](../internals/testing.md) |

---

## 1 · Thirty seconds

igui is a **backend-neutral 2D/UI core**: you build a persistent scene tree, and
each frame you lay it out, paint it into a backend-neutral `DrawList`, and hand
that list to a backend.

```
Input -> SceneTree -> Update -> Layout -> Paint -> DrawList -> RenderBackend -> Pixels
```

```bash
cargo run -p wgpu_demo --release   # the gallery, native (wgpu + winit)
cargo test  --workspace            # the whole suite, no window, no browser
cargo run   -p wgpu_demo -- --selfcheck   # renders headlessly, asserts layouts
```

Nothing in the UI layer knows about `wgpu`, the DOM or a window: the same
`DrawList` goes to `igui_backend_wgpu`, `igui_backend_canvas` in a browser, or
`igui_backend_recording` in a test. That is why everything here is verifiable
with plain `cargo test`.

**Three things to know before you type anything:**

1. A `Control` is a rectangle with a layout `Container` and optional self-drawn
   content. Components (`Card`, `Text`, `Button`, …) are values that build one.
2. The theme is a `&'static dyn Theme` you pass to constructors. It is never
   stored in the tree, and colours/spacing always come from it.
3. Imports are not flattened: `igui_components::Card`, or
   `igui::igui_components::Card` through the facade.

---

## 2 · The smallest app that runs

This is a complete program: it builds a small settings page, lays it out, paints
one frame, and renders it through the recording backend (no GPU, no window).

```rust
use std::rc::Rc;

use igui_backend_recording::RecordingBackend;
use igui_components::{Button, Card, Column, Component, Row, Text};
use igui_core::{Edges, FontWeight, Size, ViewportSize};
use igui_font::{FontConfig, FontMetrics, FontServer};
use igui_render::{PaintContext, RenderBackend};
use igui_scene::SceneTree;
use igui_theme::{default_theme, Mode, Space, Theme, Tone};
use igui_ui::TextMeasurer;

/// Layout must measure with the font the backend paints with; the default
/// `ApproxTextMeasurer` guesses widths and leaves text visibly off-centre.
struct FontMeasurer(FontMetrics);

impl TextMeasurer for FontMeasurer {
    fn advance(&self, ch: char, font_size: f32) -> f32 {
        self.0.advance(ch, font_size)
    }
    fn advance_weighted(&self, ch: char, font_size: f32, weight: FontWeight) -> f32 {
        self.0.advance_weighted(ch, font_size, weight)
    }
    fn line_height(&self, font_size: f32) -> f32 {
        self.0.line_height(font_size)
    }
    fn ascent(&self, font_size: f32) -> f32 {
        self.0.ascent(font_size)
    }
    fn measure_run(&self, text: &str, font_size: f32) -> f32 {
        self.0.measure_run(text, font_size)
    }
    fn measure_run_weighted(&self, text: &str, font_size: f32, weight: FontWeight) -> f32 {
        self.0.measure_run_weighted(text, font_size, weight)
    }
}

fn build(theme: &'static dyn Theme) -> SceneTree {
    let mut tree = SceneTree::new();

    // 1. Install the measurer *before* the first layout.
    let server = Rc::new(FontServer::load_with(FontConfig::default()));
    igui_ui::set_text_measurer(&mut tree, Rc::new(FontMeasurer(FontMetrics::new(server))));

    // 2. Build the view. Every number comes from the theme, never from a literal.
    let page = tree.add_child(
        tree.root(),
        Column::new()
            .gap(theme.spacing(Space::LG))
            .padding(Edges::all(theme.spacing(Space::XXL))),
    );
    tree.add_child(page, Text::heading("Settings", theme));
    tree.add_child(
        page,
        Text::new("Changes are saved automatically.", theme).tone(Tone::Muted),
    );
    tree.add_child(
        page,
        Card::new(theme)
            .gap(theme.spacing(Space::SM))
            .padding(Edges::all(theme.spacing(Space::LG)))
            .child(
                Row::new()
                    .gap(theme.spacing(Space::SM))
                    .child(Button::primary("Save", theme))
                    .child(Button::ghost("Cancel", theme)),
            ),
    );

    tree
}

fn main() {
    let theme = default_theme(Mode::Dark);
    let mut tree = build(theme);
    let viewport = ViewportSize::new(Size::new(480.0, 320.0));

    // 3. One frame: layout -> update -> paint.
    igui_ui::layout(&mut tree, viewport);
    tree.update();
    let mut ctx = PaintContext::new();
    igui_ui::paint(&tree, &mut ctx);
    let list = ctx.into_draw_list();

    // 4. Any backend: recording here, wgpu / canvas in a real host (§5).
    let mut backend = RecordingBackend::new();
    backend.begin_frame(viewport).unwrap();
    backend.submit(&list).unwrap();
    backend.end_frame().unwrap();

    println!(
        "{} commands for {} controls",
        list.commands().len(),
        igui_ui::control_count(&tree),
    );
}
```

Run it with `igui_backend_recording` as a dependency (that is what every demo's
`--selfcheck` does). Swapping in a window is §5.

### 2.1 The frame loop is the only contract

Per frame, in this order:

```rust,ignore
// fragment: `app`, `tree` and the two view states belong to your host/app
// 1. state -> view (rebuild or mutate the tree)
app.update(viewport, dt);
// 2. geometry (two-pass measure + arrange; cached, only dirty subtrees move)
igui_ui::layout(&mut tree, viewport);
// 3. deferred component work (list/scroll sync, focus fixes, router switches)
tree.update();
// 3b. scroll-like views settle *after* layout — re-lay out if they moved
if list_state.sync(&mut tree) || scroll_state.sync(&mut tree) {
    igui_ui::layout(&mut tree, viewport);
}
// 4. paint into the IR
let mut ctx = igui_render::PaintContext::new();
igui_ui::paint(&tree, &mut ctx);
// 5. hand `ctx.into_draw_list()` to a backend
```

Getting this order wrong is the single most common source of "it renders stale /
misaligned": `List`, `ScrollView` and `Router` need `sync` and, when it returns
`true`, one more `layout`.

---

## 3 · Layout that does not look crude

The design language (see [`design-system.md`](design-system.md)) is
**monochrome + thin borders + subtle surfaces + precise spacing + strong
typography**. Five rules produce that look; the sixth is what usually breaks it.

### 3.1 Use a page shell

One page = a header, an optional sidebar, and a content column that grows. The
demos (`examples/file_browser`, `examples/deepseek_balance`) all do exactly this:

```rust
use igui_components::{Column, Component, Flex, Text};
use igui_core::Edges;
use igui_theme::{Space, Theme, Tone};
use igui_ui::SizeBasis;

fn page_shell(theme: &'static dyn Theme, tree: &mut igui_scene::SceneTree) -> igui_core::NodeId {
    let root = tree.add_child(tree.root(), Flex::row());

    // Sidebar: fixed width, never shrinks, keeps its own spacing.
    tree.add_child(
        root,
        Column::new()
            .basis(SizeBasis::Px(240.0))
            .shrink(0.0)
            .gap(theme.spacing(Space::XXS))
            .padding(Edges::all(theme.spacing(Space::MD)))
            .child(Text::small("Inbox", theme).tone(Tone::Muted)),
    );

    // Content: takes the rest, aligns its children, and owns the spacing
    // between sections (one gap, not a margin per child).
    tree.add_child(
        root,
        Column::new()
            .grow(1.0)
            .gap(theme.spacing(Space::XL))
            .padding(Edges::all(theme.spacing(Space::XXL)))
            .child(Text::title("Inbox", theme)),
    );
    root
}
```

Rules of thumb: **one gap per container**, never per-child margins; padding on
the container, not on the card; `grow(1.0)` on exactly one child per row/column.

### 3.2 Take every number from the tokens

| Need | Use | Not |
|---|---|---|
| spacing | `theme.spacing(Space::XS|SM|MD|LG|XL|XXL)` | `8.0`, `12.0` |
| font size | `Text::{display,title,heading,subheading,new,small,caption}` | `.font_size(17.0)` |
| weight | `.bold()` / `.weight(FontWeight::MEDIUM)` | a font family swap |
| radius | `theme.radius(Radius::Sm|Md|Lg|Panel)` | `4.0` |
| colour | `theme.palette().*`, `.tone(Tone::Muted)`, `theme.surface(SurfaceLevel::Raised)` | `#1f1f1f` |
| control height | `theme.control_height(ControlSize::Regular)` | `36.0` |
| row height | `theme.row_height()` | `28.0` |

Density is a token swap, not a second layout: `compact_theme(mode)` returns the
same theme at `0.75x` spacing, so `theme.spacing(..)` stays correct in both.

### 3.3 Hierarchy comes from surfaces and type, not borders everywhere

- `Card::new(theme)` = `surface_raised` + hairline border. Use it to group.
- `Divider::horizontal(theme)` separates rows *inside* a card.
- `Text::heading` + `Text::new(..).tone(Tone::Muted)` for title + supporting copy.
- `Badge::new("beta", theme)` for metadata; `Tone::{Muted, Subtle}` for
  de-emphasis; semantic tones (`success`/`warning`/`error`) only for status.
- Accent colour is for interaction and status only — not for decoration.

### 3.4 Size controls from the theme, and give text room

`Button::…(text, theme)` already reads `control_height`. If you need a custom
control, use `theme.control_height(size)` rather than a literal, and let text
wrap (`Text::wrap(true)`, `.max_lines(n)`, `.ellipsis(true)`) instead of
guessing widths.

### 3.5 Alignment: pick one, keep it

`Justify::{Start,Center,End,SpaceBetween,SpaceAround,SpaceEvenly}` and
`Align::{Start,Center,End,Stretch}` are on every container
(`Flex::column().justify(..).align(..)`). Left-align text, top-align cards, and
give every group the same gap. Ragged alignment is what makes a UI read as
"rough" even when each element is fine.

### 3.6 The text measurer (the invisible one)

If text looks slightly too wide, too tight, or vertically off-centre, the cause
is almost always the **measurer**, not the layout: without
`igui_ui::set_text_measurer(..)` (see §2) layout uses guessed advances and an
`0.8 * font_size` ascent while the backend paints real glyphs. Install it before
the first `layout()` — hosts do it in a plugin (`igui_winit::TextMeasurePlugin`).

### 3.7 Checklist when a layout looks off

1. `layout → tree.update → (sync) → paint` in that order?
2. A real `TextMeasurer` installed before the first layout?
3. Is exactly one child per row/column `grow(1.0)`?
4. One `gap` per container instead of per-child margins?
5. Every spacing/radius/size from `theme.*`, so density works?
6. `use igui_components::Component;` — otherwise `.child()` is missing (E0599)?
7. Anything meant to be flush with the edge is a child of the padded container,
   not the padded container itself?

---

## 4 · What each part gives you

| Crate | Gives you | Reference |
|---|---|---|
| `igui_core` | `Vec2`/`Size`/`Rect`/`Edges`/`Transform2D`, `Color`, `NodeId`, `InputEvent`, `ViewportSize` | [`components.md`](components.md) |
| `igui_render` | `DrawCommand`, `DrawList`, `PaintContext`, `RenderBackend` (the IR) | [`backend.md`](backend.md) |
| `igui_scene` | `SceneTree`, `Node`, `CanvasItem`, `Node2D`, `Viewport`/`Camera2D`, `process`/input callbacks | [`../internals/viewport-model.md`](../internals/viewport-model.md) |
| `igui_ui` | layout (flex/grid/anchors), paint, hit-test + input routing, focus, text editing | [`components.md`](components.md) |
| `igui_theme` | palette, spacing/type/radius scales, density, `Theme` trait | [`design-system.md`](design-system.md) |
| `igui_components` | `Component`/`Spec`, base builders, themed library, overlays | [`components.md`](components.md) |
| `igui_font` | system font discovery, family/weight + fallback, shaping, atlas | [`font.md`](font.md) |
| `igui_svg` | SVG subset → stroked polylines (icon packs such as Lucide) | [`svg.md`](svg.md) |
| `igui_assets` | PNG bytes → packed RGBA8 + size | [`font.md`](font.md) |
| `igui_anim` | tweens/easing; `is_animating()` is the "needs another frame" signal | [`../internals/plan.md`](../internals/plan.md) |
| `igui_profile` | frame timing, stage split, `inspect()` findings | [`debug.md`](debug.md) |
| `igui_debug_ui` | component bounds + performance panel overlays | [`debug.md`](debug.md) |
| `igui_backend_*` | `wgpu` (native), `canvas` (web), `recording` (tests) | [`backend.md`](backend.md) |
| `igui_app` / `igui_winit` / `igui_headless` | plugin runtime, window/GPU/input plugins, recording presenter | [`ui-guide.md`](ui-guide.md) §6 |
| `igui_ffi` / `wgpu_ffi` / `demoapp_ffi` | C ABI for value types + `DrawList`, the Rust backend, the real gallery | [`cpp-ffi.md`](cpp-ffi.md) |

The gallery in `examples/demo_app` is the same map, but live — one card per
capability. Its groups: `Layout`, `Text`, `Surfaces`, `Content`, `Icons`,
`Controls`, `Data`, `Overlays`, `Theme & Platform`, `Animation`
(`examples/demo_app/src/catalog.rs`). When you need to know "can it do X?", find
the card before writing code.

---

## 5 · Hosting it: three shapes

| Host | Crates | Use when |
|---|---|---|
| headless | `igui_backend_recording` | tests, `--selfcheck`, CI |
| native | `igui_app` + `igui_winit` (+ `igui_backend_wgpu`) | a desktop window |
| web | `igui_wasm` + `igui_backend_canvas` | a browser page |

A native host is assembly, not inheritance: `App::new(config).plugin(..)` with
`WinitPlugin` (window/loop), `WgpuPlugin` (surface + backend), `PointerPlugin` /
`KeyboardPlugin` / `ImePlugin` (input), `TextMeasurePlugin` (real metrics) and
`ClipboardPlugin`; the app implements `AppLogic`, which owns the tree and runs
§2.1. `examples/wgpu_demo` is the reference (~the whole file is plugin wiring
plus one `AppLogic`).

Input is always the same neutral vocabulary: `igui_core::InputEvent` →
`igui_ui::route_input(&mut tree, &event) -> EventResult`, with `Handled` telling
the host to stop (hover cursor comes from `igui_ui::hovered_cursor(&tree)`).

---

## 6 · The ten pitfalls

| Symptom | Cause | Fix |
|---|---|---|
| `.child(...)` / `.grow(...)` / `.ref_(..)` / `.on_click(..)` "does not exist" | those are trait methods | `use igui_components::Component;` |
| `Theme::dark()` does not exist | `Theme` is a trait | `igui_theme::default_theme(Mode::Dark)` |
| `on_click(|| …)` type error | callbacks take the tree and node | `.on_click(|_tree, _id| …)`; pointer/drag/key add their own args |
| nothing renders | no backend, no `submit` | `begin_frame` → `submit(&list)` → `end_frame` |
| text is wide/tight/off-centre | guessed metrics | install a `TextMeasurer` before the first layout (§2) |
| scrolling shows the wrong rows | sync missing or out of order | `layout → sync → layout if changed` (§2.1) |
| layout is stale after a change | layout is cached | `igui_ui::mark_dirty(&mut tree, id)` / `request_paint` |
| UI is stuck at the old size | `layout` not re-run on resize | call `layout(&mut tree, new_viewport)` on resize |
| a view switch rebuilds state | rebuilding the subtree | build once, toggle with `Router` (keeps scroll/focus) |
| everything is cramped or touching | `Column`/`Row` have zero gap by default | `.gap(theme.spacing(Space::MD))`; `Flex` defaults to 16/8 |

Two more, cheap to remember: `SceneTree` is **not** app state (keep state in
`Rc<Cell<..>>` and drain it in `update`), and `examples/demo_app`'s cards are the
compilable answer to almost every "how do I lay this out?" question.

---

## 7 · Where to go next

1. [`ui-guide.md`](ui-guide.md) — the long-form walkthrough: frame loop, state
   and events, hosting, conventions.
2. [`components.md`](components.md) — every component and layout option by name.
3. [`design-system.md`](design-system.md) — tokens, palette, density, the
   component library, overlays.
4. [`backend.md`](backend.md) — the backend contract and how to add one.
5. [`../internals/architecture.md`](../internals/architecture.md) — why the
   pipeline and crate split look like this.

`docs/README.md` has the full map, including the internals tree.
