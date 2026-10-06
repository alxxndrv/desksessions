#pragma once

#include <cstdint>
#include <optional>

#include "desk/domain/command.hpp"

namespace desk::app {

/// Outbound side of the BLE link to the Mac.
class MacLink {
 public:
  virtual ~MacLink() = default;
  virtual void send(const domain::Command& command) = 0;
  /// Wipes the bond and starts advertising for a new pairing.
  virtual void forgetPairing() = 0;
};

/// The screen's backlight.
class Backlight {
 public:
  virtual ~Backlight() = default;
  /// `level` in [0, 1].
  virtual void set(float level) = 0;
};

/// Local wall-clock time, once known (the Mac sets it on every connect).
struct WallTime {
  std::uint8_t hour = 0;
  std::uint8_t minute = 0;
  friend constexpr bool operator==(WallTime, WallTime) = default;
};

}  // namespace desk::app
