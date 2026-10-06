#include "desk/ui/glyphs.hpp"

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
