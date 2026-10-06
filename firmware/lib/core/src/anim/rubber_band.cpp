#include "desk/anim/rubber_band.hpp"

#include <cmath>

namespace desk::anim {

float rubberBand(float offset, float dimension, float c) {
  if (dimension <= 0.0f) {
    return 0.0f;
  }
  const float magnitude = (1.0f - 1.0f / (std::fabs(offset) * c / dimension + 1.0f)) * dimension;
  return std::copysign(magnitude, offset);
}

}  // namespace desk::anim
