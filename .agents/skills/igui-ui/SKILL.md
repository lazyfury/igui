---
name: igui-ui
description: Build, lay out, paint, test and host an igui UI. Use when creating a view, adding a component, wiring the frame loop or input, theming, or debugging layout/paint results.
---

# igui UI

Source of truth: `docs/ui-guide.md` (start here), then `docs/components.md` for
the API by name and `docs/design-system.md` for tokens.

## Frame loop (the order matters)

```rust
app.update(viewport, dt);          // state -> view
igui_ui::layout(&mut tree, viewport);  // resolve geometry
tree.update();                     // flush deferred tree changes
let list = tree.paint(viewport);   // Paint step -> DrawList
backend.begin_frame(viewport)?; backend.submit(&list)?; backend.end_frame()?;
```

## Build

- Components: `igui_components::{Column, Row, Flex, Card, Text, Button, List,
  Overlays, Router, ...}`; attach with `SceneChild::into_tree` +
  `SceneTree::add_child`, compose with `.child(..)`.
- Imports are **not flattened**: use `igui_components::Button`, never
  `igui::Button`. The `igui` facade only re-exports crates behind features.
- Colours/metrics come from the theme (`theme.palette()`,
  `theme.surface(SurfaceLevel::…)`, `theme.spacing(Space::MD)`), never hex
  literals or the `space::`/`control::` consts in components.
- Mutate an existing node with `set_text` / `set_on_click` / `mark_dirty`;
  static layout should be built once, not per frame.

## Input

`igui_ui::route_input(&mut tree, event)` returns which layer consumed it. Native
hosts map platform events to `igui_core::InputEvent` (see `igui_winit::input`).
Focus: `set_focus` / `focus_move` / `focus_next`; `Component::focus_name`,
`focus_neighbor_*`, `tab_index`, `group`, `group_hover`.

## Verify without a screen

Never screenshot. Assert layout rects, paint into `igui_backend_recording` and
check the `DrawList` command sequence, or run the app's `--selfcheck`. Every
core behaviour must be testable with plain `cargo test` (see `docs/testing.md`).

## Don't

Read/write the tree inside a callback without a shared cell; store app state in
the tree; add `web_sys`/`wgpu` anywhere in `igui_scene` / `igui_ui` /
`igui_components`.
