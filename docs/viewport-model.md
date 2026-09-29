# Single-tree viewport model + world-space `Control` (M0 design)

Status: **approved — M0 design freeze.** Target confirmed by the user:
game-first, one `SceneTree`, `Viewport` nodes (root + `SubViewport`),
`CanvasLayer` (screen HUD), and `Control` as a first-class `CanvasItem` so a
`Control` inside the 2D world follows it (health bars, name tags).

This document is the contract for stages **M1–M6**. Each stage ends with a
report and waits for approval (AGENTS rule 6). Roadmap change recorded in
`AGENTS.md` (rule 10).

## Progress

- **M0 — done.** This document + the `AGENTS.md` roadmap entry.
- **M1 — done.** Node behaviour callbacks unified to
  `(&mut SceneTree, NodeId, …)` with clone-out dispatch across `igui_scene`,
  `igui_ui` and `igui_game` (the `PointerCallback`/`PointerTreeCallback` split
  merged; `Area`/`Timers`/`Signal` handlers tree-aware; `Signal` gained a
  per-handler `Connection`).
- **M2 — done (H2).** A `Control` parented to a `Node2D` resolves anchors
  against the node origin and paints under `canvas_transform * world_transform`,
  so a health bar / name tag follows its actor and the camera; a `Control` under
  a transformed `CanvasLayer` composites under it. UI-only apps are unchanged.
- **M4 — partial.** `MouseFilter::Pass` (a no-op) is removed; igui uses
  "topmost non-`Ignore` wins, activation bubbles to the nearest ancestor with a
  click callback". Godot's `MOUSE_FILTER_PASS` `_gui_call_input` walk that makes
  `Stop` block ancestors is **deferred**: the forgiving current behaviour is load
  bearing for components (a `Stop` child would otherwise mute a container's
  click).
- **M5 — done.** `examples/game_demo` is game-first on one tree (root viewport
  is the game; HUD under a `CanvasLayer`; the player's name tag is a `Control`
  under the player `Node2D`); `igui_game::GameView` and its `ui` feature are
  removed.
- **M3 — remaining.** Multi-instance `Viewport` + `SubViewport` (a second
  view / render target with its own camera and input routing). Not needed by the
  now-single-tree `game_demo`; it is the next focused stage.
- **M6 — done.** Docs updated (`docs/design-system.md`, `AGENTS.md`, README,
  `docs/architecture.md`).

Owner: `igui_scene` (viewport/transform/lifecycle) + `igui_ui` (layout/paint of
`Control`). `igui_game` is a consumer. Backend-neutral throughout.

---

## 1. Why (the real failure)

`examples/game_demo` is the acceptance case, not the crate internals. It is
currently an application-level mess:

| # | Symptom | Cause |
|---|---|---|
| 1 | `Game` holds `ui: SceneTree` **and** `view: GameView` (which owns a second `world: SceneTree`) | `GameView` owns a second tree (H1 resurfaces at the game layer) |
| 2 | two `update()` calls, two `add_*` styles, a hand-written frame scheduler in `advance()` | same |
| 3 | `collect_marks: Rc<RefCell<Vec<NodeId>>>` deferred removal | callbacks cannot touch the tree |
| 4 | `score: Rc<Cell<u32>>`, `keys: Rc<RefCell<Keys>>` | same |
| 5 | `event()` hand-parses keys, bypasses `route_input` | the world is not in the main tree |
| 6 | two clocks (`Game.clock`, `GameView.clock`), manual `sync_walk_animation` + `painting_walk` state machine | no node behaviour API |
| 7 | host feeds `view.set_viewport(control.rect)` every frame, `painted_generation` cache cell | `GameView` is detached from layout/paint |
| 8 | a `Control` under a `Node2D` is pinned to the full viewport, not the node | **H2** |

The pipeline primitives (paint/input/layer) were unified in Stage 33 / Phase 5,
but the **application composition** was not, because:
- `GameView` owns a second tree (H1), and
- `Control` layout is viewport-absolute and `Ui::paint` ignores the
  `CanvasItem.world_transform` (H2).

