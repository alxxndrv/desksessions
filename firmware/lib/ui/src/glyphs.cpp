#include "desk/ui/glyphs.hpp"

#include <algorithm>
#include <cmath>
#include <numbers>

namespace desk::ui::glyphs {
namespace {

constexpr float kPi = std::numbers::pi_v<float>;

}  // namespace

void sparkle(Canvas& g, float cx, float cy, float radius, float angle, std::uint32_t color) {
  for (int spoke = 0; spoke < 4; ++spoke) {
    const float a = angle + static_cast<float>(spoke) * kPi / 2.0f;
    g.wedge(cx, cy, cx + std::cos(a) * radius, cy + std::sin(a) * radius, radius * 0.3f, 0.35f,
            color);
  }
}

void hexagon(Canvas& g, float cx, float cy, float radius, float angle, std::uint32_t color) {
  for (int side = 0; side < 6; ++side) {
    const float a0 = angle + static_cast<float>(side) * kPi / 3.0f;
    const float a1 = a0 + kPi / 3.0f;
    g.line(cx + std::cos(a0) * radius, cy + std::sin(a0) * radius, cx + std::cos(a1) * radius,
           cy + std::sin(a1) * radius, radius * 0.15f, color);
  }
}

void check(Canvas& g, float cx, float cy, float radius, std::uint32_t color) {
  const float w = radius * 0.18f;
  const float kneeX = cx - radius * 0.18f;
  const float kneeY = cy + radius * 0.48f;
  g.line(cx - radius * 0.62f, cy + radius * 0.02f, kneeX, kneeY, w, color);
  g.line(kneeX, kneeY, cx + radius * 0.68f, cy - radius * 0.52f, w, color);
}

}  // namespace desk::ui::glyphs

namespace desk::ui::glyphs {

void face(Canvas& g, float cx, float cy, float radius, const Face& f, std::uint32_t body,
          std::uint32_t features) {
  const float ry = radius * f.squash;
  const float bottom = cy + radius;  // squash from the ground up
  const float centerY = bottom - ry;
  // Body: a soft rounded blob, slightly wider than tall.
  const int w = static_cast<int>(std::lround(radius * 2.3f));
  const int h = static_cast<int>(std::lround(ry * 2.0f));
  g.fillSmoothRoundRect(static_cast<int>(std::lround(cx - radius * 1.15f)),
                        static_cast<int>(std::lround(centerY - ry)), w, h,
                        static_cast<int>(std::lround(std::min(radius, ry) * 0.95f)), body);

  // Eyes: dots that close into lines.
  const float eyeY = centerY - ry * 0.12f;
  const float eyeDx = radius * 0.42f;
  const float eyeR = radius * 0.13f;
  for (float side : {-1.0f, 1.0f}) {
    const float ex = cx + side * eyeDx;
    const float open = std::clamp(f.eyesOpen, 0.0f, 1.0f);
    if (open > 0.15f) {
      const float half = eyeR * open;
      g.line(ex, eyeY - half + eyeR * 0.6f, ex, eyeY + half - eyeR * 0.6f, eyeR, features);
    } else {
      g.line(ex - eyeR * 1.2f, eyeY + eyeR * 0.4f, ex + eyeR * 1.2f, eyeY + eyeR * 0.4f,
             eyeR * 0.45f, features);
    }
  }

  // Mouth: a small curve made of two short strokes.
  const float mouthY = centerY + ry * 0.32f;
  const float mw = radius * 0.28f;
  const float dip = radius * 0.16f * f.smile;
  g.line(cx - mw, mouthY, cx, mouthY + dip, radius * 0.06f, features);
  g.line(cx, mouthY + dip, cx + mw, mouthY, radius * 0.06f, features);
}

}  // namespace desk::ui::glyphs
