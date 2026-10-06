#pragma once

#include <etl/string.h>
#include <etl/vector.h>

#include <cstddef>
#include <cstdint>

#include "desk/util/time.hpp"

namespace desk::domain {

inline constexpr std::size_t kMaxSessions = 16;
inline constexpr std::size_t kMaxNameBytes = 48;    // UTF-8
inline constexpr std::size_t kMaxDetailBytes = 12;  // UTF-8

/// Stable identifier the Mac assigns to a session for as long as it lives.
struct SessionId {
  std::uint32_t value = 0;
  friend constexpr bool operator==(SessionId, SessionId) = default;
};

enum class Agent : std::uint8_t { Claude, Codex };

/// Ordered by urgency: lower values sort first on the board.
enum class SessionState : std::uint8_t { NeedsYou, Done, Working };

struct Session {
  SessionId id;
  Agent agent = Agent::Claude;
  SessionState state = SessionState::Working;
  etl::string<kMaxNameBytes> name;
  etl::string<kMaxDetailBytes> detail;  // current tool, "perm", "?" …
  std::uint8_t subagents = 0;
  Millis inState{0};  // time spent in `state` when the snapshot was taken
};

/// Everything the Mac currently wants shown, in any order.
struct Snapshot {
  etl::vector<Session, kMaxSessions> sessions;
};

}  // namespace desk::domain
