#pragma once

#include <optional>

#include "desk/app/button_gestures.hpp"
#include "desk/app/ports.hpp"
#include "desk/domain/session_board.hpp"
#include "desk/scene/board_scene.hpp"
#include "desk/util/time.hpp"

namespace desk::app {

/// Raw inputs for one tick.
struct Inputs {
  bool bootDown = false;
  bool auxDown = false;
};

/// The application: owns the board and its scene, turns gestures into board operations and
/// commands for the Mac. Runs on the UI task only.
class DeskApp {
 public:
  static constexpr Millis kCursorIdleReset{30'000};

  explicit DeskApp(CommandLink& link) : link_(link) {}

  /// A fresh snapshot from the Mac.
  domain::Alert receive(const domain::Snapshot& snapshot, Instant now);

  /// Advances one frame: buttons, idle behaviour, animation.
  void tick(const Inputs& inputs, Instant now);

  [[nodiscard]] const domain::SessionBoard& board() const { return board_; }
  [[nodiscard]] const scene::BoardScene& scene() const { return scene_; }
  [[nodiscard]] const ButtonGestures& gestures() const { return gestures_; }

 private:
  void onGesture(const Gesture& gesture, Instant now);
  void moveCursor(domain::CursorMove move, Instant now);

  CommandLink& link_;
  domain::SessionBoard board_;
  scene::BoardScene scene_;
  ButtonGestures gestures_;
  Instant lastInteraction_;
};

}  // namespace desk::app