The stated goal — a `Control` that follows its `Node2D` (health bar, name tag) —
is **0 %** today. M2 (H2) is the enabling change, not an optimisation.

## 2. Godot semantics adopted (authoritative)

Two independent rules decide where a `Control` lands. Both are already recorded
from the Godot source in `docs/godot-migration.md` (Q1/Q3 Findings) and are
re-confirmed here as the contract.

### R1 — anchorable rect (the layout frame)

`Control::get_parent_anchorable_rect()`:

| parent | anchorable rect |
|---|---|
| `Control` | that control's rect (`0,0,size`) |
| other `CanvasItem` (`Node2D`) | `CanvasItem::get_anchorable_rect()` → origin, **size 0** |
| no canvas-item parent (under `CanvasLayer` / `Viewport`) | the containing `Viewport`'s visible rect |

### R2 — canvas transform (whether the camera applies)

`CanvasItem::get_canvas_transform()`:
- nearest `CanvasLayer` → `CanvasLayer::get_final_transform()`;
- otherwise → the containing `Viewport`'s `canvas_transform` (its current
  `Camera2D`).

So membership in the default canvas (layer 0) means "camera-affected"; membership
in a `CanvasLayer` means "screen-fixed" (unless `follow_viewport`).

### R3 — world transform composition (uniform for `Node2D` and `Control`)

```
draw_transform(id) = canvas_transform_of(id) * world_transform(id)
```

`world_transform` accumulates local transforms up the `CanvasItem` chain. A
`Control`'s local transform is written by layout (origin = its local rect),
exactly like a `Node2D`'s is written by `set_position`.

### R4 — viewport nesting

A `Viewport` is a `Node`. The root is one; a `SubViewport` is a non-root
`Viewport` whose subtree renders into a render target. A viewport's own camera
drives **its** `canvas_transform`; the container that displays it composites the
target under the parent viewport's transform.

## 3. Target architecture (game-first)

```
SceneTree
└─ Viewport (root)                 root = the game viewport
   ├─ default canvas (layer 0)     R2: camera applies
   │   ├─ Node2D world
   │   │   ├─ Player (Node2D)
   │   │   │   └─ Control(healthbar/nametag)   R1: parent=Node2D → origin,size 0
   │   │   └─ Camera2D                          → follows the player + camera
   │   └─ ...
   ├─ CanvasLayer (HUD)            R2: no camera → screen-fixed
   │   └─ Control(HUD)
   └─ SubViewport (minimap/RT)     R4: own size + camera + target
      ├─ Camera2D
      └─ ...
```

Three `Control` placements, one rule set:

| Placement | R1 frame | R2 transform | Example |
|---|---|---|---|
| under `Node2D` (default canvas) | parent origin, size 0 | world canvas × node world | health bar, name tag |
| under `CanvasLayer` | viewport rect | layer transform | HUD |
| under a `SubViewport` | that viewport's rect | that viewport's camera | minimap labels |

No explicit "align to viewport" flag is added: the tree position is the marker
(confirmed by the user). `CanvasLayer` selects the canvas; the nearest
`Viewport` ancestor selects the frame/camera.

## 4. Core abstractions

### 4.1 `Viewport` as a (multi-instance) node

- `NodeKind::Viewport` may occur anywhere, not only at the root.
- `Viewport { size, canvas_transform }` stays the per-node data; `visible_rect()`
  is its layout frame.
- `SceneTree::viewport_of(id) -> NodeId` — nearest ancestor `Viewport` (root
  fallback).
