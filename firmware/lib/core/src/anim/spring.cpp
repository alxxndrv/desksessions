#include "desk/anim/spring.hpp"

#include <cmath>

namespace desk::anim {

bool Spring::settled() const {
  constexpr float kPositionTolerance = 0.01f;
  constexpr float kVelocityTolerance = 0.05f;
  return std::fabs(value - target) < kPositionTolerance && std::fabs(velocity) < kVelocityTolerance;
}

void step(Spring& spring, const SpringParams& params) {
  if (spring.settled()) {
    spring.snapTo(spring.target);
    return;
  }
  const float displacement = spring.value - spring.target;
  const float acceleration = -params.stiffness * displacement - params.damping * spring.velocity;
  spring.velocity += acceleration * kSpringStep;
  spring.value += spring.velocity * kSpringStep;
}

}  // namespace desk::anim
