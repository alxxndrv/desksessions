#include "desk/app/connection.hpp"

namespace desk::app {

void Connection::onEvent(const LinkEvent& event, Instant now) {
  switch (event.kind) {
    case LinkEvent::Kind::Unpaired:
    case LinkEvent::Kind::Connected:
      lostAt_.reset();
      break;
    case LinkEvent::Kind::Passkey:
      passkey_ = event.passkey;
      lostAt_.reset();
      break;
    case LinkEvent::Kind::Disconnected:
      if (!lostAt_) {
        lostAt_ = now;  // repeated reports don't restart the grace period
      }
      break;
  }
  last_ = event.kind;
}

Phase Connection::phase(Instant now) const {
  switch (last_) {
    case LinkEvent::Kind::Unpaired:
      return Phase::Unpaired;
    case LinkEvent::Kind::Passkey:
      return Phase::Pairing;
    case LinkEvent::Kind::Connected:
      return Phase::Online;
    case LinkEvent::Kind::Disconnected:
      break;
  }
  const Millis lost = now - lostAt_.value_or(now);
  if (lost >= kAsleepAfter) {
    return Phase::Asleep;
  }
  return lost >= kOfflineAfter ? Phase::Offline : Phase::Reconnecting;
}

}  // namespace desk::app
