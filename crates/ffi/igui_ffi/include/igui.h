/*
 * igui.h — C ABI over the igui core (`igui_ffi`).
 *
 * A foreign-language host builds its own scene/UI and uses this header to fill
 * a backend-neutral `DrawList`, then reads it back command by command and
 * rasterizes it with its own backend (the C++ `examples/cpp_ffi` OpenGL
 * renderer). Nothing from `igui_scene` / `igui_ui` crosses this boundary.
 *
 * The layout here is a hand-maintained mirror of `crates/ffi/igui_ffi/src/lib.rs`.
 * Every type is `#[repr(C)]` on the Rust side and made of `float` / a C enum /
 * an opaque pointer, so the two agree. If you change one, change the other and
 * bump `IGUI_ABI_VERSION`; a host should refuse to run when
 * `igui_abi_version()` disagrees with this constant.
 */
#ifndef IGUI_H
#define IGUI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define IGUI_ABI_VERSION 1u

/* An opaque, Rust-owned draw list. Never inspect it; use the functions below. */
typedef struct IguiDrawList IguiDrawList;

/* Command discriminator. Discriminants are part of the ABI. */
typedef enum IguiCommandTag {
    IGUI_CMD_SAVE = 0,
    IGUI_CMD_RESTORE = 1,
    IGUI_CMD_SET_TRANSFORM = 2,
    IGUI_CMD_SET_OPACITY = 3,
    IGUI_CMD_CLIP_RECT = 4,
    IGUI_CMD_FILL_RECT = 5,
    IGUI_CMD_STROKE_RECT = 6,
    IGUI_CMD_LINE = 7,
    IGUI_CMD_FILL_CIRCLE = 8,
    IGUI_CMD_STROKE_CIRCLE = 9,
    IGUI_CMD_FILL_ROUNDED_RECT = 10,
    IGUI_CMD_STROKE_ROUNDED_RECT = 11,
    /* DrawImage / DrawText are outside ABI v1. A record with this tag is
     * otherwise zeroed and should be skipped. */
    IGUI_CMD_UNSUPPORTED = 12
} IguiCommandTag;

typedef struct IguiVec2 {
    float x;
    float y;
} IguiVec2;

typedef struct IguiRect {
    float x;
    float y;
    float width;
    float height;
} IguiRect;

typedef struct IguiColor {
    float r;
    float g;
    float b;
    float a;
} IguiColor;

/* Two basis axes plus an origin. A point maps to
 * x_axis * p.x + y_axis * p.y + origin. */
typedef struct IguiTransform {
    IguiVec2 x_axis;
    IguiVec2 y_axis;
    IguiVec2 origin;
} IguiTransform;

/* Clockwise from the top-left, in logical pixels. */
typedef struct IguiCornerRadii {
    float top_left;
    float top_right;
    float bottom_right;
    float bottom_left;
} IguiCornerRadii;

typedef struct IguiPaint {
    IguiColor color;
} IguiPaint;

/* One command, flattened to a fixed layout. Only the fields named by `tag`
 * are meaningful; every other field is zeroed. The mapping is:
 *
 *   SAVE / RESTORE            no fields
 *   SET_TRANSFORM             transform
 *   SET_OPACITY               opacity
 *   CLIP_RECT                 rect
 *   FILL_RECT                 rect, paint
 *   STROKE_RECT               rect, paint, width
 *   LINE                      from, to, paint, width
 *   FILL_CIRCLE               center, radius, paint
 *   STROKE_CIRCLE             center, radius, paint, width
 *   FILL_ROUNDED_RECT         rect, corners, paint
 *   STROKE_ROUNDED_RECT       rect, corners, paint, width
 */
typedef struct IguiCommand {
    IguiCommandTag tag;
    IguiTransform transform;
    IguiRect rect;
    IguiCornerRadii corners;
    IguiVec2 from;
    IguiVec2 to;
    IguiVec2 center;
    IguiPaint paint;
    float radius;
    float width;
    float opacity;
} IguiCommand;

/* Returns IGUI_ABI_VERSION. */
uint32_t igui_abi_version(void);

/* Allocate an empty list. Release it with igui_draw_list_free. */
IguiDrawList *igui_draw_list_new(void);

/* Free a list. A null pointer is a no-op. */
void igui_draw_list_free(IguiDrawList *list);

/* Remove every command, keeping the allocation. */
void igui_draw_list_clear(IguiDrawList *list);

/* Number of commands; 0 for a null list. */
size_t igui_draw_list_len(const IguiDrawList *list);

/* Read one command. Out of bounds (or a null list) yields
 * IGUI_CMD_UNSUPPORTED with every field zeroed. */
IguiCommand igui_draw_list_command(const IguiDrawList *list, size_t index);

/* State commands. */
void igui_draw_list_save(IguiDrawList *list);
void igui_draw_list_restore(IguiDrawList *list);
void igui_draw_list_set_transform(IguiDrawList *list, IguiTransform transform);
void igui_draw_list_set_opacity(IguiDrawList *list, float opacity);
void igui_draw_list_clip_rect(IguiDrawList *list, IguiRect rect);

/* Geometry commands. */
void igui_draw_list_fill_rect(IguiDrawList *list, IguiRect rect, IguiPaint paint);
void igui_draw_list_stroke_rect(IguiDrawList *list, IguiRect rect, IguiPaint paint, float width);
void igui_draw_list_line(IguiDrawList *list, IguiVec2 from, IguiVec2 to, IguiPaint paint, float width);
void igui_draw_list_fill_circle(IguiDrawList *list, IguiVec2 center, float radius, IguiPaint paint);
void igui_draw_list_stroke_circle(IguiDrawList *list, IguiVec2 center, float radius, IguiPaint paint, float width);
void igui_draw_list_fill_rounded_rect(IguiDrawList *list, IguiRect rect, IguiCornerRadii corners, IguiPaint paint);
void igui_draw_list_stroke_rounded_rect(IguiDrawList *list, IguiRect rect, IguiCornerRadii corners, IguiPaint paint, float width);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* IGUI_H */
