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
