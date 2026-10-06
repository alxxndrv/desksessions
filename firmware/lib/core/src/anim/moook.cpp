#include "desk/anim/moook.hpp"

#include <algorithm>
#include <cmath>

#include "desk/anim/easing.hpp"

namespace desk::anim {
namespace {

struct Keyframes {
  const MoookShape& shape;
  float from;
  float to;
  float direction;  // ±scale: sign of travel times the short-move scale factor

  [[nodiscard]] std::size_t count() const {
    return shape.in.size() + shape.midFrames + shape.out.size();
  }

  // Value of frame `index`; one past the last frame is the target itself.
  [[nodiscard]] float at(std::size_t index) const {
    const std::size_t inCount = shape.in.size();
    const std::size_t midEnd = inCount + shape.midFrames;
    if (index >= count()) {
      return to;
    }
    if (index < inCount) {
      return from + direction * shape.in[index];
    }
    if (index < midEnd) {
      const float start = from + direction * shape.in.back();
      const float end = to + direction * shape.out.front();
      const auto step = static_cast<float>(index - inCount + 1);
      return lerp(start, end, step / static_cast<float>(shape.midFrames + 1));
    }
    return to + direction * shape.out[index - midEnd];
  }
};

}  // namespace

float moook(const MoookShape& shape, float from, float to, Millis elapsed) {
  const float distance = to - from;
  if (distance == 0.0f || elapsed >= duration(shape)) {
    return to;
  }
  if (elapsed.count() <= 0) {
    return from;
  }

  const float scale = std::min(1.0f, std::fabs(distance) / kMoookReferenceDistance);
  const Keyframes frames{shape, from, to, std::copysign(scale, distance)};

  const float position =
      static_cast<float>(elapsed.count()) / static_cast<float>(kPebbleFrame.count());
  const auto index = static_cast<std::size_t>(position);
  const float within = position - static_cast<float>(index);
  return lerp(frames.at(index), frames.at(index + 1), within);
}

}  // namespace desk::anim
