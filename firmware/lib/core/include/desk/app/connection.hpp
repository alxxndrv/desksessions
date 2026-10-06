#pragma once

#include <cstdint>
#include <optional>

#include "desk/util/time.hpp"

namespace desk::app {

/// What the BLE adapter reports about the link to the Mac.
struct LinkEvent {
  enum class Kind : std::uint8_t {
    Unpaired,      // no bond: advertising for a Mac to pair
    Passkey,       // pairing in progress: show `passkey`
    Connected,     // bonded Mac connected and encrypted
    Disconnected,  // bonded Mac went away
  };

  Kind kind;
  std::uint32_t passkey = 0;
};

/// Where the connection stands, with the grace periods from the spec applied.
enum class Phase : std::uint8_t {
  Unpaired,
  Pairing,
  Online,
  Reconnecting,  // lost the Mac under 10 s ago: look online, retry quietly
  Offline,       // 10 s – 5 min: dim the list, timers frozen
  Asleep,        // over 5 min: sleepy face
};

class Connection {
 public:
  static constexpr Millis kOfflineAfter{10'000};
  static constexpr Millis kAsleepAfter{5 * 60'000};

  void onEvent(const LinkEvent& event, Instant now);

  [[nodiscard]] Phase phase(Instant now) const;
  [[nodiscard]] std::uint32_t passkey() const { return passkey_; }
  /// When the Mac was lost, while it is.
  [[nodiscard]] std::optional<Instant> lostAt() const { return lostAt_; }

 private:
  LinkEvent::Kind last_ = LinkEvent::Kind::Unpaired;
  std::uint32_t passkey_ = 0;
  std::optional<Instant> lostAt_;
};

}  // namespace desk::app
