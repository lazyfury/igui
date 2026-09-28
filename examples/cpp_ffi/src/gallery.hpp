// A small component gallery in C++, styled from the same `cobbled_theme` tokens as
// `demo_app` (see `theme.hpp`).
//
// The point is a like-for-like comparison: each painter mirrors the geometry
// and colors of its `cobbled_components` counterpart — button variants, checkbox,
// switch, card, divider and badge. ABI v1 has no text, so labels are drawn as
// neutral bars; the chrome (fill, border, radius, sizes) is what is being
// compared.

#pragma once

#include "theme.hpp"

namespace cppffi {

enum class ButtonVariant { Primary, Secondary, Ghost, Destructive };

struct ButtonState {
    bool hovered = false;
    bool pressed = false;
};

/// `cobbled_components::Button`: height 36, radius 6, padding-x 12.
void paint_button(Canvas& canvas, Rect rect, ButtonVariant variant, ButtonState state = {});

/// `cobbled_components::Checkbox`: 16x16 box, radius 4.
void paint_checkbox(Canvas& canvas, Rect box, bool checked);

/// `cobbled_components::Switch`: 34x18 track, full radius, 13px knob.
void paint_switch(Canvas& canvas, Rect track, bool on);

/// `cobbled_components::Card`: raised surface, hairline border, radius 8.
void paint_card(Canvas& canvas, Rect rect, bool bordered = true);

/// `cobbled_components::Divider`: a 1px rule at the rect's centre.
void paint_divider(Canvas& canvas, Rect rect);

/// `cobbled_components::Badge`: tinted or solid, radius 4.
void paint_badge(Canvas& canvas, Rect rect, Color tone, bool solid);

/// Lays out the gallery in `bounds` and paints it into `canvas`.
void build_gallery(Canvas& canvas, Rect bounds);

}  // namespace cppffi
