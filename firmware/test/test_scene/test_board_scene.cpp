#include <gtest/gtest.h>

#include "desk/anim/marquee.hpp"
#include "desk/scene/board_scene.hpp"
#include "desk/scene/layout.hpp"

using namespace desk;
using namespace desk::domain;
using namespace desk::scene;

namespace {

Session session(std::uint32_t id, SessionState state) {
  Session s;
  s.id = SessionId{id};
  s.state = state;
  return s;
}

Snapshot snapshotOf(std::initializer_list<Session> sessions) {
  Snapshot snapshot;
  for (const Session& s : sessions) {
    snapshot.sessions.push_back(s);
  }
  return snapshot;
}

class SceneTest : public ::testing::Test {
 protected:
  void apply(const Snapshot& snapshot) {
    board.apply(snapshot, now);
    scene.sync(board, now);
  }
  void run(int ms) {
    for (int t = 0; t < ms; t += 16) {
      now = now + Millis{16};
      scene.update(now);
    }
  }
  const RowSlot* slot(std::uint32_t id) const {
    for (const RowSlot& s : scene.slots()) {
      if (s.session.id.value == id) {
        return &s;
      }
    }
    return nullptr;
  }

  Instant now{Millis{10'000}};
  SessionBoard board;
  BoardScene scene;
};

}  // namespace

TEST_F(SceneTest, NewRowsSlideInFromTheRightAndSettle) {
  apply(snapshotOf({session(1, SessionState::Working)}));
  ASSERT_NE(slot(1), nullptr);
  EXPECT_FLOAT_EQ(slot(1)->x.value(), layout::kEnterOffset);
  EXPECT_FLOAT_EQ(slot(1)->opacity.value(), 0.0f);
  run(1'500);
  EXPECT_NEAR(slot(1)->x.value(), 0.0f, 0.05f);
  EXPECT_FLOAT_EQ(slot(1)->opacity.value(), 1.0f);
}

TEST_F(SceneTest, RemovedRowsSlideOutThenFreeTheirSlot) {
  apply(snapshotOf({session(1, SessionState::Done), session(2, SessionState::Working)}));
  run(1'500);
  apply(snapshotOf({session(2, SessionState::Working)}));
  EXPECT_TRUE(slot(1)->exiting);
  run(100);
  EXPECT_LT(slot(1)->x.value(), 0.0f);
  run(400);
  EXPECT_EQ(slot(1), nullptr);
  // Row 2 closed the gap.
  run(1'000);
  EXPECT_NEAR(slot(2)->y.value(), 0.0f, 0.05f);
}

TEST_F(SceneTest, RowReturningMidExitReversesInsteadOfPopping) {
  apply(snapshotOf({session(1, SessionState::Working)}));
  run(1'500);
  apply(Snapshot{});
  run(80);
  const float midway = slot(1)->x.value();
  ASSERT_LT(midway, 0.0f);

  apply(snapshotOf({session(1, SessionState::Working)}));
  EXPECT_FALSE(slot(1)->exiting);
  EXPECT_FLOAT_EQ(slot(1)->x.value(), midway);
  run(1'500);
  EXPECT_NEAR(slot(1)->x.value(), 0.0f, 0.05f);
}

TEST_F(SceneTest, ReorderedRowTravelsWithTheHighlight) {
  apply(snapshotOf({session(1, SessionState::Working), session(2, SessionState::Working)}));
  run(1'500);
  board.moveCursor(CursorMove::Down);
  scene.cursorMoved(board, {.moved = true, .wrapped = false}, now);
  run(500);
  EXPECT_FLOAT_EQ(scene.highlightY(), layout::kRowHeight);

  // Session 2 needs you: it moves to the top and the highlight goes with it.
  apply(snapshotOf({session(1, SessionState::Working), session(2, SessionState::NeedsYou)}));
  run(1'500);
  EXPECT_NEAR(slot(2)->y.value(), 0.0f, 0.05f);
  EXPECT_NEAR(scene.highlightY(), 0.0f, 0.05f);
  EXPECT_EQ(scene.selectedId()->value, 2u);
}

TEST_F(SceneTest, ScrollsToRevealTheCursorAndWrapsBack) {
  Snapshot many;
  for (std::uint32_t id = 1; id <= 8; ++id) {
    many.sessions.push_back(session(id, SessionState::Working));
  }
  apply(many);
  for (int i = 0; i < 6; ++i) {
    scene.cursorMoved(board, board.moveCursor(CursorMove::Down), now);
    run(48);
  }
  run(500);
  EXPECT_FLOAT_EQ(scene.scroll(), 7 * layout::kRowHeight - layout::kViewportHeight);

  scene.cursorMoved(board, board.moveCursor(CursorMove::Down), now);  // row 8, bottom
  run(300);
  scene.cursorMoved(board, board.moveCursor(CursorMove::Down), now);  // wraps to top
  run(30);
  EXPECT_GT(scene.highlightY(), 7 * layout::kRowHeight);  // stretched past the bottom edge
  run(600);
  EXPECT_FLOAT_EQ(scene.highlightY(), 0.0f);
  EXPECT_FLOAT_EQ(scene.scroll(), 0.0f);
}

TEST_F(SceneTest, HoldFillDrainsWhenReleasedEarly) {
  apply(snapshotOf({session(1, SessionState::Done)}));
  scene.hold(0.5f, now);
  EXPECT_FLOAT_EQ(scene.holdFill(), 0.5f);
  scene.hold(std::nullopt, now);
  run(1'000);
  EXPECT_NEAR(scene.holdFill(), 0.0f, 0.01f);
}

TEST_F(SceneTest, CountsRollToNewValues) {
  apply(snapshotOf({session(1, SessionState::NeedsYou), session(2, SessionState::NeedsYou)}));
  run(1'500);
  EXPECT_NEAR(scene.count(SessionState::NeedsYou), 2.0f, 0.01f);
  EXPECT_NEAR(scene.count(SessionState::Done), 0.0f, 0.01f);
}

TEST(Marquee, PausesScrollsPausesRewinds) {
  using anim::marqueeOffset;
  EXPECT_FLOAT_EQ(marqueeOffset(50.0f, Millis{300}), 0.0f);
  EXPECT_FLOAT_EQ(marqueeOffset(50.0f, Millis{600 + 500}), 25.0f);
  EXPECT_FLOAT_EQ(marqueeOffset(50.0f, Millis{600 + 1000 + 300}), 50.0f);
  EXPECT_FLOAT_EQ(marqueeOffset(50.0f, Millis{600 + 1000 + 750 + 50}), 25.0f);
  EXPECT_FLOAT_EQ(marqueeOffset(0.0f, Millis{5000}), 0.0f);
}

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
