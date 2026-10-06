#include "desk/anim/breathing.hpp"

#include <cmath>
#include <numbers>

namespace desk::anim {

float breathe(Millis elapsed) {
  const Millis active = kBreathPeriod - kBreathRest;
  const auto phase = Millis{((elapsed.count() % kBreathPeriod.count()) + kBreathPeriod.count()) %
                            kBreathPeriod.count()};
  if (phase >= active) {
    return 0.0f;
  }
  const float t = progress(phase, active);
  return 0.5f - 0.5f * std::cos(2.0f * std::numbers::pi_v<float> * t);
}

}  // namespace desk::anim
