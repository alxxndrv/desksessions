#pragma once

#include <cstdint>

/// Heltec Vision Master T190 wiring (from Heltec's schematic, as used by Meshtastic's variant).
namespace desk::board {

inline constexpr std::uint8_t kBootButton = 0;
inline constexpr std::uint8_t kAuxButton = 21;

inline constexpr std::uint8_t kVextEnable = 5;  // HIGH powers the external rail
inline constexpr std::uint8_t kTftPower = 7;    // LOW powers the panel

inline constexpr std::uint8_t kTftSck = 38;
inline constexpr std::uint8_t kTftMosi = 48;
inline constexpr std::uint8_t kTftDc = 47;
inline constexpr std::uint8_t kTftCs = 39;
inline constexpr std::uint8_t kTftReset = 40;
inline constexpr std::uint8_t kTftBacklight = 17;  // HIGH = on, PWM for brightness

/// The panel is driven in its native portrait order (so writes follow its scan); the frame
/// buffer itself is rotated to landscape. 3 is the right way up in the Moon Phase Clock case.
inline constexpr std::uint8_t kCanvasRotation = 3;
inline constexpr std::uint8_t kCanvasRotationFlipped = 1;

}  // namespace desk::board
