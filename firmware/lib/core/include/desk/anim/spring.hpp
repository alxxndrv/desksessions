#pragma once

#include <numbers>

namespace desk::anim {

/// Damped spring with unit mass.
struct SpringParams {
  float stiffness;
  float damping;
};

/// Apple's perceptual spring parameters (WWDC23 "Animate with springs"): `duration` is the
/// period of the undamped oscillation in seconds, `bounce` is 0 for critical damping and
/// approaches 1 for no damping. Negative bounce (overdamped) is not supported here.
[[nodiscard]] constexpr SpringParams springFrom(float duration, float bounce) {
  const float omega = 2.0f * std::numbers::pi_v<float> / duration;
  return SpringParams{omega * omega, 4.0f * std::numbers::pi_v<float> * (1.0f - bounce) / duration};
}

// SwiftUI presets (iOS 17 / macOS 14 and later).
inline constexpr SpringParams kSmooth = springFrom(0.5f, 0.0f);
inline constexpr SpringParams kSnappy = springFrom(0.5f, 0.15f);
inline constexpr SpringParams kBouncy = springFrom(0.5f, 0.30f);

/// Fixed physics step. Springs are only ever advanced by this amount so motion is identical on
/// the device, in the simulator and in tests, whatever the frame rate.
inline constexpr float kSpringStep = 1.0f / 120.0f;

struct Spring {
  float value = 0.0f;
  float velocity = 0.0f;
  float target = 0.0f;

  /// Jumps straight to `position` with no motion.
  void snapTo(float position) {
    value = position;
    target = position;
    velocity = 0.0f;
  }

  [[nodiscard]] bool settled() const;
};

/// Advances `spring` by one kSpringStep (semi-implicit Euler).
void step(Spring& spring, const SpringParams& params);

}  // namespace desk::anim
