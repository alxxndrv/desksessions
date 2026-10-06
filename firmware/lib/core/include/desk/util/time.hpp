#pragma once

#include <chrono>
#include <compare>

namespace desk {

using Millis = std::chrono::milliseconds;

/// Point on the monotonic clock, measured from boot. Only the Clock port produces these.
struct Instant {
  Millis sinceBoot{0};

  friend constexpr auto operator<=>(Instant, Instant) = default;
  friend constexpr Millis operator-(Instant a, Instant b) { return a.sinceBoot - b.sinceBoot; }
  friend constexpr Instant operator+(Instant a, Millis d) { return Instant{a.sinceBoot + d}; }
  friend constexpr Instant operator-(Instant a, Millis d) { return Instant{a.sinceBoot - d}; }
};

/// Fraction of `total` covered by `elapsed`, clamped to [0, 1]. Zero-length spans count as done.
[[nodiscard]] constexpr float progress(Millis elapsed, Millis total) {
  if (total.count() <= 0 || elapsed >= total) {
    return 1.0f;
  }
  if (elapsed.count() <= 0) {
    return 0.0f;
  }
  return static_cast<float>(elapsed.count()) / static_cast<float>(total.count());
}

}  // namespace desk
