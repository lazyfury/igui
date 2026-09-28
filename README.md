# igui

A backend-neutral 2D/UI drawing core in Rust — Godot's
`SceneTree -> Node -> CanvasItem -> Node2D / Control` model with a
Rust-friendly API.

- **Users (app authors):** [Quick start](#quick-start) ·
  [UI guide](docs/ui-guide.md) · [Examples](#examples)
- **Agents / contributors:** [AGENTS.md](AGENTS.md) (hard rules + task map) ·
  [For agents](#for-agents) · [Skills](#skills-for-agents)

## Architecture

```
Input -> SceneTree -> Update -> Layout -> Paint -> DrawList -> RenderBackend -> Pixels
```

| Crate | Responsibility |
|---|---|
| `igui_core` | math, color, IDs, base types |
| `igui_render` | `DrawCommand`, `DrawList`, `PaintContext`, `RenderBackend` |
| `igui_scene` | `Node`, `SceneTree`, `CanvasItem`, `Node2D`, transforms, `Viewport`/`Camera2D` |
| `igui_ui` | `Control` runtime, layout, paint and input routing |
| `igui_theme` | design tokens (palette, spacing, radius, type, motion) |
| `igui_components` | component library: base builders (`Component`/`Spec`) + themed components |
| `igui_profile` | frame timing/counters, `Profiler`, `inspect` / `InspectionReport` |
| `igui_debug_ui` | component debug bounds (`DebugOverlay`) + performance panel (`PerformanceOverlay`) |
| `igui_bench` | dependency-free benchmark harness (`BenchRunner`, `Baseline`, regression verdicts) |
| `igui_bench_suite` | deterministic CPU pipeline benchmarks (scene / ui / pipeline) |
| `igui_backend_canvas` | Canvas 2D backend |
| `igui_backend_recording` | headless recording backend for tests |
| `igui_backend_wgpu` | native `wgpu` backend (offscreen, pixel readback) |
| `igui_wasm` | browser glue (events, RAF, canvas wiring) |

`igui` is the **facade** over the crates above: it re-exports them behind
opt-in features (`ui`, `anim`, `game`, `app`, `headless`) without flattening
names, so imports still read `igui_components::…`. Dependency direction is
enforced by crate boundaries: the pure core crates never depend on browser APIs
or a concrete backend. See [`AGENTS.md`](AGENTS.md).

## Quick start

Depend on the crates you need (paths relative to this repo) — either the
facade, or the fine-grained crates directly (what the docs and examples use):

```toml
[dependencies]
igui = { path = "igui", default-features = false, features = ["ui"] }        # scene + ui + theme + components
igui_backend_wgpu = { path = "crates/platform/wgpu/igui_backend_wgpu" }  # pick one backend
```

Every frame is the same four steps; the tree is persistent and the frame is
immediate-mode over it:

```rust
// 1. state -> view: advance state, then rebuild/refresh the tree
app.update(viewport, dt);
// 2. resolve geometry (and flush deferred tree changes)
igui_ui::layout(&mut tree, viewport);
tree.update();
// 3. emit this frame's backend-neutral draw list
let mut ctx = igui_render::PaintContext::new();
igui_ui::paint(&tree, &mut ctx);
let list = ctx.into_draw_list();
// 4. hand `list` to a backend (wgpu / canvas / recording)
```

Read [`docs/ui-guide.md`](docs/ui-guide.md) for the full walkthrough (building
views, input, hosting, conventions) and
[`docs/getting-started.md`](docs/getting-started.md) for the first scene and
control.

## Status

Stage 31 (`igui_app` plugin runtime) is accepted: a non-core `igui_app`
runtime (`App`/`AppBuilder`/`Plugin`/`AppLogic`/`Runner` + a neutral
`Presenter`) with the `igui_winit` platform plugins and a winit-free
`igui_headless` recording presenter. Next up is Stage 32 (remaining `igui`
facade backend features). See [`docs/architecture.md`](docs/architecture.md)
for the stage ledger and [`docs/godot-migration.md`](docs/godot-migration.md)
for the migration plan.

## Examples

| Example | Shows |
|---|---|
| `examples/demo_app` | Shared three-column, macOS-style notes app (backend-neutral `DemoApp`) |
| `examples/web_demo` | `demo_app` on the Canvas 2D backend (`igui_wasm`) |
| `examples/wgpu_demo` | `demo_app` on a native `wgpu` surface + component/perf debug overlays |
| `examples/game_demo` | Top-down collect game in an `igui_game::GameView` + HUD |
| `examples/multi_tree` | Headless: repeated `into_tree()` calls yield independent trees (no shared ids/state) |
| `examples/cpp_ffi` | C++ UI + OpenGL backend on the `igui` C ABI (`igui_ffi`/`demoapp_ffi`/`wgpu_ffi`) |
| `examples/deepseek_balance` | Standalone macOS menu-bar tool: DeepSeek balance panel built from `igui_theme`/`igui_components` on a transparent `wgpu` surface |

## Build & test

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo bench --workspace --no-run
```

## Benchmark

```bash
cargo bench -p igui_bench_suite                       # CPU pipeline suite
cargo bench -p igui_bench_suite --bench pipeline -- --filter scene/update
cargo bench -p igui_backend_wgpu --bench wgpu         # offscreen + readback (skips with no adapter)
```

Save a baseline and later fail on a regression:

```bash
cargo bench -p igui_bench_suite --bench pipeline -- --save-baseline benches/cpu.baseline.txt
cargo bench -p igui_bench_suite --bench pipeline -- --baseline benches/cpu.baseline.txt
```

See `docs/benchmarking.md`.

## Run the web demo

```bash
# one-time: install matching tooling
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128

# build wasm + JS glue into examples/web_demo/dist
./examples/web_demo/build.sh

# serve (ES modules need http, not file://)
python3 -m http.server 8080 --directory examples/web_demo
# open http://localhost:8080/
```

## Run the native wgpu demo

```bash
cargo run -p wgpu_demo --release
```

See `examples/wgpu_demo/README.md` for details. Press **F3** / `` ` `` / **d** to
toggle component debug bounds and **F4** / **p** for the performance panel; see
`docs/debug.md` for wiring them into your own app.

## For agents

Two documents are the entry point; read them before scanning crates:

- [`AGENTS.md`](AGENTS.md) — the working agreement: the pipeline contract, hard
  rules (backend-neutral core, no screenshots, ask when unclear), dependency
  direction, the per-stage gate, and the **"Where to look"** map.
- [`docs/ui-guide.md`](docs/ui-guide.md) — the practical "build an app UI" guide:
  frame loop, widgets, hosting, conventions, and an agent cheat sheet.

When upgrading across releases, read [`release.md`](release.md): the breaking
changes, the new capabilities, and the rename / ABI migration steps.

Keep the workspace gate green:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo bench --workspace --no-run
```

Standalone demos (`examples/file_browser`, `examples/deepseek_balance`) are not
workspace members; run their gate with `--manifest-path` and their `--selfcheck`
when you touch one. Verify rendering programmatically (draw-list assertions,
recorded pixels, self-checks) — never with a screenshot.

### Skills (for agents)

Usage knowledge lives in **Agent Skills**, not in this README. Pi (and other
implementations of the [Agent Skills spec](https://agentskills.io/specification))
discovers a skill from a directory containing `SKILL.md`, advertises its
`name` + `description`, and loads the body only when a task matches.

**Agents working in this repo should create and maintain skills that explain
usage.** When you learn a usage pattern that the docs do not cover, write it
down as a skill; when an API moves, update the skill in the same change.

- **Location:** `.agents/skills/<name>/SKILL.md` (project skills, discovered
  from the working directory up to the repo root). Bundle any helper scripts or
  references beside `SKILL.md` and refer to them with relative paths.
- **Frontmatter:** `name` (lowercase, hyphenated, matching the directory) and
  `description`. The description must say **what** the skill does **and when to
  use it** — that is what routes the model to the skill.
- **One skill per usage area**, kept short and pointing at the source of truth
  rather than copying it. Suggested areas:
  - `igui-ui` — build / route / paint a UI (`docs/ui-guide.md`, `docs/components.md`)
  - `igui-backend` — choose or add a backend (`docs/backend.md`)
  - `igui-ffi` — the C ABI and foreign hosts (`docs/cpp-ffi.md`)
  - `igui-game` — the 2D game layer (`igui_game` / `GameView`)
- **Validate** by running Pi with the repository as the working directory and
  checking the startup diagnostics and the `/skill:<name>` command.

Example skeleton:

```markdown
---
name: igui-ui
description: Build, route and paint an igui UI. Use when building a view, wiring input, or hosting a frame loop.
---

# igui UI

Read `docs/ui-guide.md` first. Import from the `igui_*` crates (the `igui`
facade does not flatten names). Keep views headless-testable.
```
