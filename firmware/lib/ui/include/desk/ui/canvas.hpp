#pragma once

#include <LovyanGFX.hpp>
#include <cstdint>

namespace desk::ui {

/// The full-screen frame sprite, plus sub-pixel anti-aliased lines: LovyanGFX's public line
/// API rounds to whole pixels, which makes small spinning glyphs shimmer.
class Canvas : public lgfx::LGFX_Sprite {
 public:
  using lgfx::LGFX_Sprite::LGFX_Sprite;

  void wedge(float x0, float y0, float x1, float y1, float r0, float r1, std::uint32_t rgb888) {
    draw_wedgeline(x0, y0, x1, y1, r0, r1, rgb888);
  }
  void line(float x0, float y0, float x1, float y1, float radius, std::uint32_t rgb888) {
    draw_wedgeline(x0, y0, x1, y1, radius, radius, rgb888);
  }
  void dot(float x, float y, float radius, std::uint32_t rgb888) {
    draw_wedgeline(x, y, x, y, radius, radius, rgb888);
  }
};

}  // namespace desk::ui
