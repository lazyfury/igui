# Release v0.2.0 — 2026-09-28

Same-day follow-up to `v0.1.0`, which was an intermediate checkpoint taken at
the internal `igui_` prefix rename (its message still said "quill facade"). This
release lands the facade / C-ABI rename to `igui`, the Godot-style explicit
control model (Stage 33), keyboard focus navigation, and the `igui` facade's
backend / observability features.

Scope: `git log v0.1.0..v0.2.0`. Earlier same-day work already contained in
`v0.1.0` (the `igui_app` plugin runtime, Stage 31; the internal `igui_` crate
prefix; the first directory reorg) is summarised at the end for context.

## Breaking changes

### 1. `igui_ui::Widget` removed — explicit control model (Stage 33)

The closed `Widget` enum is gone. A `Control` is now a rectangle plus a
`Container` (`Leaf` / `Flex` / `Grid`) and an optional `ControlContent`
(Godot `_get_minimum_size` + `_draw`), so any crate can add a self-drawing
control without touching the core.

- `Component::widget()` -> `Component::{container(), content()}`; both have
  defaults (leaf container, no content).
- Read state through `igui_ui::content` / `igui_ui::container` (the old
  `igui_ui::widget` helper is gone); `dyn ControlContent::as_text()` replaces
  matching on `Widget::Label`.
- `igui_ui::paint` is the single canvas-item walker: world `Visual` and
  `Control` in one layer-ordered pass over `SceneTree::paint_items`
  (`SceneTree::paint_visual` shared). `SceneTree::paint` stays world-only.
- Activation counting moved to `Control::click_count`; hover/press come from
  `InteractState`.
- Removed: `Widget`, `ButtonData`, `ButtonState`, `button_state`, `BoxLayout`,
  `igui_ui::widget`.
- Added: `igui_ui::{Container, ControlContent, ContentRef, PaintEnv,
  PanelContent, TextContent, ButtonContent, TextVAlign, content, container}`
  and `igui_scene::{PaintItem, paint_visual}`.

Migration: replace a `widget()` impl with `content()` (or delete it when the
component already draws via `spec.foreground` / chrome), and replace every
`igui_ui::widget(..)` / `Control::widget` read with
`igui_ui::content(..).and_then(|c| c.as_text())`.

### 2. `NodeDecor` removed — chrome is now a `ControlContent`

Surface / foreground chrome is now `igui_ui::Chrome`, a `ControlContent` that
wraps an inner content with `paint_behind` / `paint_front`. A `Control` stores
exactly one self-draw value (`Control.decorations` is gone).

- Removed: `NodeDecor`, `DecorRef`, `add_decor`, `surface_decor`,
  `dynamic_surface_decor`, `foreground_decor`.
- Added: `add_background` / `add_foreground`, and `Spec::chrome`, which composes
  a component's `background` / `foreground` around its `content()`.
- `InteractState` moved to `igui_ui::state`; `decor.rs` deleted.

Note: `Spec.foreground` and the `.foreground(..)` / `.background(..)` builders
keep their existing signatures — only the low-level decorator factories moved.

### 3. Facade and C ABI renamed `quill` -> `igui`

The facade crate is now `igui/` (package `igui`). The C ABI header is
`igui.h`, symbols are `igui_*`, opaque types are `Igui*`, macros are `IGUI_*`.
`IGUI_ABI_VERSION` and every `repr(C)` layout are unchanged, so there is no
data migration — only a rename on the host side. See "Renames & downstream
follow-up" below for the mechanical mapping.

### 4. Crates grouped by role (path-dependency break)

`crates/core/` now holds only the engine crates
(`igui_core` / `igui_render` / `igui_scene` / `igui_ui` / `igui_anim` /
`igui_game`). The rest moved to role directories:

- `crates/components/` — `igui_theme`, `igui_components`
- `crates/assets/` — `igui_font`, `igui_svg`, `igui_assets`

No crate dependency or code changed — only the directory layout. Git
dependencies resolve by package name and are unaffected; **path** dependencies
must be pointed at the new directories.

## New capabilities

- **Keyboard focus navigation + named group hover** (`igui_ui::focus`):
  `FocusNav` / `FocusDir` + `set_focus` / `focus_move` / `focus_up..right` /
  `focus_next` / `focus_prev` / `focusable_nodes` / `set_focus_nav`. Directional
  moves prefer an explicit named neighbor, then fall back to a spatial search;
  Tab walks `tab_index` then tree order, wrapping. `handle_input` runs the
  focused control's `on_key` first, then arrow/Tab navigation, then Enter/Space
  activation. `Control` gained `focus` / `group` / `group_hover`;
  `InteractState` gained `group_hovered`. Components expose
  `Component::{focus_name, focus_neighbor_up/down/left/right, tab_index, group,
  group_hover}`; `Button` is focusable by default (opt out with
  `focusable(false)`).
- **Programmatic autofocus**: `Spec.focus_on_mount` + `Component::autofocus(bool)`
  let a `TextInput` take the keyboard on mount, in the window tree and in an
  overlay's own tree alike; a deeper autofocus child wins.
- **`igui` facade backend / observability features** (Stage 32):
  `wgpu`, `canvas`, `wasm` (implies `canvas`), `profile`, `debug` (implies
  `ui` + `profile`), `recording`, `bench`. Each also enables the `igui_core` /
  `igui_render` re-exports it needs. The facade still contains no logic and
  never forces a backend.
- **Unified canvas-item paint**: world visuals and controls now paint in one
  layer-ordered pass, plus per-node allocation fixes in
  `SceneTree::update_subtree` and `hit_test`.
- **Agent skills directive** in the README (`.agents/skills/<name>/SKILL.md`).

## Renames & downstream follow-up

The renames are naming-only — no API shape, ABI layout or behaviour changed —
so downstream follows a mechanical mapping instead of re-deriving APIs.

Mapping:

| Before | After |
|---|---|
| `quill` (facade crate) | `igui` |
| `cobbled_*` (internal prefix) | `igui_*` |
| `quill.h` | `igui.h` |
| C symbols `quill_*` | `igui_*` |
| C types `Quill*` | `Igui*` |
| C macros `QUILL_*` | `IGUI_*` |
| env `QUILL_ROOT` / `QUILL_TRACE` | `IGUI_ROOT` / `IGUI_TRACE` |

Follow-up per consumer:

1. **Rust deps / imports**: rename `quill` / `cobbled_*` paths to `igui` /
   `igui_*`, and `use quill::...` to `use igui::...`.
2. **Path dependencies**: repoint at `crates/core/`, `crates/components/`,
   `crates/assets/`. Git dependencies need no change (resolved by package name).
3. **C / C++ host**: include `igui.h`, rename symbols / types / macros, and
   rebuild. `IGUI_ABI_VERSION` and the `repr(C)` layout are unchanged, so a
   mixed old/new header is the only incompatibility.
4. **Scripts / env / bundles**: swap `QUILL_*` for `IGUI_*` (temp-dir prefixes,
   Info.plist bundle id, web_demo storage/dataset keys).
5. **Lockfiles**: regenerate — standalone lockfiles still carried stale
   `cobbled_*` entries.

## Verification

Gate for the tag: `cargo fmt --all -- --check`, `cargo check --workspace`,
`cargo test --workspace`, `cargo bench --workspace --no-run`, plus each
standalone demo's own gate (`deepseek_balance`, `file_browser`) and its
`--selfcheck`. `cpp_ffi` has no `cmake` on this host, so its C++ side is
verified by header / symbol inspection only. The `wasm32-unknown-unknown`
target was not installed, so the wasm cross-check was not run.
