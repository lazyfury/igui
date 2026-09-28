#include "canvas.hpp"

namespace cppffi {

IguiVec2 to_igui(Vec2 v) { return IguiVec2{v.x, v.y}; }

IguiRect to_igui(Rect r) { return IguiRect{r.x, r.y, r.w, r.h}; }

IguiColor to_igui(Color c) { return IguiColor{c.r, c.g, c.b, c.a}; }

IguiPaint to_igui_paint(Color c) { return IguiPaint{to_igui(c)}; }

IguiTransform rotation(float angle, Vec2 origin) {
    const float s = std::sin(angle);
    const float c = std::cos(angle);
    // p' = R * (p - origin) + origin = R * p + (origin - R * origin).
    const float rx = c * origin.x - s * origin.y;
    const float ry = s * origin.x + c * origin.y;
    return IguiTransform{
        IguiVec2{c, s},
        IguiVec2{-s, c},
        IguiVec2{origin.x - rx, origin.y - ry},
    };
}

void Canvas::save() { igui_draw_list_save(list_); }
void Canvas::restore() { igui_draw_list_restore(list_); }

void Canvas::set_transform(IguiTransform transform) {
    igui_draw_list_set_transform(list_, transform);
}

void Canvas::set_opacity(float opacity) { igui_draw_list_set_opacity(list_, opacity); }

void Canvas::clip(Rect rect) { igui_draw_list_clip_rect(list_, to_igui(rect)); }

void Canvas::fill_rect(Rect rect, Color color) {
    igui_draw_list_fill_rect(list_, to_igui(rect), to_igui_paint(color));
}

void Canvas::stroke_rect(Rect rect, Color color, float width) {
    igui_draw_list_stroke_rect(list_, to_igui(rect), to_igui_paint(color), width);
}

void Canvas::line(Vec2 from, Vec2 to, Color color, float width) {
    igui_draw_list_line(list_, to_igui(from), to_igui(to), to_igui_paint(color), width);
}

void Canvas::fill_circle(Vec2 center, float radius, Color color) {
    igui_draw_list_fill_circle(list_, to_igui(center), radius, to_igui_paint(color));
}

void Canvas::stroke_circle(Vec2 center, float radius, Color color, float width) {
    igui_draw_list_stroke_circle(list_, to_igui(center), radius, to_igui_paint(color), width);
}

void Canvas::fill_rounded(Rect rect, float radius, Color color) {
    const IguiCornerRadii corners{radius, radius, radius, radius};
    igui_draw_list_fill_rounded_rect(list_, to_igui(rect), corners, to_igui_paint(color));
}

void Canvas::stroke_rounded(Rect rect, float radius, float width, Color color) {
    const IguiCornerRadii corners{radius, radius, radius, radius};
    igui_draw_list_stroke_rounded_rect(list_, to_igui(rect), corners, to_igui_paint(color),
                                        width);
}

}  // namespace cppffi
