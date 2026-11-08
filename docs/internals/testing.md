# Testing

## Layers

- Math tests — `igui_core`.
- SceneTree / transform / visibility tests — `igui_scene`.
- DrawList / golden tests — `igui_render` + `igui_scene::paint`.
- RecordingBackend assertions — `igui_backend_recording` (`CommandAsserts`,
  `tests/pipeline.rs`).
- wgpu pixel readback — `igui_backend_wgpu` renders to an offscreen texture and
  asserts on returned RGBA8 pixels (`tests/render.rs`); no window is created.
- Canvas op mapping — `igui_backend_canvas` resolves every `DrawCommand` into a
  `CanvasOp` on the CPU (`src/ops.rs`: CSS colours, device-space clip rects,
  clamped corner radii, resolved image source rects, font spec + `textAlign`)
  and asserts that mapping with native `cargo test`. Only the `web-sys`
  executor in `src/canvas.rs` needs a browser, and it makes no decisions of its
  own.
- Layout / hit-test / input tests — `igui_ui` unit tests (includes flex/grid
  arrangement, align/justify/order, span-aware tracks, whole-viewport layout
  caching, partial relayout via `last_arranged_nodes`, order-cache
  invalidation, and the injected-measurer wrapping path).
- Text measurement tests — `igui_ui` asserts per-character measurement, soft
  wrapping (spaces, CJK, hard breaks), `TextOptions` (`max_lines`/`ellipsis`),
  button wrapping, that paint emits one `DrawText` per laid-out line, and that
  swapping the measurer recomputes the cached text layout.
- Font service tests — `igui_font` needs no browser: discovery metadata
  (`name` / `OS/2` / per-face `cmap` coverage), family/weight resolution,
  shaping + bidi, per-character fallback, and atlas growth (a slot's texel rect
  stays valid after the atlas doubles).
- Font render tests — `igui_backend_wgpu` asserts bitmap-fallback dimensions
  and missing glyphs, and (when a system font is present) proportional advances
  (`i` < `W`), `ascent`/`line_height`, and that ASCII + CJK glyphs rasterize.
  Render tests assert `DrawText` ink and CJK ink headlessly.
- Image / SVG / animation tests — `igui_assets` decodes PNG fixtures to RGBA8
  and rejects bad input; `igui_svg` parses the Lucide subset into flattened
  polylines (stroke only); `igui_anim` advances time by hand (no clock) and
  asserts easing, completion and `is_animating`.
- Runtime tests — `igui_app` unit-tests the type-keyed `ServiceMap` (plugin
  order/lifecycle is covered end-to-end by `wgpu_demo`'s `--selfcheck`);
  `igui_headless` asserts a recorded frame; `igui_winit` tests its pure platform
  → `InputEvent` mappings (named keys, wheel device/phase, pinch/rotate,
  double-click, IME) without opening a window.
- FFI tests — `igui_ffi` pins the builder → iterator round-trip and the ABI
  version (the C header stays the layout contract); `demoapp_ffi` / `wgpu_ffi`
  are exercised by `examples/cpp_ffi --selfcheck`.
- Profiler / inspector tests — `igui_profile` (ring buffer, summary math, every
  `FindingCode`, budget escalation). Pure data, no clock.
- Component debug tests — `igui_ui` asserts `paint_debug` emits one yellow
  `StrokeRect` + a `Name #id` `DrawText` per visible control; `igui_debug_ui`
  tests `DebugOverlay` (bounds, labels, open/closed, label options).
- Performance panel tests — `igui_debug_ui` asserts the computed `OverlayText`,
  that the labels mirror it, that the panel is pinned, and that paint emits the
  expected `DrawText`/`FillRect` commands (and nothing while closed).
- Benchmark harness tests — `igui_bench` asserts stats math, percentile
  interpolation, baseline text round-trips, verdict classification (including
  threshold boundaries) and the filter/runner behavior. These are pure data, no
  timing assertions.
- Benchmark scenario tests — `igui_bench_suite` asserts fixtures are
  deterministic (identical `DrawList`s), start clean, and hit-test to the
  expected control. They never assert on measured time.
- Shared demo app tests — `demo_app` drives the backend-neutral `DemoApp`
  natively and asserts layout rects (panel bounds, wrapped-label clipping, flex
  fill, grid tiling/no overlap, resize) plus a full recorded frame via
  `igui_backend_recording`. This is how the wgpu/WASM example layout is verified
  without a window or screenshot.

Core behavior must be testable with native `cargo test`, without a browser.
Only the Canvas backend and WASM glue need a browser.

## Hard rule: no screenshot / screen-recording testing

Never verify rendering with `screencapture`, browser screenshots, screen
recording, or any OS/window capture. Verify programmatically instead:

- **Backend pixel/output assertions** — assert against what a backend produces
  (e.g. recorded command sequences via `CommandAsserts`).
- **DrawList command sequences** — `CommandAsserts` and golden comparisons.
- **DOM state markers** — the web demo exposes `data-igui-*` attributes that
  headless checks read from `--dump-dom` (no image capture).

If a claim cannot be verified without a screenshot, say so explicitly rather than
capturing one.

## Doc examples

Every Rust doc example in `src/` is a `no_run` doctest with hidden `# ` setup
lines, so `cargo test --doc --workspace` — part of the normal gate — fails when
an API moves and a snippet stops matching it. Prefer fixing a snippet to match
the API over hiding it again.

Markdown is compiled too: `scripts/check-docs.py` runs `cargo check` on every
fence tagged `rust` in `README.md`, `AGENTS.md`, `release.md` and `docs/`. A
fence that is a genuine fragment is tagged `rust,ignore` and says in one line
what is missing; `text` / `bash` / `toml` / `cpp` fences are skipped.

## Golden / snapshot tests

`DrawList` is deterministic. `igui_scene`'s `scene_to_draw_list_is_deterministic`
and `igui_render`'s `drawing_is_deterministic` compare exact command sequences.
Extend by asserting the `Vec<DrawCommand>` directly.

## Benchmarks vs tests

Benchmarks live in `benches/` targets (`harness = false`) and are run with
`cargo bench`, never `cargo test`. They measure time and are machine-dependent,
so they are **not** part of the correctness suite and make no assertions on
measured time. What *is* tested is the harness itself (`igui_bench`) and the
determinism of every scenario fixture (`igui_bench_suite`). The `bench` profile is
pinned to `opt-level = 3`. See `docs/internals/benchmarking.md`.

## What needs a browser

- `igui_backend_canvas` + `igui_wasm` (Canvas 2D) — a browser.
- `examples/web_demo` — a browser (functionality is also covered by native
  `igui_ui` tests).

`igui_backend_wgpu` needs no browser: it renders offscreen and reads pixels back,
so it runs under plain `cargo test`. Its windowed demo `examples/wgpu_demo` opens a
real window and cannot be verified without a display; it is compiled by
`cargo check` and run manually, and the render path it uses is the same one
covered by the readback tests.

The `examples/wgpu_demo` overlays are host-wired: the component bounds and the
performance panel text, layout and emitted commands are covered by
`igui_ui`/`igui_debug_ui` tests, and the numbers the panel displays come from
`igui_profile` (tested with injected durations). The windowed overlays themselves
are **not** screenshot-verified.
