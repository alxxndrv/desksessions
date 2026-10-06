#pragma once

#include <array>
#include <cstdint>
#include <span>

#include "desk/util/time.hpp"

namespace desk::anim {

/// Pebble's design frame: its hand-tuned curves are tables of one value per 33 ms frame.
inline constexpr Millis kPebbleFrame{33};

/// A Pebble "moook" motion: a few frames of anticipation near the start, optional linear middle
/// frames, then frames of overshoot past the target that settle back onto it.
///
/// Offsets are in pixels, as in PebbleOS (fw/applib/ui/animation_interpolate.c).
struct MoookShape {
  std::span<const std::int16_t> in;   // added to `from`, along the direction of travel
  std::span<const std::int16_t> out;  // added to `to`, past the target
  std::uint8_t midFrames;
};

inline constexpr std::array<std::int16_t, 3> kMoookIn{0, 1, 20};
inline constexpr std::array<std::int16_t, 4> kMoookOut{4, 2, 1, 0};
inline constexpr std::array<std::int16_t, 5> kPeekOut{8, 4, 2, 1, 0};

/// 7 frames ≈ 231 ms. Cursor moves, window slides, row exits.
inline constexpr MoookShape kMoook{kMoookIn, kMoookOut, 0};
/// 10 frames ≈ 330 ms. Pebble's timeline scroll; our reconnect "wake".
inline constexpr MoookShape kMoookSoft{kMoookIn, kMoookOut, 3};
/// Doubled bounce from Pebble's timeline peek; the NEEDS YOU glyph hop.
inline constexpr MoookShape kPeek{kMoookIn, kPeekOut, 0};

/// Distance at which the pixel tables apply unscaled. Shorter moves get proportionally smaller
/// anticipation and overshoot so a 5 px move cannot jump 20 px past its target.
inline constexpr float kMoookReferenceDistance = 30.0f;

[[nodiscard]] constexpr Millis duration(const MoookShape& shape) {
  return kPebbleFrame * static_cast<int>(shape.in.size() + shape.midFrames + shape.out.size());
}

/// Position at `elapsed` of a moook move from `from` to `to`. Pebble steps from frame to frame;
/// this interpolates between neighbouring frames so the same motion renders smoothly at 60 fps.
[[nodiscard]] float moook(const MoookShape& shape, float from, float to, Millis elapsed);

}  // namespace desk::anim
