#include <gtest/gtest.h>

#include "desk/scene/stage_scene.hpp"

using namespace desk;
using namespace desk::scene;
using app::Phase;

namespace {

void run(StageScene& stage, Phase phase, bool empty, std::size_t rows, Instant& now, int ms) {
  for (int t = 0; t < ms; t += 16) {
    now = now + Millis{16};
    stage.update(phase, empty, rows, now);
  }
}

}  // namespace

TEST(StageScene, PortHoleToQuietReversesMidWay) {
  StageScene stage;
  Instant now{Millis{1'000}};
  run(stage, Phase::Online, false, 3, now, 300);
  run(stage, Phase::Online, true, 0, now, 96);
  const float midway = stage.quiet();
  ASSERT_GT(midway, 0.1f);
  ASSERT_LT(midway, 0.9f);
  run(stage, Phase::Online, false, 1, now, 16);  // turns around here
  const float turned = stage.quiet();
  EXPECT_NEAR(turned, midway, 0.2f);  // no jump
  run(stage, Phase::Online, false, 1, now, 16);
  EXPECT_LT(stage.quiet(), turned);
  run(stage, Phase::Online, false, 1, now, 300);
  EXPECT_FLOAT_EQ(stage.quiet(), 0.0f);
}

TEST(StageScene, AsleepCollapsesRowsInTurn) {
  StageScene stage;
  Instant now{Millis{1'000}};
  run(stage, Phase::Online, false, 4, now, 100);
  run(stage, Phase::Asleep, false, 4, now, 64);
  EXPECT_GT(stage.collapse(0), stage.collapse(1));
  EXPECT_GT(stage.collapse(1), stage.collapse(3));
  run(stage, Phase::Asleep, false, 4, now, 600);
  EXPECT_FLOAT_EQ(stage.collapse(3), 1.0f);
}

TEST(StageScene, WakesWithABounceWhenTheMacReturns) {
  StageScene stage;
  Instant now{Millis{1'000}};
  run(stage, Phase::Online, false, 2, now, 100);
  run(stage, Phase::Offline, false, 2, now, 1'000);
  run(stage, Phase::Online, false, 2, now, 16);
  EXPECT_LT(stage.wake(), 0.0f);
  run(stage, Phase::Online, false, 2, now, 600);
  EXPECT_FLOAT_EQ(stage.wake(), 0.0f);
  EXPECT_FLOAT_EQ(stage.offline(), 0.0f);
}
