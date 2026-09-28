// A C++ OpenGL 3.3 backend for the igui `DrawList`.
//
// It is the host's own `RenderBackend`: transform, opacity and clip are
// resolved on the CPU while tessellating, so the GPU pass is one flat
// colored-triangle pipeline. This mirrors how `igui_backend_wgpu` works, but
// lives entirely in C++ and consumes the FFI command stream.

#pragma once

#include <cstdint>
#include <vector>

#include "igui.h"

namespace cppffi {

class GlBackend {
public:
    GlBackend() = default;
    ~GlBackend();
    GlBackend(const GlBackend&) = delete;
    GlBackend& operator=(const GlBackend&) = delete;

    // Compiles the program and creates the vertex array/buffer. Requires a
    // current GL context.
    bool init();

    // Starts a frame for a `fb_width x fb_height` device framebuffer whose
    // logical size is `fb_size / scale`.
    void begin_frame(int fb_width, int fb_height, float scale);

    // Consumes a whole draw list.
    void submit(const IguiDrawList* list);

    // Flushes anything left in the batch.
    void end_frame();

    std::uint64_t triangles() const { return triangles_; }
    std::uint64_t commands() const { return commands_; }

private:
    struct Vertex {
        float x, y;
        float r, g, b, a;
    };

    struct State {
        IguiTransform transform;
        float opacity;
        bool has_clip;
        IguiRect clip;
    };

    void reset_state();
    void apply(const IguiCommand& command);
    void flush();
    void set_scissor(const IguiRect& rect);
    void clear_scissor();

    // Tessellation, in the current local space; `apply` bakes the transform.
    IguiVec2 transform_point(IguiVec2 p) const;
    void triangle(IguiVec2 a, IguiVec2 b, IguiVec2 c, const IguiColor& color);
    void fill_convex(const std::vector<IguiVec2>& points, const IguiColor& color);
    void ring(const std::vector<IguiVec2>& outer, const std::vector<IguiVec2>& inner,
              const IguiColor& color);
    void stroke_segment(IguiVec2 from, IguiVec2 to, float width, const IguiColor& color);

    unsigned program_ = 0;
    unsigned vao_ = 0;
    unsigned vbo_ = 0;
    int viewport_location_ = -1;
    int fb_width_ = 0;
    int fb_height_ = 0;
    float scale_ = 1.0f;
    std::vector<Vertex> vertices_;
    std::vector<State> stack_;
    State state_{};
    bool scissor_on_ = false;
    IguiRect scissor_{};
    std::uint64_t triangles_ = 0;
    std::uint64_t commands_ = 0;
};

}  // namespace cppffi
