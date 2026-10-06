#pragma once

namespace desk::scene::layout {

inline constexpr float kScreenWidth = 320.0f;
inline constexpr float kScreenHeight = 170.0f;
inline constexpr float kHeaderHeight = 20.0f;
inline constexpr float kRowHeight = 30.0f;
inline constexpr float kViewportHeight = kScreenHeight - kHeaderHeight;  // five rows

inline constexpr float kEnterOffset = 48.0f;         // rows slide in from the right
inline constexpr float kExitOffset = -kScreenWidth;  // and leave to the left
inline constexpr float kNudge = 5.0f;                // Pebble action-bar press offset
inline constexpr float kWrapStretch = 6.0f;          // rubber band before wrapping around
inline constexpr float kHoldLift = 0.6f;             // row brightening at full hold

}  // namespace desk::scene::layout
