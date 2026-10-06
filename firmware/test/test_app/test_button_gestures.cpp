#include <gtest/gtest.h>

#include <vector>

#include "desk/app/button_gestures.hpp"

using namespace desk;
using namespace desk::app;

namespace {

using Kind = Gesture::Kind;

// Drives the classifier one millisecond at a time and records every gesture.
class Fingers {
 public:
  void hold(bool boot, bool aux, int ms) {
    for (int i = 0; i < ms; ++i) {
      now_ = now_ + Millis{1};
      if (auto g = gestures_.update(boot, aux, now_)) {
        seen_.push_back(*g);
      }
    }
  }

  [[nodiscard]] const std::vector<Gesture>& seen() const { return seen_; }
  [[nodiscard]] std::optional<Hold> holdState() const { return gestures_.hold(now_); }

 private:
  ButtonGestures gestures_;
  Instant now_;
  std::vector<Gesture> seen_;
};

}  // namespace

TEST(ButtonGestures, ShortPressGivesDownThenPress) {
  Fingers f;
  f.hold(false, false, 10);
  f.hold(true, false, 100);
  f.hold(false, false, 50);
  EXPECT_EQ(f.seen(),
            (std::vector<Gesture>{{Kind::Down, Button::Boot}, {Kind::Press, Button::Boot}}));
}

TEST(ButtonGestures, IgnoresBounces) {
  Fingers f;
  for (int i = 0; i < 5; ++i) {
    f.hold(false, true, 3);
    f.hold(false, false, 3);
  }
  EXPECT_TRUE(f.seen().empty());
}

TEST(ButtonGestures, LongPressFiresWhileHeldAndReleaseIsSilent) {
  Fingers f;
  f.hold(false, true, 400);
  ASSERT_TRUE(f.holdState().has_value());
  EXPECT_EQ(f.holdState()->button, Button::Aux);
  EXPECT_NEAR(f.holdState()->progress, (400.0f - 15.0f) / 600.0f, 0.01f);

  f.hold(false, true, 300);
  EXPECT_EQ(f.seen(),
            (std::vector<Gesture>{{Kind::Down, Button::Aux}, {Kind::LongPress, Button::Aux}}));
  EXPECT_FALSE(f.holdState().has_value());

  f.hold(false, false, 50);
  EXPECT_EQ(f.seen().size(), 2u);
}

TEST(ButtonGestures, BothButtonsForFiveSecondsIsAChordOnly) {
  Fingers f;
  f.hold(true, false, 100);
  f.hold(true, true, 5'100);
  f.hold(true, false, 100);
  f.hold(false, false, 100);
  EXPECT_EQ(f.seen(),
            (std::vector<Gesture>{{Kind::Down, Button::Boot}, {Kind::Chord, Button::Boot}}));
}

TEST(ButtonGestures, AbandonedChordProducesNothing) {
  Fingers f;
  f.hold(false, true, 50);
  f.hold(true, true, 1'000);
  f.hold(false, false, 100);
  EXPECT_EQ(f.seen(), (std::vector<Gesture>{{Kind::Down, Button::Aux}}));
}

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
