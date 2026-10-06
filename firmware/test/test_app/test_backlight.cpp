#include <gtest/gtest.h>

#include <cmath>

#include "desk/app/backlight.hpp"
#include "desk/app/wall_clock.hpp"

using namespace desk;
using namespace desk::app;
using domain::Alert;

namespace {

constexpr WallTime kNoon{12, 0};
constexpr WallTime kMidnight{0, 30};

float settle(BacklightController& b, Phase phase, bool empty, WallTime t, Instant& now) {
  float level = 0.0f;
  for (int i = 0; i < 1'000; i += 16) {
    now = now + Millis{16};
    level = b.level(phase, empty, t, now);
  }
  return level;
}

}  // namespace

TEST(Backlight, RestingLevelsBySituation) {
  BacklightController b;
  Instant now{Millis{1'000}};
  EXPECT_FLOAT_EQ(settle(b, Phase::Online, false, kNoon, now), 1.0f);
  EXPECT_FLOAT_EQ(settle(b, Phase::Online, true, kNoon, now), BacklightController::kQuiet);
  EXPECT_FLOAT_EQ(settle(b, Phase::Asleep, false, kNoon, now), BacklightController::kAsleep);
  EXPECT_FLOAT_EQ(settle(b, Phase::Online, false, kMidnight, now), BacklightController::kNight);
}

TEST(Backlight, NeedsYouPulsesThreeTimes) {
  BacklightController b;
  Instant now{Millis{1'000}};
  settle(b, Phase::Online, false, kNoon, now);
  b.alert(Alert::NeedsYou, now);
  const Instant start = now;
  int dips = 0;
  bool inDip = false;
  while (now - start < Millis{7'000}) {
    now = now + Millis{16};
    const bool dipping = b.level(Phase::Online, false, kNoon, now) < 0.6f;
    dips += dipping && !inDip ? 1 : 0;
    inDip = dipping;
  }
  EXPECT_EQ(dips, 3);
}

TEST(Backlight, BurstsMergeIntoOneSequence) {
  BacklightController b;
  Instant now{Millis{1'000}};
  settle(b, Phase::Online, false, kNoon, now);
  b.alert(Alert::Done, now);
  now = now + Millis{1'000};
  b.alert(Alert::Done, now);  // within 5 s: does not restart
  now = now + Millis{1'000};  // 2 s after the first: its single pulse is over
  EXPECT_FLOAT_EQ(b.level(Phase::Online, false, kNoon, now), 1.0f);
}

TEST(Backlight, LateAlertsExtendPulsingWithoutAJump) {
  BacklightController b;
  Instant now{Millis{1'000}};
  settle(b, Phase::Online, false, kNoon, now);
  b.alert(Alert::NeedsYou, now);
  float previous = b.level(Phase::Online, false, kNoon, now);
  float largestStep = 0.0f;
  for (int t = 0; t < 12'000; t += 16) {
    now = now + Millis{16};
    if (t == 5'200) {
      b.alert(Alert::NeedsYou, now);  // past the merge window, mid third breath
    }
    const float level = b.level(Phase::Online, false, kNoon, now);
    largestStep = std::max(largestStep, std::fabs(level - previous));
    previous = level;
  }
  EXPECT_LT(largestStep, 0.05f);
}

TEST(Backlight, NoPulsesAtNight) {
  BacklightController b;
  Instant now{Millis{1'000}};
  settle(b, Phase::Online, false, kMidnight, now);
  b.alert(Alert::NeedsYou, now);
  now = now + Millis{900};
  EXPECT_FLOAT_EQ(b.level(Phase::Online, false, kMidnight, now), BacklightController::kNight);
}

TEST(WallClock, CarriesTimeForward) {
  WallClock clock;
  EXPECT_FALSE(clock.now(Instant{}).has_value());
  clock.set(23 * 3600 + 59 * 60 + 30, Instant{Millis{1'000}});
  EXPECT_EQ(clock.now(Instant{Millis{1'000}}), (WallTime{23, 59}));
  EXPECT_EQ(clock.now(Instant{Millis{31'000}}), (WallTime{0, 0}));
}
