#include <gtest/gtest.h>

#include "desk/domain/session_board.hpp"

using namespace desk;
using namespace desk::domain;

namespace {

Session session(std::uint32_t id, SessionState state, int secondsInState = 0) {
  Session s;
  s.id = SessionId{id};
  s.state = state;
  s.name = "session";
  s.inState = Millis{secondsInState * 1000};
  return s;
}

Snapshot snapshotOf(std::initializer_list<Session> sessions) {
  Snapshot snapshot;
  for (const Session& s : sessions) {
    snapshot.sessions.push_back(s);
  }
  return snapshot;
}

std::vector<std::uint32_t> ids(const SessionBoard& board) {
  std::vector<std::uint32_t> out;
  for (const Session& row : board.rows()) {
    out.push_back(row.id.value);
  }
  return out;
}

constexpr Instant kT0{Millis{1'000}};

}  // namespace

TEST(SessionBoard, SortsByUrgencyThenLongestWaiting) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Working, 50), session(2, SessionState::Done, 10),
                          session(3, SessionState::NeedsYou, 5), session(4, SessionState::Done, 90),
                          session(5, SessionState::NeedsYou, 60)}),
              kT0);
  EXPECT_EQ(ids(board), (std::vector<std::uint32_t>{5, 3, 4, 2, 1}));

  const StateCounts counts = board.counts();
  EXPECT_EQ(counts.needsYou, 2);
  EXPECT_EQ(counts.done, 2);
  EXPECT_EQ(counts.working, 1);
}

TEST(SessionBoard, CursorFollowsTheSameSessionAcrossReorders) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Working)}), kT0);
  board.moveCursor(CursorMove::Down);
  ASSERT_EQ(board.selected()->id.value, 2u);

  // Session 7 jumps in above it: the cursor index changes, the selection does not.
  board.apply(snapshotOf({session(7, SessionState::NeedsYou), session(1, SessionState::Done),
                          session(2, SessionState::Working)}),
              kT0);
  EXPECT_EQ(board.cursor(), 2u);
  EXPECT_EQ(board.selected()->id.value, 2u);
}

TEST(SessionBoard, CursorClampsWhenItsSessionDisappears) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Working)}), kT0);
  board.moveCursor(CursorMove::Down);
  board.apply(snapshotOf({session(1, SessionState::Done)}), kT0);
  EXPECT_EQ(board.cursor(), 0u);
  board.apply(Snapshot{}, kT0);
  EXPECT_EQ(board.selected(), nullptr);
}

TEST(SessionBoard, CursorWrapsAndReportsIt) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Done),
                          session(3, SessionState::Done)}),
              kT0);
  EXPECT_TRUE(board.moveCursor(CursorMove::Up).wrapped);
  EXPECT_EQ(board.cursor(), 2u);
  const CursorMoved down = board.moveCursor(CursorMove::Down);
  EXPECT_TRUE(down.moved);
  EXPECT_TRUE(down.wrapped);
  EXPECT_EQ(board.cursor(), 0u);
  EXPECT_FALSE(board.moveCursor(CursorMove::Down).wrapped);
}

TEST(SessionBoard, SingleRowCursorDoesNotMove) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Done)}), kT0);
  EXPECT_FALSE(board.moveCursor(CursorMove::Down).moved);
}

TEST(SessionBoard, AlertsOnlyOnEnteringAttentionStates) {
  SessionBoard board;
  EXPECT_EQ(board.apply(snapshotOf({session(1, SessionState::Working)}), kT0), Alert::None);
  EXPECT_EQ(board.apply(snapshotOf({session(1, SessionState::Done)}), kT0), Alert::Done);
  EXPECT_EQ(board.apply(snapshotOf({session(1, SessionState::Done)}), kT0), Alert::None);
  EXPECT_EQ(
      board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::NeedsYou)}),
                  kT0),
      Alert::NeedsYou);
}

TEST(SessionBoard, DismissHidesDoneUntilItChangesState) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Working)}), kT0);

  const auto command = board.dismissSelected();
  ASSERT_TRUE(command.has_value());
  EXPECT_EQ(*command, (Command{.kind = Command::Kind::Dismiss, .session = SessionId{1}}));
  EXPECT_EQ(ids(board), (std::vector<std::uint32_t>{2}));

  // The Mac hasn't processed the dismissal yet and still reports it DONE: stays hidden.
  board.apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Working)}), kT0);
  EXPECT_EQ(ids(board), (std::vector<std::uint32_t>{2}));

  // You prompted it again: it is back.
  board.apply(snapshotOf({session(1, SessionState::Working), session(2, SessionState::Working)}),
              kT0);
  EXPECT_EQ(ids(board).size(), 2u);

  // Done again later: a fresh DONE shows and alerts.
  EXPECT_EQ(board.apply(snapshotOf({session(1, SessionState::Done)}), kT0), Alert::Done);
  EXPECT_EQ(ids(board), (std::vector<std::uint32_t>{1}));
}

TEST(SessionBoard, OnlyDoneRowsCanBeDismissed) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::NeedsYou), session(2, SessionState::Working)}),
              kT0);
  EXPECT_FALSE(board.dismissSelected().has_value());
  board.moveCursor(CursorMove::Down);
  EXPECT_FALSE(board.dismissSelected().has_value());
  EXPECT_EQ(ids(board).size(), 2u);
}

TEST(SessionBoard, JumpTargetsSelection) {
  SessionBoard board;
  EXPECT_FALSE(board.jumpToSelected().has_value());
  board.apply(snapshotOf({session(1, SessionState::NeedsYou), session(2, SessionState::Working)}),
              kT0);
  board.moveCursor(CursorMove::Down);
  EXPECT_EQ(board.jumpToSelected()->session.value, 2u);
}

TEST(SessionBoard, TimeInStateCountsLocallyAndFreezes) {
  SessionBoard board;
  board.apply(snapshotOf({session(1, SessionState::Working, 10)}), kT0);
  const Session& row = board.rows()[0];
  EXPECT_EQ(board.timeInState(row, kT0 + Millis{5'000}), Millis{15'000});

  board.freezeTime(kT0 + Millis{2'000});
  EXPECT_EQ(board.timeInState(row, kT0 + Millis{60'000}), Millis{12'000});
}

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
