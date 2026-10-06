#pragma once

#include <cstdint>
#include <optional>

#include "desk/anim/easing.hpp"
#include "desk/anim/moook.hpp"
#include "desk/anim/spring.hpp"
#include "desk/util/time.hpp"

namespace desk::anim {

/// How a channel travels to a new target.
struct Motion {
  enum class Kind : std::uint8_t { Spring, Tween, Moook };

  Kind kind = Kind::Spring;
  SpringParams spring = kSmooth;
  Curve curve = Curve::Linear;
  Millis duration{0};
  const MoookShape* shape = nullptr;

  [[nodiscard]] static constexpr Motion springy(SpringParams params) {
    return Motion{.kind = Kind::Spring, .spring = params};
  }
  [[nodiscard]] static constexpr Motion tween(Curve curve, Millis duration) {
    return Motion{.kind = Kind::Tween, .curve = curve, .duration = duration};
  }
  [[nodiscard]] static constexpr Motion moook(const MoookShape& shape) {
    return Motion{.kind = Kind::Moook, .shape = &shape};
  }
};

/// Settle-only moook: what an interrupted moook plays from wherever it was (Pebble menu_layer).
inline constexpr MoookShape kMoookSettle{{}, kMoookOut, 1};

/// One animated value. Every change of target starts from the value currently on screen, with
/// its current velocity, so any motion can be interrupted or reversed without a jump:
///  - springs keep their velocity (Apple);
///  - an interrupted tween continues with ease-out (Pebble scroll_layer);
///  - an interrupted moook plays only its settle (Pebble menu_layer).
class Channel {
 public:
  explicit Channel(float value = 0.0f) { snapTo(value); }

  /// Starts travelling to `target`. Re-requesting the current target with the same kind of
  /// motion leaves the motion in flight alone.
  void animateTo(float target, const Motion& motion, Instant now);

  /// Queues one follow-up leg, started the moment the current motion finishes. For scripted
  /// multi-part gestures (nudge out and back, stretch then jump). Any animateTo/snapTo drops it.
  void then(float target, const Motion& motion) { next_ = Leg{target, motion}; }

  /// Jumps to `value` with no motion. For values driven directly, such as a finger's hold.
  void snapTo(float value);

  /// Adds velocity (units/s) for a spring-driven nudge or pop.
  void kick(float velocity, Instant now, SpringParams params = kBouncy);

  /// Advances to `now`. Call once per frame before reading.
  void update(Instant now);

  [[nodiscard]] float value() const { return value_; }
  [[nodiscard]] float velocity() const { return velocity_; }
  [[nodiscard]] float target() const { return target_; }
  [[nodiscard]] bool animating() const { return animating_; }

 private:
  struct Leg {
    float target;
    Motion motion;
  };

  [[nodiscard]] float sample(Instant at) const;
  void finish(Instant now);

  Motion motion_{};
  float value_ = 0.0f;
  float velocity_ = 0.0f;
  float from_ = 0.0f;
  float target_ = 0.0f;
  Instant start_;
  Instant lastUpdate_;
  float stepBacklog_ = 0.0f;  // seconds of spring time not yet simulated
  Spring spring_;
  bool animating_ = false;
  std::optional<Leg> next_;
};

}  // namespace desk::anim
