#pragma once

#include <cstdint>
#include <optional>

#include "desk/app/ports.hpp"
#include "desk/util/time.hpp"

namespace desk::app {

/// Time of day, set by the Mac on connect and carried forward on the monotonic clock.
class WallClock {
 public:
  /// The Mac says it is `secondsOfDay` past local midnight at `now`.
  void set(std::uint32_t secondsOfDay, Instant now) {
    base_ = Millis{static_cast<Millis::rep>(secondsOfDay % 86'400) * 1000};
    at_ = now;
  }

  [[nodiscard]] std::optional<WallTime> now(Instant now) const {
    if (!base_) {
      return std::nullopt;
    }
    const auto ms = (base_->count() + (now - at_).count()) % 86'400'000;
    const auto minutes = ms / 60'000;
    return WallTime{static_cast<std::uint8_t>(minutes / 60),
                    static_cast<std::uint8_t>(minutes % 60)};
  }

 private:
  std::optional<Millis> base_;
  Instant at_;
};

}  // namespace desk::app
