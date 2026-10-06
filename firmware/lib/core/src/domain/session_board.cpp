#include "desk/domain/session_board.hpp"

#include <algorithm>

namespace desk::domain {
namespace {

bool sortsBefore(const Session& a, const Session& b) {
  if (a.state != b.state) {
    return a.state < b.state;
  }
  if (a.inState != b.inState) {
    return a.inState > b.inState;  // longest-waiting first
  }
  return a.id.value < b.id.value;
}

const Session* find(std::span<const Session> rows, SessionId id) {
  const auto it =
      std::find_if(rows.begin(), rows.end(), [id](const Session& s) { return s.id == id; });
  return it == rows.end() ? nullptr : &*it;
}

Alert alertFor(const Session& now, const Session* before) {
  const bool changed = before == nullptr || before->state != now.state;
  if (!changed) {
    return Alert::None;
  }
  switch (now.state) {
    case SessionState::NeedsYou:
      return Alert::NeedsYou;
    case SessionState::Done:
      return Alert::Done;
    case SessionState::Working:
      return Alert::None;
  }
  return Alert::None;
}

}  // namespace

Alert SessionBoard::apply(const Snapshot& snapshot, Instant now) {
  forgetStaleDismissals(snapshot);

  const std::optional<SessionId> selectedId =
      selected() != nullptr ? std::optional{selected()->id} : std::nullopt;

  etl::vector<Session, kMaxSessions> next;
  Alert alert = Alert::None;
  for (const Session& session : snapshot.sessions) {
    if (isDismissed(session)) {
      continue;
    }
    alert = std::max(alert, alertFor(session, find(rows_, session.id)));
    next.push_back(session);
  }
  std::sort(next.begin(), next.end(), sortsBefore);

  rows_ = next;
  receivedAt_ = now;
  frozenAt_.reset();

  if (selectedId) {
    const auto it = std::find_if(rows_.begin(), rows_.end(),
                                 [&](const Session& s) { return s.id == *selectedId; });
    if (it != rows_.end()) {
      cursor_ = static_cast<std::size_t>(it - rows_.begin());
      return alert;
    }
  }
  cursor_ = rows_.empty() ? 0 : std::min(cursor_, rows_.size() - 1);
  return alert;
}

StateCounts SessionBoard::counts() const {
  StateCounts counts;
  for (const Session& row : rows_) {
    switch (row.state) {
      case SessionState::NeedsYou:
        ++counts.needsYou;
        break;
      case SessionState::Done:
        ++counts.done;
        break;
      case SessionState::Working:
        ++counts.working;
        break;
    }
  }
  return counts;
}

const Session* SessionBoard::selected() const {
  return cursor_ < rows_.size() ? &rows_[cursor_] : nullptr;
}

CursorMoved SessionBoard::moveCursor(CursorMove move) {
  if (rows_.size() < 2) {
    return {};
  }
  const std::size_t last = rows_.size() - 1;
  if (move == CursorMove::Up) {
    const bool wrapped = cursor_ == 0;
    cursor_ = wrapped ? last : cursor_ - 1;
    return {.moved = true, .wrapped = wrapped};
  }
  const bool wrapped = cursor_ == last;
  cursor_ = wrapped ? 0 : cursor_ + 1;
  return {.moved = true, .wrapped = wrapped};
}

void SessionBoard::resetCursor() { cursor_ = 0; }

std::optional<Command> SessionBoard::dismissSelected() {
  const Session* row = selected();
  if (row == nullptr || row->state != SessionState::Done || dismissed_.full()) {
    return std::nullopt;
  }
  const Command command{.kind = Command::Kind::Dismiss, .session = row->id};
  dismissed_.push_back({row->id});
  rows_.erase(rows_.begin() + static_cast<std::ptrdiff_t>(cursor_));
  cursor_ = rows_.empty() ? 0 : std::min(cursor_, rows_.size() - 1);
  return command;
}

std::optional<Command> SessionBoard::jumpToSelected() const {
  const Session* row = selected();
  if (row == nullptr) {
    return std::nullopt;
  }
  return Command{.kind = Command::Kind::Jump, .session = row->id};
}

Millis SessionBoard::timeInState(const Session& row, Instant now) const {
  const Instant until = frozenAt_.value_or(now);
  return row.inState + std::max(Millis{0}, until - receivedAt_);
}

bool SessionBoard::isDismissed(const Session& session) const {
  return session.state == SessionState::Done &&
         std::any_of(dismissed_.begin(), dismissed_.end(),
                     [&](const Dismissal& d) { return d.id == session.id; });
}

void SessionBoard::forgetStaleDismissals(const Snapshot& snapshot) {
  // A dismissal lasts while the Mac still reports that session as DONE.
  const auto stale = [&](const Dismissal& d) {
    const auto& sessions = snapshot.sessions;
    const auto it = std::find_if(sessions.begin(), sessions.end(),
                                 [&](const Session& s) { return s.id == d.id; });
    return it == sessions.end() || it->state != SessionState::Done;
  };
  dismissed_.erase(std::remove_if(dismissed_.begin(), dismissed_.end(), stale), dismissed_.end());
}

}  // namespace desk::domain
