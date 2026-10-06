#include "desk/scene/board_scene.hpp"

#include <algorithm>

#include "desk/scene/layout.hpp"

namespace desk::scene {
namespace {

using anim::Curve;
using anim::Motion;
using domain::SessionState;

constexpr Motion kReflow = Motion::springy(anim::kSmooth);
constexpr Motion kReorder = Motion::springy(anim::kSnappy);
constexpr Motion kCursor = Motion::moook(anim::kMoook);
constexpr Motion kScroll = Motion::tween(Curve::EaseInOut, Millis{250});
constexpr Motion kFadeIn = Motion::tween(Curve::EaseOut, Millis{200});
constexpr Motion kFadeOut = Motion::tween(Curve::EaseIn, anim::duration(anim::kMoook));
constexpr Motion kCrossfade = Motion::tween(Curve::CaDefault, Millis{250});
constexpr Motion kCount = Motion::springy(anim::kSnappy);
constexpr anim::SpringParams kWiggle = anim::springFrom(0.18f, 0.55f);

float rowTop(std::size_t index) { return static_cast<float>(index) * layout::kRowHeight; }

}  // namespace

void RowSlot::update(Instant now) {
  y.update(now);
  x.update(now);
  opacity.update(now);
  lift.update(now);
  stateBlend.update(now);
  scale.update(now);
  glyphScale.update(now);
}

void BoardScene::sync(const domain::SessionBoard& board, Instant now) {
  const auto rows = board.rows();

  for (RowSlot& slot : slots_) {
    const bool stillThere = std::any_of(rows.begin(), rows.end(), [&](const domain::Session& s) {
      return s.id == slot.session.id;
    });
    if (!stillThere && !slot.exiting) {
      slot.exiting = true;
      slot.x.animateTo(layout::kExitOffset, Motion::moook(anim::kMoook), now);
      slot.opacity.animateTo(0.0f, kFadeOut, now);
    }
  }

  for (std::size_t i = 0; i < rows.size(); ++i) {
    const domain::Session& session = rows[i];
    const float top = rowTop(i);
    RowSlot* slot = find(session.id);
    if (slot == nullptr) {
      enter(session, top, now);
      continue;
    }

    if (slot->exiting) {
      // Came back while sliding out: reverse from wherever it got to.
      slot->exiting = false;
      slot->x.animateTo(0.0f, kReorder, now);
      slot->opacity.animateTo(1.0f, kFadeIn, now);
    }

    const bool stateChanged = slot->session.state != session.state;
    if (stateChanged) {
      slot->previousState = slot->session.state;
      slot->stateSince = now;
      slot->stateBlend.snapTo(0.0f);
      slot->stateBlend.animateTo(1.0f, kCrossfade, now);
      if (session.state == SessionState::Done) {
        slot->glyphScale.snapTo(0.0f);
        slot->glyphScale.animateTo(1.0f, Motion::springy(anim::kBouncy), now);
        slot->glyphScale.kick(6.0f, now);
      }
    }
    slot->session = session;

    if (slot->y.target() != top) {
      if (stateChanged) {
        slot->y.animateTo(top, kReorder, now);
        slot->lift.animateTo(1.0f, Motion::tween(Curve::EaseOut, Millis{120}), now);
        slot->lift.then(0.0f, kReflow);
      } else {
        slot->y.animateTo(top, kReflow, now);
      }
    }
  }

  visibleRows_ = rows.size();
  needsYou_.animateTo(board.counts().needsYou, kCount, now);
  done_.animateTo(board.counts().done, kCount, now);
  working_.animateTo(board.counts().working, kCount, now);
  follow(board, now);
}

void BoardScene::cursorMoved(const domain::SessionBoard& board, domain::CursorMoved moved,
                             Instant now) {
  if (!moved.moved) {
    return;
  }
  const float target = rowTop(board.cursor());
  if (moved.wrapped) {
    // Stretch past the edge it is leaving, then jump to the other end.
    const bool fromBottom = target == 0.0f;
    const float edge =
        highlight_.target() + (fromBottom ? layout::kWrapStretch : -layout::kWrapStretch);
    highlight_.animateTo(edge, Motion::tween(Curve::CaEaseOut, Millis{66}), now);
    highlight_.then(target, kCursor);
  } else {
    highlight_.animateTo(target, kCursor, now);
  }
  selected_ = board.selected() != nullptr ? std::optional{board.selected()->id} : std::nullopt;
  selectedSince_ = now;
  scrollToReveal(target, now);
}

void BoardScene::nudge(float direction, Instant now) {
  nudge_.animateTo(direction * layout::kNudge, Motion::tween(Curve::EaseOut, Millis{48}), now);
  nudge_.then(0.0f, Motion::tween(Curve::EaseOut, Millis{144}));
}

void BoardScene::hold(std::optional<float> progress, Instant now) {
  RowSlot* slot = selectedSlot();
  if (progress) {
    holding_ = true;
    holdFill_.snapTo(*progress);
    if (slot != nullptr) {
      slot->lift.snapTo(*progress * layout::kHoldLift);
    }
    return;
  }
  if (holding_) {
    // Let go early: everything drains back.
    holding_ = false;
    holdFill_.animateTo(0.0f, Motion::springy(anim::kSnappy), now);
    if (slot != nullptr) {
      slot->lift.animateTo(0.0f, kReflow, now);
    }
  }
}

void BoardScene::longPressFired(Instant now) {
  holding_ = false;
  holdFill_.snapTo(1.0f);
  holdFill_.animateTo(0.0f, Motion::tween(Curve::EaseOut, Millis{250}), now);
  if (RowSlot* slot = selectedSlot()) {
    slot->scale.kick(1.2f, now);
    slot->lift.animateTo(0.0f, kReflow, now);
  }
}

void BoardScene::refuse(Instant now) {
  if (RowSlot* slot = selectedSlot()) {
    slot->x.kick(260.0f, now, kWiggle);
  }
}

void BoardScene::update(Instant now) {
  for (RowSlot& slot : slots_) {
    slot.update(now);
  }
  slots_.erase(std::remove_if(slots_.begin(), slots_.end(),
                              [](const RowSlot& s) { return s.exiting && !s.x.animating(); }),
               slots_.end());
  highlight_.update(now);
  highlightOpacity_.update(now);
  scroll_.update(now);
  nudge_.update(now);
  holdFill_.update(now);
  needsYou_.update(now);
  done_.update(now);
  working_.update(now);
}

float BoardScene::count(domain::SessionState state) const {
  switch (state) {
    case SessionState::NeedsYou:
      return needsYou_.value();
    case SessionState::Done:
      return done_.value();
    case SessionState::Working:
      return working_.value();
  }
  return 0.0f;
}

float BoardScene::contentHeight() const { return rowTop(visibleRows_); }

RowSlot* BoardScene::find(domain::SessionId id) {
  const auto it = std::find_if(slots_.begin(), slots_.end(),
                               [id](const RowSlot& s) { return s.session.id == id; });
  return it == slots_.end() ? nullptr : &*it;
}

RowSlot* BoardScene::selectedSlot() { return selected_ ? find(*selected_) : nullptr; }

void BoardScene::enter(const domain::Session& session, float y, Instant now) {
  if (slots_.full()) {
    // Out of slots only while many rows are still sliding out: drop the oldest leaver.
    const auto leaver =
        std::find_if(slots_.begin(), slots_.end(), [](const RowSlot& s) { return s.exiting; });
    if (leaver == slots_.end()) {
      return;
    }
    slots_.erase(leaver);
  }
  RowSlot& slot = slots_.emplace_back();
  slot.session = session;
  slot.previousState = session.state;
  slot.stateSince = now;
  slot.y.snapTo(y);
  slot.x.snapTo(layout::kEnterOffset);
  slot.x.animateTo(0.0f, kReorder, now);
  slot.opacity.snapTo(0.0f);
  slot.opacity.animateTo(1.0f, kFadeIn, now);
  slot.stateBlend.snapTo(1.0f);
}

void BoardScene::follow(const domain::SessionBoard& board, Instant now) {
  const domain::Session* selected = board.selected();
  highlightOpacity_.animateTo(selected != nullptr ? 1.0f : 0.0f, kFadeIn, now);
  if (selected == nullptr) {
    selected_.reset();
    return;
  }
  if (selected_ != selected->id) {
    selected_ = selected->id;
    selectedSince_ = now;
  }
  const float target = rowTop(board.cursor());
  if (highlight_.target() != target) {
    // The selected row moved under the cursor: travel with it.
    highlight_.animateTo(target, kReorder, now);
  }
  scrollToReveal(target, now);
}

void BoardScene::scrollToReveal(float top, Instant now) {
  const float maxScroll = std::max(0.0f, contentHeight() - layout::kViewportHeight);
  float target = scroll_.target();
  if (top < target) {
    target = top;
  } else if (top + layout::kRowHeight > target + layout::kViewportHeight) {
    target = top + layout::kRowHeight - layout::kViewportHeight;
  }
  target = std::clamp(target, 0.0f, maxScroll);
  if (target != scroll_.target()) {
    scroll_.animateTo(target, kScroll, now);
  }
}

}  // namespace desk::scene
