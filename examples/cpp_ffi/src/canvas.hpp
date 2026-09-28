// The C++ side of the FFI: value types plus a thin `Canvas` wrapper.
//
// Everything the rest of the C++ code touches goes through here, so `igui.h`
// and the `Igui*` names appear in exactly one place. The UI and the OpenGL
// backend speak `Vec2` / `Rect` / `Color` / `Canvas`.

#pragma once

#include <cmath>

#include "igui.h"

namespace cppffi {

struct Vec2 {
    float x = 0.0f;
    float y = 0.0f;
};

struct Rect {
    float x = 0.0f;
    float y = 0.0f;
    float w = 0.0f;
    float h = 0.0f;

    float right() const { return x + w; }
    float bottom() const { return y + h; }
    Vec2 center() const { return {x + w * 0.5f, y + h * 0.5f}; }
};

struct Color {
    float r = 0.0f;
    float g = 0.0f;
    float b = 0.0f;
    float a = 1.0f;
};

// Conversions to the ABI types.
IguiVec2 to_igui(Vec2 v);
IguiRect to_igui(Rect r);
IguiColor to_igui(Color c);
IguiPaint to_igui_paint(Color c);

// Builds a rotation about `origin`, in the y-down convention the core uses
// (positive angle turns +X toward +Y, i.e. clockwise on screen).
IguiTransform rotation(float angle, Vec2 origin);

// Named command methods over an opaque `IguiDrawList`.
//
// The UI writes through these instead of calling the C functions directly, so
// it reads like drawing code rather than FFI marshalling.
class Canvas {
public:
    explicit Canvas(IguiDrawList* list) : list_(list) {}

    void save();
    void restore();
    void set_transform(IguiTransform transform);
    void set_opacity(float opacity);
    void clip(Rect rect);

    void fill_rect(Rect rect, Color color);
    void stroke_rect(Rect rect, Color color, float width = 1.0f);
    void line(Vec2 from, Vec2 to, Color color, float width = 1.0f);
    void fill_circle(Vec2 center, float radius, Color color);
    void stroke_circle(Vec2 center, float radius, Color color, float width = 1.0f);
    void fill_rounded(Rect rect, float radius, Color color);
    void stroke_rounded(Rect rect, float radius, float width, Color color);

private:
    IguiDrawList* list_;
};

}  // namespace cppffi
