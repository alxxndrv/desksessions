#pragma once

#include <cstdint>

namespace desk::anim {

/// Cubic Bézier timing curve through (0,0), (x1,y1), (x2,y2), (1,1), as in CAMediaTimingFunction.
struct CubicBezier {
  float x1;
  float y1;
  float x2;
  float y2;

  /// Eased progress for linear progress `t` in [0, 1].
  [[nodiscard]] float operator()(float t) const;
};

// Core Animation's named timing functions.
inline constexpr CubicBezier kCaDefault{0.25f, 0.1f, 0.25f, 1.0f};
inline constexpr CubicBezier kCaEaseIn{0.42f, 0.0f, 1.0f, 1.0f};
inline constexpr CubicBezier kCaEaseOut{0.0f, 0.0f, 0.58f, 1.0f};
inline constexpr CubicBezier kCaEaseInOut{0.42f, 0.0f, 0.58f, 1.0f};

/// Named curves used by tweens. The quadratic ones match Pebble's animation_timing tables.
enum class Curve : std::uint8_t {
  Linear,
  EaseIn,     // Pebble: t²
  EaseOut,    // Pebble: 1 − (1 − t)²
  EaseInOut,  // Pebble: both halves joined
  CaDefault,
  CaEaseIn,
  CaEaseOut,
  CaEaseInOut,
};

/// Eased progress for linear progress `t`; `t` is clamped to [0, 1].
[[nodiscard]] float ease(Curve curve, float t);

[[nodiscard]] constexpr float lerp(float from, float to, float t) { return from + (to - from) * t; }

}  // namespace desk::anim
