---
name: igui-ffi
description: Drive igui from C/C++ or another language over the C ABI. Use when writing a foreign host, reading the DrawList command record, loading the real demo_app, or rendering through the Rust wgpu backend from C++.
---

# igui C ABI

Source of truth: `docs/guide/cpp-ffi.md`; headers live next to their crates
(`crates/ffi/igui_ffi/include/igui.h`, `examples/demoapp_ffi/include/demoapp.h`,
`examples/wgpu_ffi/include/wgpu_ffi.h`).

## What exists

| Crate | Gives a foreign host |
|---|---|
| `igui_ffi` | core value types + a `DrawList` builder/iterator (**no** scene/UI, **no** backend) |
| `demoapp_ffi` | the real `demo_app` gallery: create -> update -> layout -> paint -> walk commands |
| `wgpu_ffi` | the Rust `wgpu` backend: hand it a window view + a `DrawList`, or render offscreen and read pixels |

Build them as `staticlib`/`cdylib`; the reference host is `examples/cpp_ffi`
(C++17, CMake, GLFW + OpenGL 3.3):

```bash
./examples/cpp_ffi/build.sh
./examples/cpp_ffi/build/cpp_ffi --selfcheck      # glReadPixels, no screenshot
./examples/cpp_ffi/build/cpp_ffi --dump           # command stream, no GPU
./examples/cpp_ffi/build/cpp_ffi --demoapp        # the real gallery
./examples/cpp_ffi/build/cpp_ffi --wgpu           # Rust backend instead of GL
```

## Rules that bite

- The boundary is **data only**: value structs, `IguiDrawList`, command records.
  A foreign host organizes its own UI and implements its own backend; nothing
  from a scene tree or a GPU type crosses over.
- Check `IGUI_ABI_VERSION` at startup; bumping a command field or a value layout
  is an ABI break.
- Commands carry a tag + the fields for that tag (see the table in
  `docs/guide/cpp-ffi.md`); unknown/newer tags must be skipped, not misread.
- Text arrives pre-shaped as glyph runs; the host still owns fonts and textures.
- `--dump` is the debugging tool: it prints the `DrawList` without a GPU, so
  layout bugs are diagnosable headlessly. Filter its output instead of printing
  everything.
