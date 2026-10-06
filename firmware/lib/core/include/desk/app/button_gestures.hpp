#pragma once

#include <array>
#include <cstdint>
#include <optional>

#include "desk/util/time.hpp"

namespace desk::app {

/// BOOT (GPIO0) moves the cursor up; Aux (GPIO21) moves it down.
enum class Button : std::uint8_t { Boot, Aux };

struct Gesture {
  enum class Kind : std::uint8_t {
    Down,       // finger landed: immediate feedback
    Press,      // released before the long-press threshold
    LongPress,  // held past the threshold (fires while still held)
    Chord,      // both buttons held for the re-pair time
  };

  Kind kind;
  Button button;  // meaningless for Chord

  friend constexpr bool operator==(const Gesture&, const Gesture&) = default;
};

/// Progress of a long press in flight, for the fill that sweeps across the row.
struct Hold {
  Button button;
  float progress;  // 0 → 1 over the long-press threshold
};

/// Turns raw button levels into gestures. Fed once per tick with the current levels.
class ButtonGestures {
 public:
  static constexpr Millis kDebounce{15};
  static constexpr Millis kLongPress{600};
  static constexpr Millis kChord{5'000};

  /// `bootDown` / `auxDown` are raw levels (true = pressed). Returns at most one gesture.
  std::optional<Gesture> update(bool bootDown, bool auxDown, Instant now);

  /// The long press currently filling up, if any.
  [[nodiscard]] std::optional<Hold> hold(Instant now) const;

 private:
  struct Key {
    bool raw = false;
    bool stable = false;
    Instant rawSince;
    Instant downSince;
    bool longFired = false;
  };

  /// Debounces one key. Returns true when its stable level changed.
  static bool debounce(Key& key, bool raw, Instant now);
  std::optional<Gesture> onEdge(Button button, Instant now);

  std::array<Key, 2> keys_{};
  bool chording_ = false;
  bool chordFired_ = false;
  Instant chordSince_;
};

}  // namespace desk::app
