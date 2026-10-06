#include <gtest/gtest.h>

#include "desk/app/connection.hpp"
#include "desk/app/desk_app.hpp"
#include "fakes.hpp"

using namespace desk;
using namespace desk::app;

TEST(Connection, StartsUnpairedThenPairsAndConnects) {
  Connection c;
  const Instant t0{Millis{1'000}};
  EXPECT_EQ(c.phase(t0), Phase::Unpaired);
  c.onEvent({.kind = LinkEvent::Kind::Passkey, .passkey = 123456}, t0);
  EXPECT_EQ(c.phase(t0), Phase::Pairing);
  EXPECT_EQ(c.passkey(), 123456u);
  c.onEvent({.kind = LinkEvent::Kind::Connected}, t0);
  EXPECT_EQ(c.phase(t0), Phase::Online);
}

TEST(Connection, GracePeriodsAfterLosingTheMac) {
  Connection c;
  const Instant t0{Millis{1'000}};
  c.onEvent({.kind = LinkEvent::Kind::Connected}, t0);
  c.onEvent({.kind = LinkEvent::Kind::Disconnected}, t0);
  EXPECT_EQ(c.phase(t0 + Millis{9'999}), Phase::Reconnecting);
  // A repeated report doesn't restart the clock.
  c.onEvent({.kind = LinkEvent::Kind::Disconnected}, t0 + Millis{5'000});
  EXPECT_EQ(c.phase(t0 + Millis{10'000}), Phase::Offline);
  EXPECT_EQ(c.phase(t0 + Millis{300'000}), Phase::Asleep);
  c.onEvent({.kind = LinkEvent::Kind::Connected}, t0 + Millis{301'000});
  EXPECT_EQ(c.phase(t0 + Millis{301'000}), Phase::Online);
  EXPECT_FALSE(c.lostAt().has_value());
}

TEST(DeskAppLink, OfflineFreezesTimersAndDimsAfterGrace) {
  test::RecordingLink link;
  test::FakeBacklight backlight;
  DeskApp app{link, backlight};
  Instant now{Millis{1'000}};
  app.linkChanged({.kind = LinkEvent::Kind::Connected}, now);
  domain::Snapshot snapshot;
  domain::Session s;
  s.id = domain::SessionId{1};
  snapshot.sessions.push_back(s);
  app.receive(snapshot, now);

  app.linkChanged({.kind = LinkEvent::Kind::Disconnected}, now);
  for (int i = 0; i < 12'000; i += 16) {
    now = now + Millis{16};
    app.tick({}, now);
  }
  EXPECT_EQ(app.phase(), Phase::Offline);
  EXPECT_NEAR(app.stage().offline(), 1.0f, 0.01f);
  EXPECT_EQ(app.board().timeInState(app.board().rows()[0], now), Millis{0});
}

TEST(DeskAppLink, ChordForgetsPairing) {
  test::RecordingLink link;
  test::FakeBacklight backlight;
  DeskApp app{link, backlight};
  Instant now{Millis{1'000}};
  for (int i = 0; i < 5'200; ++i) {
    now = now + Millis{1};
    app.tick({.bootDown = true, .auxDown = true}, now);
  }
  EXPECT_EQ(link.forgotPairing, 1);
}
