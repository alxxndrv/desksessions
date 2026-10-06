#pragma once

#include "desk/util/time.hpp"

namespace desk::anim {

/// Pebble's status-bar marquee timing.
inline constexpr Millis kMarqueeLeadPause{600};
inline constexpr Millis kMarqueeTailPause{750};
inline constexpr Millis kMarqueePerPixel{20};
inline constexpr Millis kMarqueeRewindPerPixel{2};

/// Leftward scroll offset (≥ 0) of text overflowing its box by `overflow` px, `elapsed` after it
/// was selected: pause, scroll to the end, pause, rewind fast, repeat.
[[nodiscard]] float marqueeOffset(float overflow, Millis elapsed);

}  // namespace desk::anim
