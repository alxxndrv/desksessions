#include "desk/app/button_gestures.hpp"

namespace desk::app {
namespace {

constexpr std::size_t indexOf(Button button) { return static_cast<std::size_t>(button); }

}  // namespace

bool ButtonGestures::debounce(Key& key, bool raw, Instant now) {
  if (raw != key.raw) {
    key.raw = raw;
    key.rawSince = now;
  }
  if (key.raw != key.stable && now - key.rawSince >= kDebounce) {
    key.stable = key.raw;
    return true;
  }
  return false;
}

std::optional<Gesture> ButtonGestures::update(bool bootDown, bool auxDown, Instant now) {
  std::optional<Gesture> gesture;
  if (debounce(keys_[indexOf(Button::Boot)], bootDown, now)) {
    gesture = onEdge(Button::Boot, now);
  }
  if (debounce(keys_[indexOf(Button::Aux)], auxDown, now)) {
    if (auto auxGesture = onEdge(Button::Aux, now)) {
      gesture = auxGesture;
    }
  }
  if (gesture) {
    return gesture;
  }

  if (chording_) {
    if (!chordFired_ && now - chordSince_ >= kChord) {
      chordFired_ = true;
      return Gesture{.kind = Gesture::Kind::Chord, .button = Button::Boot};
    }
    return std::nullopt;
  }

  for (Button button : {Button::Boot, Button::Aux}) {
    Key& key = keys_[indexOf(button)];
    if (key.stable && !key.longFired && now - key.downSince >= kLongPress) {
      key.longFired = true;
      return Gesture{.kind = Gesture::Kind::LongPress, .button = button};
    }
  }
  return std::nullopt;
}

std::optional<Gesture> ButtonGestures::onEdge(Button button, Instant now) {
  Key& key = keys_[indexOf(button)];
  const Key& other = keys_[1 - indexOf(button)];

  if (key.stable) {
    key.downSince = now;
    key.longFired = false;
    if (other.stable) {
      // Second finger down: from now on this is a chord, not two presses.
      chording_ = true;
      chordFired_ = false;
      chordSince_ = now;
      return std::nullopt;
    }
    return Gesture{.kind = Gesture::Kind::Down, .button = button};
  }

  if (chording_) {
    if (!other.stable) {
      chording_ = false;  // both released
    }
    key.longFired = true;  // the remaining finger's release must not count as a press
    return std::nullopt;
  }
  if (key.longFired) {
    return std::nullopt;
  }
  return Gesture{.kind = Gesture::Kind::Press, .button = button};
}

std::optional<Hold> ButtonGestures::hold(Instant now) const {
  if (chording_) {
    return std::nullopt;
  }
  for (Button button : {Button::Boot, Button::Aux}) {
    const Key& key = keys_[indexOf(button)];
    if (key.stable && !key.longFired) {
      return Hold{.button = button, .progress = progress(now - key.downSince, kLongPress)};
    }
  }
  return std::nullopt;
}

}  // namespace desk::app
