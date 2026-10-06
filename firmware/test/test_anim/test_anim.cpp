#include <gtest/gtest.h>

#include <algorithm>

#include "desk/anim/breathing.hpp"
#include "desk/anim/easing.hpp"
#include "desk/anim/moook.hpp"
#include "desk/anim/rubber_band.hpp"
#include "desk/anim/spring.hpp"

using namespace desk;
using namespace desk::anim;

TEST(Easing, EndpointsAreExactForEveryCurve) {
  for (auto curve : {Curve::Linear, Curve::EaseIn, Curve::EaseOut, Curve::EaseInOut,
                     Curve::CaDefault, Curve::CaEaseIn, Curve::CaEaseOut, Curve::CaEaseInOut}) {
    EXPECT_FLOAT_EQ(ease(curve, 0.0f), 0.0f);
    EXPECT_FLOAT_EQ(ease(curve, 1.0f), 1.0f);
    EXPECT_FLOAT_EQ(ease(curve, -1.0f), 0.0f);
    EXPECT_FLOAT_EQ(ease(curve, 2.0f), 1.0f);
  }
}

TEST(Easing, PebbleQuadraticsMatchTables) {
  EXPECT_FLOAT_EQ(ease(Curve::EaseIn, 0.5f), 0.25f);
  EXPECT_FLOAT_EQ(ease(Curve::EaseOut, 0.5f), 0.75f);
  EXPECT_FLOAT_EQ(ease(Curve::EaseInOut, 0.5f), 0.5f);
  EXPECT_FLOAT_EQ(ease(Curve::EaseInOut, 0.25f), 0.125f);
}

TEST(Easing, CubicBezierIsMonotonicAndSymmetric) {
  float previous = 0.0f;
  for (int i = 1; i <= 100; ++i) {
    const float value = kCaEaseInOut(static_cast<float>(i) / 100.0f);
    EXPECT_GE(value, previous);
    previous = value;
  }
  EXPECT_NEAR(kCaEaseInOut(0.5f), 0.5f, 1e-4f);
  EXPECT_NEAR(kCaEaseInOut(0.3f) + kCaEaseInOut(0.7f), 1.0f, 1e-4f);
}

TEST(Easing, CaDefaultIsFrontLoaded) {
  // (0.25, 0.1, 0.25, 1) reaches ~80 % by the halfway point.
  EXPECT_NEAR(kCaDefault(0.5f), 0.8024f, 2e-3f);
}

TEST(Moook, DurationIsSevenPebbleFrames) {
  EXPECT_EQ(duration(kMoook), Millis{231});
  EXPECT_EQ(duration(kMoookSoft), Millis{330});
  EXPECT_EQ(duration(kPeek), Millis{264});
}

TEST(Moook, HitsPebbleKeyframesOnFrameBoundaries) {
  // A one-row move down: 0 → 30 px.
  const float expected[] = {0, 1, 20, 34, 32, 31, 30};
  for (int frame = 0; frame < 7; ++frame) {
    EXPECT_FLOAT_EQ(moook(kMoook, 0.0f, 30.0f, kPebbleFrame * frame), expected[frame])
        << "frame " << frame;
  }
  EXPECT_FLOAT_EQ(moook(kMoook, 0.0f, 30.0f, duration(kMoook)), 30.0f);
}

TEST(Moook, InterpolatesBetweenFrames) {
  EXPECT_FLOAT_EQ(moook(kMoook, 0.0f, 30.0f, Millis{0} + kPebbleFrame + Millis{16}),
                  1.0f + 19.0f * 16.0f / 33.0f);
}

TEST(Moook, MovesUpwardMirrorImage) {
  EXPECT_FLOAT_EQ(moook(kMoook, 30.0f, 0.0f, kPebbleFrame * 2), 10.0f);
  EXPECT_FLOAT_EQ(moook(kMoook, 30.0f, 0.0f, kPebbleFrame * 3), -4.0f);
}

