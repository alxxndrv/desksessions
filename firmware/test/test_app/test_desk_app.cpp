#include <gtest/gtest.h>

#include "desk/app/desk_app.hpp"
#include "fakes.hpp"

using namespace desk;
using namespace desk::app;
using namespace desk::domain;

namespace {

class DeskAppTest : public ::testing::Test {
 protected:
  void SetUp() override {
    Snapshot snapshot;
    for (auto [id, state] : {std::pair{1u, SessionState::NeedsYou},
                             {2u, SessionState::Done},
                             {3u, SessionState::Working}}) {
      Session s;
      s.id = SessionId{id};
      s.state = state;
      snapshot.sessions.push_back(s);
    }
    app.receive(snapshot, now);
  }

  void hold(bool boot, bool aux, int ms) {
    for (int t = 0; t < ms; ++t) {
      now = now + Millis{1};
      app.tick({.bootDown = boot, .auxDown = aux}, now);
    }
  }
  void tap(bool boot, bool aux) {
    hold(boot, aux, 80);
    hold(false, false, 80);
  }

  Instant now{Millis{5'000}};
  test::RecordingLink link;
  DeskApp app{link};
};

}  // namespace

TEST_F(DeskAppTest, TapsMoveTheCursor) {
  tap(false, true);
  EXPECT_EQ(app.board().cursor(), 1u);
  tap(true, false);
  EXPECT_EQ(app.board().cursor(), 0u);
}

TEST_F(DeskAppTest, LongPressOnDoneDismissesAndTellsTheMac) {
  tap(false, true);
  hold(false, true, 700);
  hold(false, false, 50);
  ASSERT_EQ(link.sent.size(), 1u);
  EXPECT_EQ(link.sent[0], (Command{.kind = Command::Kind::Dismiss, .session = SessionId{2}}));
  EXPECT_EQ(app.board().rows().size(), 2u);
}

TEST_F(DeskAppTest, LongPressBootJumps) {
  hold(true, false, 700);
  ASSERT_EQ(link.sent.size(), 1u);
  EXPECT_EQ(link.sent[0].kind, Command::Kind::Jump);
  EXPECT_EQ(link.sent[0].session.value, 1u);
}

TEST_F(DeskAppTest, DismissingANonDoneRowSendsNothing) {
  hold(false, true, 700);
  EXPECT_TRUE(link.sent.empty());
  EXPECT_EQ(app.board().rows().size(), 3u);
}

TEST_F(DeskAppTest, CursorDriftsHomeAfterThirtySecondsIdle) {
  tap(false, true);
  tap(false, true);
  hold(false, false, 29'000);
  EXPECT_EQ(app.board().cursor(), 2u);
  hold(false, false, 1'100);
  EXPECT_EQ(app.board().cursor(), 0u);
}
