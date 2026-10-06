#pragma once

#include "desk/util/time.hpp"

namespace desk::anim {

/// Apple's sleep-light rhythm (patent US6658577B2): a raised-cosine breath over 1.4 s followed
/// by 0.4 s of rest, repeating every 1.8 s.
inline constexpr Millis kBreathPeriod{1800};
inline constexpr Millis kBreathRest{400};

/// Intensity in [0, 1] at `elapsed` since breathing began.
[[nodiscard]] float breathe(Millis elapsed);

}  // namespace desk::anim
