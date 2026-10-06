#include "desk/scene/stage_scene.hpp"

#include <algorithm>

namespace desk::scene {
namespace {

using anim::Curve;
using anim::Motion;
using app::Phase;

constexpr Millis kRowStagger{33};
constexpr Millis kRowCollapse = anim::duration(anim::kMoook);
constexpr float kWakeDrop = 14.0f;

constexpr Motion kPortHole = Motion::tween(Curve::EaseInOut, Millis{198});
constexpr Motion kOfflineFade = Motion::tween(Curve::CaEaseInOut, Millis{400});
constexpr Motion kPairingSlide = Motion::moook(anim::kMoook);

Millis collapseLength(std::size_t rows) {
  return kRowCollapse + kRowStagger * static_cast<int>(rows);
}

}  // namespace

void StageScene::update(Phase phase, bool boardEmpty, std::size_t rows, Instant now) {
  const bool wasAway = previous_ == Phase::Offline || previous_ == Phase::Asleep;
  if (phase == Phase::Online && wasAway) {
    // Back from being away: the list drops in with Pebble's soft moook.
    wake_.snapTo(-kWakeDrop);
    wake_.animateTo(0.0f, Motion::moook(anim::kMoookSoft), now);
  }
  if ((phase == Phase::Offline) && previous_ != Phase::Offline && previous_ != Phase::Asleep) {
    offlineSince_ = now;
  }
  if (phase == Phase::Pairing && previous_ != Phase::Pairing) {
    pairingSince_ = now;
  }
  previous_ = phase;
  if (phase != Phase::Asleep) {
    rows_ = rows;  // keep the count the collapse started with
  }

  const bool away = phase == Phase::Offline || phase == Phase::Asleep;
  const bool pairingCard = phase == Phase::Unpaired || phase == Phase::Pairing;

  quiet_.animateTo(boardEmpty ? 1.0f : 0.0f, kPortHole, now);
  offline_.animateTo(away ? 1.0f : 0.0f, kOfflineFade, now);
  asleep_.animateTo(phase == Phase::Asleep ? 1.0f : 0.0f,
                    Motion::tween(Curve::Linear, collapseLength(rows_)), now);
  pairing_.animateTo(pairingCard ? 1.0f : 0.0f, kPairingSlide, now);

  quiet_.update(now);
  offline_.update(now);
  asleep_.update(now);
  pairing_.update(now);
  wake_.update(now);
}

float StageScene::collapse(std::size_t index) const {
  const float total = static_cast<float>(collapseLength(rows_).count());
  const float start = static_cast<float>((kRowStagger * static_cast<int>(index)).count());
  const float length = static_cast<float>(kRowCollapse.count());
  return std::clamp((asleep_.value() * total - start) / length, 0.0f, 1.0f);
}

}  // namespace desk::scene
