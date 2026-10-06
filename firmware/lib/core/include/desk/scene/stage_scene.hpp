#pragma once

#include <cstddef>

#include "desk/anim/channel.hpp"
#include "desk/app/connection.hpp"
#include "desk/util/time.hpp"

namespace desk::scene {

/// Screen-level motion: which screen shows and how far each transition has got. Targets are set
/// from the current phase every frame, so any transition reverses mid-way if the phase flips back.
class StageScene {
 public:
  /// `rows` sizes the staggered collapse into sleep.
  void update(app::Phase phase, bool boardEmpty, std::size_t rows, Instant now);

  /// 0 = list, 1 = "all quiet" (port-hole transition between them).
  [[nodiscard]] float quiet() const { return quiet_.value(); }
  /// 0 = live, 1 = offline: rows at half strength, header says the Mac is asleep.
  [[nodiscard]] float offline() const { return offline_.value(); }
  /// 0 = list, 1 = collapsed into the sleepy face. Rows go one after another.
  [[nodiscard]] float asleep() const { return asleep_.value(); }
  /// Collapse progress of row `index`, staggered 33 ms per row.
  [[nodiscard]] float collapse(std::size_t index) const;
  /// 0 = hidden, 1 = pairing card fully in.
  [[nodiscard]] float pairing() const { return pairing_.value(); }
  /// Vertical offset of the whole list while it bounces awake after a reconnect.
  [[nodiscard]] float wake() const { return wake_.value(); }
  /// When the current offline stretch began, for the bobbing zzz.
  [[nodiscard]] Instant offlineSince() const { return offlineSince_; }
  /// When the passkey appeared, for its digits dropping in one by one.
  [[nodiscard]] Instant pairingSince() const { return pairingSince_; }

 private:
  anim::Channel quiet_;
  anim::Channel offline_;
  anim::Channel asleep_;
  anim::Channel pairing_;
  anim::Channel wake_;
  app::Phase previous_ = app::Phase::Unpaired;
  std::size_t rows_ = 0;
  Instant offlineSince_;
  Instant pairingSince_;
};

}  // namespace desk::scene
