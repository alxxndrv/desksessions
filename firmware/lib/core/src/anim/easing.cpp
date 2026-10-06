#include "desk/anim/easing.hpp"

#include <algorithm>
#include <cmath>

namespace desk::anim {
namespace {

// Polynomial coefficients of one Bézier axis: B(s) = ((a·s + b)·s + c)·s.
struct Axis {
  float a;
  float b;
  float c;

  constexpr Axis(float p1, float p2)
      : a(1.0f - 3.0f * p2 + 3.0f * p1), b(3.0f * p2 - 6.0f * p1), c(3.0f * p1) {}

  [[nodiscard]] constexpr float at(float s) const { return ((a * s + b) * s + c) * s; }
  [[nodiscard]] constexpr float slope(float s) const { return (3.0f * a * s + 2.0f * b) * s + c; }
};

// Finds s with x(s) = x: Newton–Raphson first, bisection when the slope is too flat.
float solveForParameter(const Axis& xAxis, float x) {
  constexpr float kEpsilon = 1e-5f;
  float s = x;
  for (int i = 0; i < 8; ++i) {
    const float error = xAxis.at(s) - x;
    if (std::fabs(error) < kEpsilon) {
      return s;
    }
    const float slope = xAxis.slope(s);
    if (std::fabs(slope) < 1e-6f) {
      break;
    }
    s -= error / slope;
  }

  float low = 0.0f;
  float high = 1.0f;
  s = x;
  for (int i = 0; i < 32 && low < high; ++i) {
    const float value = xAxis.at(s);
    if (std::fabs(value - x) < kEpsilon) {
      break;
    }
    (x > value ? low : high) = s;
    s = (low + high) * 0.5f;
  }
  return s;
}

float quadIn(float t) { return t * t; }
float quadOut(float t) { return 1.0f - (1.0f - t) * (1.0f - t); }
float quadInOut(float t) { return t < 0.5f ? 2.0f * t * t : 1.0f - 2.0f * (1.0f - t) * (1.0f - t); }

}  // namespace

float CubicBezier::operator()(float t) const {
  t = std::clamp(t, 0.0f, 1.0f);
  if (t == 0.0f || t == 1.0f) {
    return t;
  }
  const Axis xAxis{x1, x2};
  const Axis yAxis{y1, y2};
  return yAxis.at(solveForParameter(xAxis, t));
}

float ease(Curve curve, float t) {
  t = std::clamp(t, 0.0f, 1.0f);
  switch (curve) {
    case Curve::Linear:
      return t;
    case Curve::EaseIn:
      return quadIn(t);
    case Curve::EaseOut:
      return quadOut(t);
    case Curve::EaseInOut:
      return quadInOut(t);
    case Curve::CaDefault:
      return kCaDefault(t);
    case Curve::CaEaseIn:
      return kCaEaseIn(t);
    case Curve::CaEaseOut:
      return kCaEaseOut(t);
    case Curve::CaEaseInOut:
      return kCaEaseInOut(t);
  }
  return t;
}

}  // namespace desk::anim
