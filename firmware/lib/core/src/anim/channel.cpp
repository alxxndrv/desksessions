#include "desk/anim/channel.hpp"

#include <algorithm>
#include <chrono>

namespace desk::anim {
namespace {

// Longest gap simulated in one update; a hitch longer than this slows motion rather than
// making springs explode or tweens skip their ending.
constexpr float kMaxCatchUp = 0.25f;

float seconds(Millis duration) {
  return std::chrono::duration_cast<std::chrono::duration<float>>(duration).count();
}

}  // namespace

void Channel::animateTo(float target, const Motion& motion, Instant now) {
  update(now);
  next_.reset();
  if (animating_ && target == target_ && motion.kind == motion_.kind) {
    return;
  }
  const bool interrupted = animating_;

  motion_ = motion;
  from_ = value_;
  target_ = target;
  start_ = now;
  animating_ = value_ != target || velocity_ != 0.0f;

  switch (motion.kind) {
    case Motion::Kind::Spring:
      spring_ = Spring{.value = value_, .velocity = velocity_, .target = target};
      stepBacklog_ = 0.0f;
      break;
    case Motion::Kind::Tween:
      if (interrupted) {
        motion_.curve = Curve::EaseOut;
      }
      break;
    case Motion::Kind::Moook:
      if (interrupted) {
        motion_.shape = &kMoookSettle;
      }
      break;
  }
}

void Channel::snapTo(float value) {
  next_.reset();
  value_ = value;
  from_ = value;
  target_ = value;
  velocity_ = 0.0f;
  spring_.snapTo(value);
  animating_ = false;
}

void Channel::kick(float velocity, Instant now, SpringParams params) {
  update(now);
  const float target = animating_ ? target_ : value_;
  velocity_ += velocity;
  animateTo(target, Motion::springy(params), now);
  spring_.velocity = velocity_;
  animating_ = true;
}

void Channel::update(Instant now) {
  const float dt = std::min(kMaxCatchUp, seconds(std::max(Millis{0}, now - lastUpdate_)));
  lastUpdate_ = now;
  if (!animating_) {
    return;
  }

  switch (motion_.kind) {
    case Motion::Kind::Spring:
      stepBacklog_ += dt;
      while (stepBacklog_ >= kSpringStep) {
        step(spring_, motion_.spring);
        stepBacklog_ -= kSpringStep;
      }
      value_ = spring_.value;
      velocity_ = spring_.velocity;
      if (spring_.settled()) {
        finish(now);
      }
      break;

    case Motion::Kind::Tween:
    case Motion::Kind::Moook: {
      const Millis length =
          motion_.kind == Motion::Kind::Tween ? motion_.duration : duration(*motion_.shape);
      if (now - start_ >= length) {
        finish(now);
        return;
      }
      value_ = sample(now);
      // Velocity over the last millisecond, so a spring taking over carries the momentum on.
      velocity_ = (value_ - sample(now - Millis{1})) * 1000.0f;
      break;
    }
  }
}

void Channel::finish(Instant now) {
  value_ = target_;
  from_ = target_;
  velocity_ = 0.0f;
  spring_.snapTo(target_);
  animating_ = false;
  if (next_) {
    const Leg leg = *next_;
    animateTo(leg.target, leg.motion, now);
  }
}

float Channel::sample(Instant at) const {
  const Millis elapsed = std::max(Millis{0}, at - start_);
  if (motion_.kind == Motion::Kind::Moook) {
    return moook(*motion_.shape, from_, target_, elapsed);
  }
  return lerp(from_, target_, ease(motion_.curve, progress(elapsed, motion_.duration)));
}

}  // namespace desk::anim
