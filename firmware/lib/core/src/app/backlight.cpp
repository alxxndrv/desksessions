#include "desk/app/backlight.hpp"

#include <algorithm>

#include "desk/anim/breathing.hpp"

namespace desk::app {
namespace {

constexpr float kPulseDepth = 0.65f;
constexpr anim::Motion kSettle = anim::Motion::tween(anim::Curve::CaEaseInOut, Millis{600});
constexpr anim::Motion kFollow = anim::Motion::tween(anim::Curve::EaseOut, Millis{120});

int pulsesFor(domain::Alert alert) {
  switch (alert) {
    case domain::Alert::NeedsYou:
      return 3;
    case domain::Alert::Done:
      return 1;
    case domain::Alert::None:
      return 0;
  }
  return 0;
}

}  // namespace

void BacklightController::alert(domain::Alert alert, Instant now) {
  const int pulses = pulsesFor(alert);
  if (pulses == 0 || nightActive_) {
    return;
  }
  if (pulseStart_) {
    const Millis elapsed = now - *pulseStart_;
    if (elapsed < kMergeWindow) {
      pulses_ = std::max(pulses_, pulses);  // a burst: one sequence, at the strongest alert
      return;
    }
    if (elapsed < anim::kBreathPeriod * pulses_) {
      // Still pulsing: carry on from the current breath instead of restarting mid-dip.
      const auto current = static_cast<int>(elapsed / anim::kBreathPeriod);
      pulses_ = std::max(pulses_, current + pulses);
      return;
    }
  }
  pulseStart_ = now;
  pulses_ = pulses;
}

float BacklightController::level(Phase phase, bool boardEmpty, std::optional<WallTime> wallTime,
                                 Instant now) {
  nightActive_ = wallTime && night_.contains(*wallTime);

  float target = day_;
  if (phase == Phase::Asleep) {
    target = std::min(day_, kAsleep);
  } else if (boardEmpty && phase != Phase::Unpaired && phase != Phase::Pairing) {
    target = day_ * kQuietFactor;
  }
  if (nightActive_) {
    target = std::min(target, nightLevel_);
  }
  if (followHand_) {
    resting_.animateTo(target, kFollow, now);
    followHand_ = false;
  } else if (target != resting_.target()) {
    resting_.animateTo(target, kSettle, now);
  }
  resting_.update(now);
  return resting_.value() * (1.0f - pulseDip(now));
}

float BacklightController::pulseDip(Instant now) const {
  if (!pulseStart_ || nightActive_) {
    return 0.0f;
  }
  const Millis elapsed = now - *pulseStart_;
  if (elapsed >= anim::kBreathPeriod * pulses_) {
    return 0.0f;
  }
  return kPulseDepth * anim::breathe(elapsed);
}

}  // namespace desk::app