TEST(Moook, ShortMovesScaleTheirOvershoot) {
  // 15 px is half the reference distance, so the 4 px overshoot becomes 2 px.
  EXPECT_FLOAT_EQ(moook(kMoook, 0.0f, 15.0f, kPebbleFrame * 3), 17.0f);
  EXPECT_FLOAT_EQ(moook(kMoook, 5.0f, 5.0f, Millis{50}), 5.0f);
}

TEST(Moook, SoftVariantRunsLinearMiddleFrames) {
  // in {0,1,20}, then 3 frames from 20 to 34, then out {4,2,1,0}.
  EXPECT_FLOAT_EQ(moook(kMoookSoft, 0.0f, 30.0f, kPebbleFrame * 3), 23.5f);
  EXPECT_FLOAT_EQ(moook(kMoookSoft, 0.0f, 30.0f, kPebbleFrame * 5), 30.5f);
  EXPECT_FLOAT_EQ(moook(kMoookSoft, 0.0f, 30.0f, kPebbleFrame * 6), 34.0f);
}

namespace {
int stepsToSettle(Spring spring, const SpringParams& params, float* peak) {
  int steps = 0;
  *peak = spring.value;
  while (!spring.settled() && steps < 10'000) {
    step(spring, params);
    *peak = std::max(*peak, spring.value);
    ++steps;
  }
  return steps;
}
}  // namespace

TEST(Spring, PresetsMatchApplesFormulas) {
  EXPECT_NEAR(kSmooth.stiffness, 157.91f, 0.01f);
  EXPECT_NEAR(kSmooth.damping, 25.13f, 0.01f);
  EXPECT_NEAR(kSnappy.damping, 21.36f, 0.01f);
  EXPECT_NEAR(kBouncy.damping, 17.59f, 0.01f);
}

TEST(Spring, SmoothNeverOvershoots) {
  Spring spring{.value = 0.0f, .velocity = 0.0f, .target = 100.0f};
  float peak = 0.0f;
  const int steps = stepsToSettle(spring, kSmooth, &peak);
  EXPECT_LE(peak, 100.01f);
  EXPECT_LT(steps, 160);  // Apple: ~0.8 s to settle
}

TEST(Spring, BouncyOvershootsMoreThanSnappy) {
  float snappyPeak = 0.0f;
  float bouncyPeak = 0.0f;
  stepsToSettle(Spring{.value = 0.0f, .velocity = 0.0f, .target = 100.0f}, kSnappy, &snappyPeak);
  stepsToSettle(Spring{.value = 0.0f, .velocity = 0.0f, .target = 100.0f}, kBouncy, &bouncyPeak);
  EXPECT_GT(snappyPeak, 100.0f);
  EXPECT_GT(bouncyPeak, snappyPeak);
}

TEST(Spring, SettledSpringSnapsAndStops) {
  Spring spring;
  spring.snapTo(42.0f);
  step(spring, kSnappy);
  EXPECT_FLOAT_EQ(spring.value, 42.0f);
  EXPECT_TRUE(spring.settled());
}

TEST(RubberBand, ResistsAndNeverExceedsDimension) {
  EXPECT_FLOAT_EQ(rubberBand(0.0f, 100.0f), 0.0f);
  EXPECT_NEAR(rubberBand(1.0f, 100.0f), 0.55f, 0.01f);
  EXPECT_LT(rubberBand(10'000.0f, 100.0f), 100.0f);
  EXPECT_FLOAT_EQ(rubberBand(-20.0f, 100.0f), -rubberBand(20.0f, 100.0f));
}

TEST(Breathing, RisesPeaksAndRests) {
  EXPECT_FLOAT_EQ(breathe(Millis{0}), 0.0f);
  EXPECT_NEAR(breathe(Millis{700}), 1.0f, 1e-5f);
  EXPECT_FLOAT_EQ(breathe(Millis{1500}), 0.0f);
  EXPECT_NEAR(breathe(Millis{1800 + 700}), 1.0f, 1e-5f);
}

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
