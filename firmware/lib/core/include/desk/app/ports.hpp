#pragma once

#include <cstdint>
#include <optional>

#include "desk/domain/command.hpp"

namespace desk::app {

/// Outbound: commands the clock sends back to the Mac.
class CommandLink {
 public:
  virtual ~CommandLink() = default;
  virtual void send(const domain::Command& command) = 0;
};

/// Local wall-clock time, once known (the Mac sets it on every connect).
struct WallTime {
  std::uint8_t hour = 0;
  std::uint8_t minute = 0;
  friend constexpr bool operator==(WallTime, WallTime) = default;
};

}  // namespace desk::app
