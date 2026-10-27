# Docs

Two audiences, two directories. Pick the one you are in, then follow the reading
path for your role.

| Directory | Answers | Audience |
|---|---|---|
| [`guide/`](guide/) | **How do I build with it?** | app authors, agents writing app code |
| [`internals/`](internals/) | **How and why is it built this way?** | contributors, reviewers, backend authors |

## Reading paths

**Start an app (or hand the repo to an agent)**
1. [`guide/start.md`](guide/start.md) — 30 seconds, then a runnable app, a layout
   recipe, a capability map and the pitfall list.
2. [`guide/ui-guide.md`](guide/ui-guide.md) — the long-form walkthrough (frame
   loop, hosting, conventions).
3. [`guide/components.md`](guide/components.md) — API reference by name.
4. [`guide/design-system.md`](guide/design-system.md) — tokens, palette, density,
   and the component library.

**Build a looked-after UI** — the same four, in the order above; `start.md §3`
is the recipe that keeps spacing, type and surfaces consistent.

**Write a backend** — [`guide/backend.md`](guide/backend.md) (contract, IR,
adding a primitive), then [`internals/architecture.md`](internals/architecture.md)
(pipeline and the reason the IR is backend-neutral).

**Use it from C/C++ or another language** — [`guide/cpp-ffi.md`](guide/cpp-ffi.md),
plus `crates/ffi/igui_ffi` and `examples/cpp_ffi`.

**Contribute or review a stage** — [`internals/architecture.md`](internals/architecture.md)
(stage ledger), [`internals/testing.md`](internals/testing.md) (test layers, the
no-screenshot rule), [`internals/benchmarking.md`](internals/benchmarking.md),
then [`internals/plan.md`](internals/plan.md) for what is open.

**Understand a design decision** — [`internals/godot-migration.md`](internals/godot-migration.md)
(why the scene is Godot-shaped) and
[`internals/viewport-model.md`](internals/viewport-model.md) (single tree,
viewports, world-space controls).

## Files

### `guide/` — using it

| File | Covers |
|---|---|
| [`start.md`](guide/start.md) | the entry point: quick start → layout recipe → capability map → pitfalls |
| [`getting-started.md`](guide/getting-started.md) | first scene, first control, debug overlays, profiling |
| [`ui-guide.md`](guide/ui-guide.md) | frame loop, view building, state/events, hosting, conventions |
| [`components.md`](guide/components.md) | every component, layout, input, text fields, extension |
| [`design-system.md`](guide/design-system.md) | tokens, palette, scale, density, component library, overlays |
| [`backend.md`](guide/backend.md) | backend contract, Canvas/wgpu/recording, adding a primitive |
| [`font.md`](guide/font.md) | font discovery, family/weight resolution, fallback, shaping |
| [`svg.md`](guide/svg.md) | the SVG subset and icon packs (Lucide) |
| [`debug.md`](guide/debug.md) | profiler, inspection findings, debug/performance overlays |
| [`cpp-ffi.md`](guide/cpp-ffi.md) | the C ABI, the C++ host, foreign-language backends |

### `internals/` — how it is built

| File | Covers |
|---|---|
| [`architecture.md`](internals/architecture.md) | pipeline, coordinates, crate roles, the stage ledger |
| [`godot-migration.md`](internals/godot-migration.md) | the Godot-style scene migration, phase by phase |
| [`viewport-model.md`](internals/viewport-model.md) | single-tree viewports, `SubViewport`, world-space `Control` |
| [`plan.md`](internals/plan.md) | the roadmap: primitives, components, deferrals |
| [`testing.md`](internals/testing.md) | test layers, doc examples, the no-screenshot rule |
| [`benchmarking.md`](internals/benchmarking.md) | the harness, baselines, regression verdicts |

## House rules for these docs

- **English**, like the code and the crate docs.
- **Code blocks must compile.** Fences are `rust` when the block stands alone;
  mark a partial fragment with `rust,ignore` *and* say what is missing. This is
  checked — see [`internals/testing.md`](internals/testing.md).
- Prefer linking over restating: one fact, one home.
