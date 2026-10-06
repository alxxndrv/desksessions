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
  static constexpr float kQuietFactor = 0.6f;  // empty board: this share of the day level
  static constexpr float kAsleep = 0.15f;      // Mac away for minutes: at most this
  static constexpr Millis kMergeWindow{5'000};

  void alert(domain::Alert alert, Instant now);

  /// Brightness in [0, 1] for this frame.
  float level(Phase phase, bool boardEmpty, std::optional<WallTime> wallTime, Instant now);

  void setNightHours(NightHours hours) { night_ = hours; }

  /// Levels chosen in the Mac app, in percent.
  void setBrightness(std::uint8_t day, std::uint8_t night) {
    const float nextDay = static_cast<float>(day) / 100.0f;
    const float nextNight = static_cast<float>(night) / 100.0f;
    followHand_ = followHand_ || nextDay != day_ || nextNight != nightLevel_;
    day_ = nextDay;
    nightLevel_ = nextNight;
  }
  [[nodiscard]] float day() const { return day_; }
  [[nodiscard]] float night() const { return nightLevel_; }

 private:
  [[nodiscard]] float pulseDip(Instant now) const;

  NightHours night_;
  float day_ = 1.0f;
  float nightLevel_ = 0.10f;
  bool followHand_ = false;  // the user is dragging a slider: track it closely
  anim::Channel resting_{1.0f};
  bool nightActive_ = false;
  std::optional<Instant> pulseStart_;
  int pulses_ = 0;
};

}  // namespace desk::app
