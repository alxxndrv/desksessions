#pragma once

#include <etl/vector.h>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "desk/domain/command.hpp"
#include "desk/domain/session.hpp"
#include "desk/util/time.hpp"

namespace desk::domain {

/// Something that should catch the eye after a snapshot.
enum class Alert : std::uint8_t { None, Done, NeedsYou };

struct StateCounts {
  std::uint8_t needsYou = 0;
  std::uint8_t done = 0;
  std::uint8_t working = 0;
};

enum class CursorMove : std::uint8_t { Up, Down };

/// Result of moving the cursor: whether it wrapped past an end of the list.
struct CursorMoved {
  bool moved = false;
  bool wrapped = false;
};

/// The aggregate behind the clock: the sorted list of visible sessions plus the cursor.
///
/// Invariants:
///  - rows are sorted NEEDS YOU → DONE → WORKING, longest-waiting first within a state;
///  - the cursor stays on the same session across snapshots while that session is visible;
///  - a dismissed DONE session stays hidden until the Mac reports it in another state.
class SessionBoard {
 public:
  /// Replaces the board's contents. `now` timestamps the snapshot so time-in-state keeps
  /// counting locally between snapshots. Returns the strongest alert the change deserves.
  Alert apply(const Snapshot& snapshot, Instant now);

  [[nodiscard]] std::span<const Session> rows() const { return rows_; }
  [[nodiscard]] bool empty() const { return rows_.empty(); }
  [[nodiscard]] StateCounts counts() const;

  [[nodiscard]] std::size_t cursor() const { return cursor_; }
  [[nodiscard]] const Session* selected() const;
  CursorMoved moveCursor(CursorMove move);
  void resetCursor();

  /// Hides the selected session if it is DONE. Returns the command to tell the Mac, or nothing
  /// when the selected row cannot be dismissed.
  std::optional<Command> dismissSelected();
  [[nodiscard]] std::optional<Command> jumpToSelected() const;

  /// Time `row` has spent in its state at `now`, counting locally since the snapshot arrived.
  [[nodiscard]] Millis timeInState(const Session& row, Instant now) const;

  /// Stops time-in-state from advancing (the Mac is unreachable) or resumes it.
  void freezeTime(std::optional<Instant> at) { frozenAt_ = at; }

 private:
  struct Dismissal {
    SessionId id;
  };

  [[nodiscard]] bool isDismissed(const Session& session) const;
  void forgetStaleDismissals(const Snapshot& snapshot);

  etl::vector<Session, kMaxSessions> rows_;
  etl::vector<Dismissal, kMaxSessions> dismissed_;
  std::size_t cursor_ = 0;
  Instant receivedAt_;
  std::optional<Instant> frozenAt_;
};

}  // namespace desk::domain
