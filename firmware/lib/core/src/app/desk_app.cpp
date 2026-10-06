#include "desk/app/desk_app.hpp"

namespace desk::app {

domain::Alert DeskApp::receive(const domain::Snapshot& snapshot, Instant now) {
  const domain::Alert alert = board_.apply(snapshot, now);
  scene_.sync(board_, now);
  backlightControl_.alert(alert, now);
  return alert;
}

domain::Alert DeskApp::receive(const protocol::Frame& frame, Instant now) {
  setTime(frame.secondsOfDay, now);
  setNightHours(NightHours{.startHour = frame.nightStart, .endHour = frame.nightEnd});
  return receive(frame.snapshot, now);
}

void DeskApp::linkChanged(const LinkEvent& event, Instant now) { connection_.onEvent(event, now); }

void DeskApp::tick(const Inputs& inputs, Instant now) {
  phase_ = connection_.phase(now);
  if ((phase_ == Phase::Offline || phase_ == Phase::Asleep) && connection_.lostAt()) {
    board_.freezeTime(*connection_.lostAt());  // the numbers stopped being true back then
  }

  if (const auto gesture = gestures_.update(inputs.bootDown, inputs.auxDown, now)) {
    lastInteraction_ = now;
    onGesture(*gesture, now);
  }
  const auto hold = gestures_.hold(now);
  scene_.hold(hold ? std::optional{hold->progress} : std::nullopt, now);

  if (board_.cursor() != 0 && now - lastInteraction_ >= kCursorIdleReset) {
    // Nobody's touched it for a while: drift back to the most urgent row.
    const std::size_t before = board_.cursor();
    board_.resetCursor();
    scene_.cursorMoved(board_, {.moved = before != 0, .wrapped = false}, now);
  }

  scene_.update(now);
  stage_.update(phase_, board_.empty(), board_.rows().size(), now);
  backlight_.set(backlightControl_.level(phase_, board_.empty(), wallClock_.now(now), now));
}

void DeskApp::onGesture(const Gesture& gesture, Instant now) {
  using Kind = Gesture::Kind;
  switch (gesture.kind) {
    case Kind::Down:
      scene_.nudge(gesture.button == Button::Boot ? -1.0f : 1.0f, now);
      break;
    case Kind::Press:
      moveCursor(gesture.button == Button::Boot ? domain::CursorMove::Up : domain::CursorMove::Down,
                 now);
      break;
    case Kind::LongPress: {
      const auto command =
          gesture.button == Button::Aux ? board_.dismissSelected() : board_.jumpToSelected();
      if (!command) {
        scene_.hold(std::nullopt, now);
        scene_.refuse(now);
        break;
      }
      scene_.longPressFired(now);
      link_.send(*command);
      scene_.sync(board_, now);
      break;
    }
    case Kind::Chord:
      link_.forgetPairing();
      break;
  }
}

void DeskApp::moveCursor(domain::CursorMove move, Instant now) {
  const domain::CursorMoved moved = board_.moveCursor(move);
  if (moved.moved) {
    scene_.cursorMoved(board_, moved, now);
  } else {
    scene_.refuse(now);
  }
}

}  // namespace desk::app
