#pragma once

#include <algorithm>
#include <cstdint>

#include "desk/domain/session.hpp"

namespace desk::ui {

struct Rgb {
  std::uint8_t r;
  std::uint8_t g;
  std::uint8_t b;
};

[[nodiscard]] constexpr Rgb mix(Rgb from, Rgb to, float t) {
  t = std::clamp(t, 0.0f, 1.0f);
  const auto channel = [t](std::uint8_t a, std::uint8_t b) {
    return static_cast<std::uint8_t>(static_cast<float>(a) +
                                     (static_cast<float>(b) - static_cast<float>(a)) * t + 0.5f);
  };
  return Rgb{channel(from.r, to.r), channel(from.g, to.g), channel(from.b, to.b)};
}

/// 0xRRGGBB, the colour form LovyanGFX takes as a uint32_t.
[[nodiscard]] constexpr std::uint32_t hex(Rgb c) {
  return (static_cast<std::uint32_t>(c.r) << 16) | (static_cast<std::uint32_t>(c.g) << 8) | c.b;
}

namespace theme {

// Dark, warm-neutral surfaces; Apple system colours (dark appearance) for state.
inline constexpr Rgb kBackground{11, 11, 13};
inline constexpr Rgb kSurface{36, 37, 42};
inline constexpr Rgb kLifted{52, 53, 60};
inline constexpr Rgb kHairline{40, 41, 46};
inline constexpr Rgb kText{238, 238, 242};
inline constexpr Rgb kTextDim{150, 152, 160};
inline constexpr Rgb kTextFaint{92, 94, 102};

inline constexpr Rgb kNeedsYou{255, 69, 58};
inline constexpr Rgb kDone{255, 214, 10};
inline constexpr Rgb kWorking{48, 209, 88};
inline constexpr Rgb kJump{10, 132, 255};

inline constexpr Rgb kClaude{217, 119, 87};
inline constexpr Rgb kCodex{214, 216, 226};

[[nodiscard]] constexpr Rgb stateColor(domain::SessionState state) {
  switch (state) {
    case domain::SessionState::NeedsYou:
      return kNeedsYou;
    case domain::SessionState::Done:
      return kDone;
    case domain::SessionState::Working:
      return kWorking;
  }
  return kWorking;
}

[[nodiscard]] constexpr Rgb agentColor(domain::Agent agent) {
  return agent == domain::Agent::Claude ? kClaude : kCodex;
}

}  // namespace theme
}  // namespace desk::ui
