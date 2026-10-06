#pragma once

#include <cstdint>

#include "desk/ui/canvas.hpp"

namespace desk::ui::glyphs {

// Colours are 0xRRGGBB.

/// Claude: a four-point sparkle.
void sparkle(Canvas& g, float cx, float cy, float radius, float angle, std::uint32_t color);

/// Codex: a hexagon outline.
void hexagon(Canvas& g, float cx, float cy, float radius, float angle, std::uint32_t color);

/// DONE: a check mark.
void check(Canvas& g, float cx, float cy, float radius, std::uint32_t color);

}  // namespace desk::ui::glyphs

namespace desk::ui::glyphs {

/// The little character on the quiet, offline and pairing screens.
struct Face {
  float eyesOpen = 1.0f;  // 0 closed (a line) … 1 open
  float smile = 1.0f;     // 0 flat … 1 smiling
  float squash = 1.0f;    // vertical scale for bounces
};

void face(Canvas& g, float cx, float cy, float radius, const Face& face, std::uint32_t body,
          std::uint32_t features);

}  // namespace desk::ui::glyphs
