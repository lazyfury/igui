---
name: igui-backend
description: Choose, implement or debug an igui render backend. Use when picking between wgpu/Canvas/recording, adding a DrawCommand primitive, wiring a new platform, or verifying rendering without a screenshot.
---

# igui backend

Source of truth: `docs/guide/backend.md`. IR: `igui_render::{DrawCommand, DrawList,
PaintContext, RenderBackend}`.

## The contract

`RenderBackend` is `begin_frame(viewport)` -> `submit(&DrawList)` (one per
camera/viewport) -> `end_frame()`, plus `register_texture(rgba, w, h)` for
`DrawImage`. Backends only ever see bytes and IR commands — no scene, no UI, no
GPU type crosses the boundary.

## Which one

| Need | Backend |
|---|---|
| Native window (winit) | `igui_backend_wgpu` (+ `igui_winit` plugins) |
| Browser | `igui_backend_canvas` (+ `igui_wasm`) |
| Tests / `--selfcheck` | `igui_backend_recording` |

## Adding a primitive

1. Add the variant + fields to `DrawCommand` in `igui_render` (backend-neutral
   data only).
2. Implement it in **every** backend: `igui_backend_canvas`,
   `igui_backend_wgpu` (`backend/tessellate.rs`), `igui_backend_recording`.
3. Teach `igui_profile`'s inspector so the overlay and findings stay accurate.
4. Test the CPU-side resolution (tessellation, clip, opacity) natively, and the
   result via recording assertions or wgpu offscreen pixel readback — never a
   screenshot (`docs/internals/testing.md`).

## Pitfalls

- `wgpu` surfaces: drop the frame's `TextureView` in `end_frame`, or DX12
  `ResizeBuffers` fails (see `release.md` v0.3.1).
- Canvas font metrics must match `igui_ui`'s measurer (`canvas_font_spec`), or
  text layout and ink disagree.
- A headless backend may ignore what it cannot store; callers must not read
  `Ok` from `register_texture` as "displayed".
