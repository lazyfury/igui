# FontServer

Status: **implemented** (`crates/assets/igui_font`). The render IR and the UI stay
text-free; `igui_backend_wgpu` consumes the service for rasterization/atlas.

Decisions (2025-09):

- **Crate**: `igui_font` (`igui_font -> igui_core`).
- **Weight**: `FontWeight` is numeric (`igui_core`, 100–900).
- **Fallback**: per-character font fallback is implemented.
- **Canvas**: no font list / picker on the Canvas/WASM path; native/wgpu only.

## Size semantics

`FontServer`/`FontMetrics` sizes are logical pixels of the font's **line box**
(ascent+descent) by default, because that is how `ab_glyph`'s `PxScale` is
defined. Set [`FontConfig::em_size`] to interpret sizes as **em pixels**
(CSS/VS Code semantics, and what the Canvas backend's `px` already means):
`14.0` then renders a 14 px em for every family, instead of 10 px on PingFang
and 12 px on Menlo. The conversion is per face
(`ascent+descent / units_per_em`) and applies to advances, shaping,
rasterization, line height and ascent alike, so a host that measures with
`FontMetrics` stays in sync.

## What it does

- **Discovery** (`FontServer::families`): scans the system font directories
  into families + weights, so an application can build a font picker. On macOS it also scans
  `/System/Library/AssetsV2/com_apple_MobileAsset_Font*/**/AssetData` — that is
  where PingFang lives (there is no `/System/Library/Fonts/PingFang.ttc`).
  **Deferred when a seed is known:** with [`FontConfig::default_face`] (a
  concrete `FaceRef`), `load_with` reads that one face and does **not** scan;
  the scan runs on the first `families()`, unknown-family
  `resolve`, or uncovered character. Without a seed it still scans up front (so
  a fontless system can fall back to the pixel bitmap before the first frame).
- **Resolution** (`FontServer::resolve`): `(family, weight)` → the nearest
  weight in that family, falling back to the default family. The family-aware
  entry points (`advance_in`, `advance_run_in`, `shape_in`, `line_height_in`,
  `ascent_in`) select the primary face per call, so one server can paint the
  proportional UI face and a fixed-pitch terminal face in the same frame.
  Coverage/primary caches are keyed by the resolved primary, never just by
  weight.
- **Shaping** (`FontServer::shape`): bidi reordering + `rustybuzz`, split by
  font coverage so Latin, CJK and other scripts can come from different faces
  in one line. Fallback probes read each candidate's cmap through a short-lived
  memory map and load only the face that actually covers the character; a font
  file's bytes are read once per path and shared by every collection index.
  This avoids loading (and retaining) large `.ttc` collections such as Apple
  Color Emoji while merely looking for a kana/symbol fallback.
- **Rasterization** (`FontServer::take_dirty_atlas`): `ab_glyph` glyphs into one
  shared shelf atlas (all faces share it, so a backend uploads one texture).
  The atlas starts at 1024² and doubles its height on demand up to 1024×4096;
  slots carry texel coordinates, so a growable atlas needs no UV rebake. The
  update is a tight dirty rectangle (the full atlas after a growth).
  `atlas_size()` reports the live size; a backend recreates its texture when
  that changes.

## API

```rust
use igui_core::FontWeight;
use igui_font::{FaceRef, FontConfig, FontRequest, FontServer};

let server = FontServer::load_with(FontConfig::default());
for family in server.families() {
    println!("{} {:?}", family.name, family.weights); // e.g. PingFang SC [100..600]
}
let id = server.resolve(&FontRequest::new("PingFang SC", 700)); // nearest weight
let glyphs = server.shape("Hello 中文", 24.0, FontWeight::NORMAL);
```

When the application ships or knows its font file, seed it and skip the scan:

```rust,ignore
// fragment: needs context from the surrounding code
let server = FontServer::load_with(FontConfig {
    default_face: Some(FaceRef::new("/opt/app/fonts/Inter.ttf", 0)),
    ..FontConfig::default()
});
// No system scan: shaping uses the seed until a fallback / picker needs more.
```

`FontMetrics` wraps an `Rc<FontServer>` for hosts that build a
`igui_ui::TextMeasurer`; it exposes `advance_weighted` / `measure_run_weighted`
so layout measures the same faces the backend draws. The `*_in` variants add a
`family: Option<&str>` argument; `igui_ui::TextMeasurer` mirrors them with
default implementations that ignore the family, so existing measurers keep
compiling.

```rust,ignore
// The same family the draw names, so cell widths match the glyphs.
let advance = metrics.advance_in('M', 14.0, Some("Menlo"));
let glyphs = server.shape_in("ls -l", 14.0, FontWeight::NORMAL, Some("Menlo"));
```

To override discovery with a single face, pass [`FontConfig::default_face`].
`igui_font` reads no environment variables; a host that wants an env override
resolves the file itself and builds the `FaceRef`. `IGUI_FONT_BOLD` is gone
(weight resolution replaces it).

## Background (the old limits)

- `candidate_paths()` in `igui_backend_wgpu` was a fixed list. PingFang is not
  in it, and it has no stable path on modern macOS — it lives under
  `/System/Library/AssetsV2/com_apple_MobileAsset_Font*/<hash>/AssetData/PingFang.ttc`.
- The old loader always used face index 0. `PingFang.ttc` has **24 faces = 6
  weights × 4 regions**, so index 0 only ever yielded 苹方-港 (PingFang HK).
- One font per backend: no family/weight selection, no fallback chain.
- `FontWeight` was two-step (`Normal`/`Bold`); PingFang has six weights and no
  700.

## Model

- **Family** — a named set of faces (`PingFang SC`, `Menlo`). A draw may name
  one per run; `None` means the server default.
- **Face** — one font in a family. For a static collection (TTC/OTC) it is
  `(file, face_index)`; for a variable font it is `(file, axis_coords)`. Weight
  is a face property (`OS/2.usWeightClass`, 100–900) or the `wght` axis.
- **Request** — `{ family: Option<String>, weight: u16 }` resolved to a
  `FontId`. Resolution never fails: nearest weight, then default family.

How the ecosystems express this (the user's question):

- **Browsers** expose `font-family` + numeric `font-weight` (100–900) and pick
  the nearest face (or variable-axis instance) internally via the OS font
  database. Authors never see a face index.
- **Godot** exposes `FontFile.face_index` for collections and
  `FontVariation.variation_opentype` (`{"wght": 700}`) for variable fonts;
  shipping one file per weight is also common.
- So *face index* is the collection/file mechanism, not the abstract weight
  scheme. `FontServer` models **family + numeric weight** and hides the
  face index / axis behind it.

PingFang, concretely (measured from `PingFang.ttc`):

| weight | usWeightClass | SC face index |
|---|---|---|
| 極細 Ultralight | 100 | 23 |
| 纖細 Thin | 200 | 19 |
| 細 Light | 300 | 15 |
| 標準 Regular | 400 | 3 |
| 中黑 Medium | 500 | 7 |
| 中粗 Semibold | 600 | 11 |

(`Bold` = 700 has no exact face; nearest is 600.)

## Layout

`igui_font` owns discovery, face loading, shaping, bidi and rasterization +
atlas; `igui_backend_wgpu` holds an `Rc<FontServer>` and turns the returned
`GlyphSlot`s into quads (it no longer parses fonts). `FontServer::font.rs` in the
backend is a thin re-export (`Font` = `FontServer`).

## Discovery

- Scans per-OS directories (`/System/Library/Fonts`, `…/Supplemental`,
  `/Library/Fonts`, `~/Library/Fonts` on macOS; `/usr/share/fonts`, … on Linux;
  `C:/Windows/Fonts` on Windows), plus the macOS `AssetsV2` font assets.
- Uses `ttf-parser` directly (no `fontdb`/CoreText dependency): for each file it
  enumerates `fonts_in_collection`, reads the English family name (`name` id 1,
  else 16) and `OS/2.usWeightClass`, and keeps `(file, face_index)` metadata
  only. Files are **memory-mapped** (`memmap2`) and dropped: only the pages the
  `name`/`OS/2` tables touch are faulted in, instead of copying whole (possibly
  tens-of-MB) collections into a heap buffer. Name records that `ttf-parser`
  cannot decode (Macintosh/Roman, which is all core faces such as `Menlo` and
  `Monaco` carry) are decoded as Mac OS Roman, so those faces are discoverable.
- A full scan is ~1–2 s in a debug build; a seed ([`FontConfig::default_face`])
  skips it (see "What it does").
- `FontServer::resolve` matches the family case-insensitively and picks the
  nearest weight (tie → heavier); unknown family → default family.

## Weight

`FontWeight` is a numeric newtype (`igui_core::FontWeight`, 1–1000) with named
steps (`NORMAL` 400, `BOLD` 700, `MEDIUM` 500, …). Resolution maps it to the
nearest `usWeightClass` — PingFang has no 700, so `BOLD` resolves to 600. The
Canvas backend passes the number straight into the CSS font shorthand.
Variable-font `wght` axes are **not** implemented yet (static collections only).

## Fallback

Per-character: each bidi run is split into maximal sub-runs covered by the same
face. The primary face (default family at the requested weight) is tried first,
then every discovered face in deterministic order. Covered characters are
cached per `(char, weight)`. Emoji/color fonts are out of scope (no outline).

## Memory / lifetime

Each used font **file** leaks its bytes once (`&'static [u8]`) because
`ab_glyph::FontRef` and `rustybuzz::Face` borrow them for the server's lifetime;
every face index of a collection borrows that same buffer, so a `.ttc` is never
kept in memory twice. Only faces that are actually rendered are loaded: a
coverage probe reads the `cmap` from a short-lived memory map and drops it. The
discovery scan does **not** leak either (it drops bytes after reading metadata).
`Rc`/`RefCell`, single-threaded.

## Status

- [x] `igui_font` crate + discovery (`families`, `resolve`, default family).
- [x] Numeric `FontWeight` + nearest-weight resolution.
- [x] Per-character fallback + shared atlas.
- [x] Growable shelf atlas (texel slots) + dirty-rect uploads.
- [x] Per-draw family + em-pixel sizes (`DrawText::family`, `FontConfig::em_size`).
- [x] Metadata-only coverage probes; one file buffer per collection.
- [x] `igui_backend_wgpu` consumes the service; Canvas weight is numeric.
- [x] `FontConfig::default_face` seed + deferred scan; discovery files mmap'd.
- [ ] Variable-font `wght` axes.
- [ ] Emoji / color-font fallback.
- [ ] Cache discovery metadata on disk (startup cost).

## Problems to be aware of

- **Name ambiguity**: family vs full vs PostScript vs localized names
  (`PingFang SC` / `苹方-简`). English (`name` id 1/16) is preferred.
- **Region variants**: PingFang SC/TC/HK differ by glyph form; the default list
  prefers `PingFang SC`, but discovery treats them as separate families.
- **Static collection vs variable font**: only `(file, face_index)` is handled.
- **Lifetime**: used faces leak their bytes (bounded by the faces actually
  rendered, but not reclaimable).
- **Per-script fallback cost**: an uncovered character walks the fallback list,
  probing each candidate's `cmap` from metadata (no face loaded) until one
  covers it, then loads only that face.
- **Canvas/WASM**: the browser cannot enumerate system fonts, so the picker is
  native/wgpu only.
- **Metrics per face**: `line_height`/`ascent` come from the primary face;
  `TextMeasurer`'s weight-aware methods are the hook for per-weight metrics.

