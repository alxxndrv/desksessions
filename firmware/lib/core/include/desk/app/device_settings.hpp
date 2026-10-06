#pragma once

#include <cstdint>

namespace desk::app {

/// How the clock sits on the desk. Set from the Mac app; the device keeps the last values.
struct DeviceSettings {
  bool flipScreen = false;            // turn the picture 180°
  bool swapButtons = false;           // BOOT moves down, GPIO21 up
  std::uint8_t dayBrightness = 100;   // percent
  std::uint8_t nightBrightness = 10;  // percent, during quiet hours

  friend constexpr bool operator==(const DeviceSettings&, const DeviceSettings&) = default;
};

}  // namespace desk::app