- `SceneTree::canvas_transform_of(id)` — resolves against `viewport_of(id)` and
  the nearest `CanvasLayer` (generalises today's root-only implementation).
- A viewport's `canvas_transform` is written by the current `Camera2D` inside
  that viewport's subtree.

### 4.2 `CanvasLayer` (unchanged role, clarified)

The only thing `CanvasLayer` decides is R2 (camera or not) plus layer ordering.
It has no rect.

### 4.3 `Control` as a first-class `CanvasItem`

- `ControlData.rect` becomes **local** to its anchorable parent (R1), not
  viewport-absolute.
- Layout writes the control's local translation (rect origin); size stays on
  `ControlData`.
- `Ui::paint` emits `ctx.set_transform(draw_transform(id))` and draws content in
  local space — the **same** path as a `Node2D`, no special case.
- Clip (`clip_rect`) is resolved in the same local/world chain.

### 4.4 `SubViewport`

- `SceneTree::add_sub_viewport(parent, name) -> NodeId` creates a non-root
  `Viewport` node with an owned `RenderTargetId`.
- The main paint pass **skips** a non-root viewport's subtree; a dedicated
  `paint_viewport(id, ctx)` paints it into its target.
- A `SubViewportContainer` (`Control`) composites the target under the parent
  viewport's transform.
- Output size = `viewport size × scale_factor`, recreated on change (today's
  `GameView` logic moves here).

### 4.5 Input routing per viewport

`SceneTree::route_input` keeps the Godot order (`_input` → world pick → GUI →
`_unhandled_input`), but each stage resolves against the **subtree of the
viewport under the pointer** (via the `SubViewportContainer` rect), so a HUD
control and a world control never fight across viewports. Exact algorithm is
finalised in M3.

## 5. Behaviour callbacks (M1, prerequisite, can run in parallel)

All node-triggered callbacks use one shape:

```rust
// igui_scene::behavior
pub type LifecycleCallback = Rc<RefCell<dyn FnMut(&mut SceneTree, NodeId, f32)>>;
pub type InputCallback   = Rc<RefCell<dyn FnMut(&mut SceneTree, NodeId, &InputEvent) -> EventResult>>;
```

Rules:
- signature `(&mut SceneTree, this NodeId, …) -> EventResult` (`()` for lifecycle);
- **clone-out dispatch**: clone the `Rc` out of the node, then call with
  `&mut SceneTree`; never hold a node's `RefCell` borrow across the call;
- storage: `Rc<RefCell<…>>` when the container is the tree being walked
  (scene/lib.rs, `Control`); `Box<dyn FnMut>` for host-owned runners
  (`Timers`/`Areas`/`Signal`);
- dispatch snapshot rule: removed nodes are skipped, added nodes run next frame,
  nested `process`/`route_input` is a debug-asserted error.

`igui_game` callbacks (`Area`, `Timers`, `Signal`, `GameView`) adopt the same
shape and receive the tree. `Areas::update` becomes two-phase (compute pairs
immutably, then fire with `&mut SceneTree`).

## 6. Stages

Each stage: implement → per-stage gate → report → wait for approval.

### M1 — behaviour callback unification

Boundaries:
- new `crates/core/igui_scene/src/behavior.rs`; `node.rs` field types; `input.rs`
  setters + dispatch (`process`/`physics_process`/`_input`/world pick/unhandled).
- `crates/core/igui_ui/src/control.rs` aliases + setters; `input.rs`, `focus.rs`,
  `content.rs`, `debug.rs` invocation sites.
- `crates/core/igui_components/src/base/mod.rs` `Spec` + builder methods.
- `crates/core/igui_game/src/{area,timer,signal,game_view}.rs`.
- examples + standalone demos mechanical migration.

Suggested slices: **M1a** scene, **M1b** ui + components, **M1c** game + examples.

Acceptance: workspace gate; `cargo test -p demo_app`; standalone
`--manifest-path` gates for `file_browser` / `deepseek_balance`; `game_demo`
`--selfcheck`.

### M2 — H2: `Control` joins the canvas-item transform chain

Boundaries:
- `crates/core/igui_scene/src/tree.rs`/`paint.rs`: `canvas_transform_of` generalised
  to nearest `Viewport`; `draw_transform(id)` helper.
- `crates/core/igui_ui/src/ui/layout.rs`: R1 `anchorable_rect` three-branch
  resolution; local rects; UI roots seeded from the containing viewport rect.
- `crates/core/igui_ui/src/ui/paint.rs`: apply `item.transform` for `Control`;
  draw in local space; clip in the same chain.
- `crates/core/igui_scene/src/input.rs`: hit-test a world `Control` through the
  inverse `draw_transform`.
- record in `docs/design-system.md`.

Acceptance: new tests (a `Control` under a moving `Node2D` emits the composed
transform; a `Control` under a `CanvasLayer` with a transform is offset; camera
moves the world `Control` and not the HUD); `cargo test -p demo_app`; existing
`--selfchecks` unchanged.

### M3 — multi-`Viewport` + `SubViewport`

Boundaries:
- `igui_scene`: `add_sub_viewport`, non-root viewport data + target, nearest
  viewport resolution, per-viewport paint pass, input resolution per viewport.
- `igui_render`/backends: reuse `create_render_target`/`render_to_target`; target
  lifetime tied to the node.
- `igui_ui`: `SubViewportContainer` control (composite the target); per-viewport
  layout.
- retire the `GameView` sub-tree in favour of the node (kept until M5 for the
  window hosts).

Acceptance: a `SubViewport` renders its own camera into a target that its
container composites; input reaches the correct viewport; recording backend
`--selfcheck`.

### M4 — `mouse_filter = PASS` bubbling + world-`Control` input

Boundaries:
- `igui_ui::input`: implement Godot `_gui_call_input` bubbling (consumed on
  `Stop`, continue on `Pass`; scroll respects pass); remove the no-op `Pass`.
- world `Control` hit-test already landed in M2, exercised here.

Acceptance: a `Pass` control forwards to its parent; a world `Control` is
clickable through the camera/viewport transform.

### M5 — `game_demo` rewrite (game-first) + `GameView` retirement

Boundaries:
- `examples/game_demo`: one `SceneTree`; world under the root viewport; HUD under
  a `CanvasLayer`; health bar / name tag as controls under the player; movement
  as `physics_process`; pickup via tree-aware `Area::on_enter`; frame is
  `physics_process` → `process` → `update` → `layout` → `paint`; `route_input`
  once.
- remove `igui_game::GameView`; drop `ui`-feature coupling where no longer
  needed; update `igui` facade features and `AGENTS.md` dependency block.
- window hosts (`wgpu_demo`, `game_demo/src/app.rs`) updated.

Acceptance: `cargo test -p game_demo`; `cargo run -p game_demo -- --selfcheck`;
`cargo test -p wgpu_demo`; workspace gate.

### M6 — docs + examples close-out

- `AGENTS.md` dependency/roadmap/status; `docs/architecture.md`;
  `docs/components.md`/`design-system.md`; this doc marked done.

## 7. Compatibility / breaking changes

- `igui_scene` and `igui_ui` may change incompatibly (migration window), as
  already allowed by `docs/godot-migration.md`.
- `ControlData.rect` changes meaning (absolute → local): every reader of
  `control.rect` that assumed screen coordinates must be audited (paint, input,
  focus nav, debug overlay, `GameView`, examples).
- Callback signatures change (M1): all `on_*` and `set_*_callback` call sites.
- `GameView` is removed in M5 (breaking for `igui`'s `ui`+`game` surface).

## 8. Risks / open items

1. **Layout rewrite blast radius (M2)** — the largest risk; land it behind the
   existing tests plus new transform assertions, and keep `--selfcheck` green.
2. **Anchor math for a `Node2D` parent** — R1 says size 0; confirm against the
   Godot source before M2 (`Control::get_parent_anchorable_rect`,
   `CanvasItem::get_anchorable_rect`).
3. **Mixed subtrees** — `Container` only arranges `Control` children (Q6
   Finding); `Node2D` children are legal but ignored by flex/grid. Keep that.
4. **Viewport nesting transforms (M3)** — a `SubViewport`'s camera must not leak
   into the parent canvas; the container composites the target, it does not
   compose the sub-camera.
5. **Input across viewports (M3/M4)** — pointer-to-viewport resolution is the
   subtle part; pin it with a test per viewport.
6. **Perf** — `hit_test`/`root_controls` currently scan the whole tree per
   pointer event; fix while in M2/M4.

## 9. Gate (every stage)

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo bench --workspace --no-run
```

Standalone demos touched by a stage add their own `--manifest-path` gate and
`--selfcheck`.
