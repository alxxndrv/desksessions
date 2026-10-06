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

/// Landscape. Flip to 3 if the clock's case holds the panel the other way up.
inline constexpr std::uint8_t kRotation = 1;

}  // namespace desk::board
