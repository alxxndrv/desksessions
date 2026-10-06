#pragma once

#include <cstdint>

#include "desk/domain/session.hpp"

namespace desk::domain {

/// A request the clock sends back to the Mac.
struct Command {
  enum class Kind : std::uint8_t { Dismiss, Jump };

  Kind kind;
  SessionId session;

  friend constexpr bool operator==(const Command&, const Command&) = default;
};

}  // namespace desk::domain
