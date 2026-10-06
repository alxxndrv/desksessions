#pragma once

#include <cstdint>
#include <optional>

#include "desk/anim/channel.hpp"
#include "desk/app/connection.hpp"
#include "desk/app/ports.hpp"
#include "desk/domain/session_board.hpp"
#include "desk/util/time.hpp"

namespace desk::app {

/// Quiet hours: the backlight stays low and alerts don't pulse.
struct NightHours {
  std::uint8_t startHour = 23;
  std::uint8_t endHour = 8;

  [[nodiscard]] constexpr bool contains(WallTime t) const {
    return startHour <= endHour ? (t.hour >= startHour && t.hour < endHour)
                                : (t.hour >= startHour || t.hour < endHour);
  }
};

/// Decides how bright the screen is: a resting level for the situation, dipped by alert pulses.
class BacklightController {
 public:
  static constexpr float kDay = 1.0f;
  static constexpr float kQuiet = 0.6f;
  static constexpr float kAsleep = 0.15f;
  static constexpr float kNight = 0.10f;
  static constexpr Millis kMergeWindow{5'000};

  void alert(domain::Alert alert, Instant now);

  /// Brightness in [0, 1] for this frame.
  float level(Phase phase, bool boardEmpty, std::optional<WallTime> wallTime, Instant now);

  void setNightHours(NightHours hours) { night_ = hours; }

 private:
  [[nodiscard]] float pulseDip(Instant now) const;

  NightHours night_;
  anim::Channel resting_{kDay};
  bool nightActive_ = false;
  std::optional<Instant> pulseStart_;
  int pulses_ = 0;
};

}  // namespace desk::app
