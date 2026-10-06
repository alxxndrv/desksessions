#pragma once

#include <Arduino.h>

#include <algorithm>
#include <cmath>
#include <cstdint>

#include "board.hpp"
#include "desk/app/ports.hpp"

namespace desk::esp32 {

/// Backlight on a 12-bit LEDC channel. Levels are perceived lightness, turned into LED duty
/// with the CIE 1931 curve so fades look even all the way down; 4096 steps keep the low end
/// smooth where an 8-bit PWM visibly stairs.
class PwmBacklight final : public app::Backlight {
 public:
  static constexpr std::uint32_t kFrequency = 12'000;  // well above flicker, inside 80 MHz / 2¹²
  static constexpr std::uint8_t kBits = 12;
  static constexpr std::uint32_t kMaxDuty = (1u << kBits) - 1;
  static constexpr float kFloor = 0.04f;  // lightness at level 0: dim, never fully dark

  void begin() {
    ledcAttach(board::kTftBacklight, kFrequency, kBits);
    ledcWrite(board::kTftBacklight, 0);
  }

  void set(float level) override {
    const float lightness = kFloor + (1.0f - kFloor) * std::clamp(level, 0.0f, 1.0f);
    const auto duty = static_cast<std::uint32_t>(std::lround(luminance(lightness) * kMaxDuty));
    if (duty != last_) {
      ledcWrite(board::kTftBacklight, duty);
      last_ = duty;
    }
  }

 private:
  /// CIE 1931: relative luminance for lightness L* (both 0–1).
  static float luminance(float lightness) {
    return lightness <= 0.08f ? lightness / 9.033f : std::pow((lightness + 0.16f) / 1.16f, 3.0f);
  }

  std::uint32_t last_ = UINT32_MAX;
};

}  // namespace desk::esp32
