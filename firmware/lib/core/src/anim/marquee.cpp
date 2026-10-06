#include "desk/anim/marquee.hpp"

#include <cmath>

namespace desk::anim {

float marqueeOffset(float overflow, Millis elapsed) {
  if (overflow <= 0.0f || elapsed.count() <= 0) {
    return 0.0f;
  }
  const auto pixels = static_cast<Millis::rep>(std::ceil(overflow));
  const Millis scroll = kMarqueePerPixel * pixels;
  const Millis rewind = kMarqueeRewindPerPixel * pixels;
  const Millis cycle = kMarqueeLeadPause + scroll + kMarqueeTailPause + rewind;

  Millis t{elapsed.count() % cycle.count()};
  if (t < kMarqueeLeadPause) {
    return 0.0f;
  }
  t -= kMarqueeLeadPause;
  if (t < scroll) {
    return overflow * progress(t, scroll);
  }
  t -= scroll;
  if (t < kMarqueeTailPause) {
    return overflow;
  }
  t -= kMarqueeTailPause;
  return overflow * (1.0f - progress(t, rewind));
}

}  // namespace desk::anim
