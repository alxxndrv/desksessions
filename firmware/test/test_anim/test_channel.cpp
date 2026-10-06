#include <gtest/gtest.h>

#include <cmath>

#include "desk/anim/channel.hpp"

using namespace desk;
using namespace desk::anim;

namespace {

// Runs `channel` frame by frame at 60 fps for `ms`, returning the largest jump between frames.
float run(Channel& channel, Instant& now, int ms) {
  float largestStep = 0.0f;
  float previous = channel.value();
  for (int t = 0; t < ms; t += 16) {
    now = now + Millis{16};
    channel.update(now);
    largestStep = std::max(largestStep, std::fabs(channel.value() - previous));
    previous = channel.value();
  }
  return largestStep;
}

}  // namespace

TEST(Channel, TweenReachesTargetExactly) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  channel.animateTo(100.0f, Motion::tween(Curve::EaseInOut, Millis{250}), now);
  run(channel, now, 300);
  EXPECT_FLOAT_EQ(channel.value(), 100.0f);
  EXPECT_FALSE(channel.animating());
}

TEST(Channel, ReversingATweenMidFlightDoesNotJump) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  channel.animateTo(100.0f, Motion::tween(Curve::EaseInOut, Millis{250}), now);
  run(channel, now, 112);
  const float before = channel.value();
  ASSERT_GT(before, 10.0f);
  ASSERT_LT(before, 90.0f);

  channel.animateTo(0.0f, Motion::tween(Curve::EaseInOut, Millis{250}), now);
  EXPECT_FLOAT_EQ(channel.value(), before);
  // Pebble: an interrupted scroll continues with ease-out, so it moves off quickly.
  const float largest = run(channel, now, 300);
  EXPECT_FLOAT_EQ(channel.value(), 0.0f);
  EXPECT_LT(largest, 20.0f);
}

TEST(Channel, InterruptedMoookOnlySettles) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  channel.animateTo(30.0f, Motion::moook(kMoook), now);
  run(channel, now, 48);
  const float before = channel.value();

  // Second press mid-flight: next row down.
  channel.animateTo(60.0f, Motion::moook(kMoook), now);
  EXPECT_FLOAT_EQ(channel.value(), before);
  now = now + Millis{1};
  channel.update(now);
  EXPECT_GT(channel.value(), before);  // no anticipation backwards
  run(channel, now, static_cast<int>(duration(kMoookSettle).count()) + 32);
  EXPECT_FLOAT_EQ(channel.value(), 60.0f);
}

TEST(Channel, RetargetingASpringKeepsMomentum) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  channel.animateTo(100.0f, Motion::springy(kSnappy), now);
  run(channel, now, 100);
  const float velocity = channel.velocity();
  ASSERT_GT(velocity, 0.0f);

  channel.animateTo(200.0f, Motion::springy(kSnappy), now);
  EXPECT_FLOAT_EQ(channel.velocity(), velocity);
  run(channel, now, 2'000);
  EXPECT_NEAR(channel.value(), 200.0f, 0.01f);
}

TEST(Channel, SpringTakesOverATweensMomentum) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  channel.animateTo(100.0f, Motion::tween(Curve::Linear, Millis{1'000}), now);
  run(channel, now, 496);
  EXPECT_NEAR(channel.velocity(), 100.0f, 1.0f);

  channel.animateTo(0.0f, Motion::springy(kSmooth), now);
  EXPECT_NEAR(channel.velocity(), 100.0f, 1.0f);
  const float before = channel.value();
  now = now + Millis{9};  // one physics step
  channel.update(now);
  EXPECT_GT(channel.value(), before);  // still coasting forward before turning back
}

TEST(Channel, SameTargetDoesNotRestart) {
  Instant now{Millis{1'000}};
  Channel channel{0.0f};
  const Motion motion = Motion::tween(Curve::Linear, Millis{100});
  channel.animateTo(10.0f, motion, now);
  run(channel, now, 48);
  channel.animateTo(10.0f, motion, now);
  run(channel, now, 64);
  EXPECT_FALSE(channel.animating());
}

TEST(Channel, KickPopsAndReturns) {
  Instant now{Millis{1'000}};
  Channel scale{1.0f};
  scale.kick(1.5f, now);
  run(scale, now, 80);
  EXPECT_GT(scale.value(), 1.02f);
  run(scale, now, 2'000);
  EXPECT_NEAR(scale.value(), 1.0f, 0.01f);
}
