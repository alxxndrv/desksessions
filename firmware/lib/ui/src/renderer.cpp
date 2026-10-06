#include "desk/ui/renderer.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <numbers>

#include "desk/anim/breathing.hpp"
#include "desk/anim/easing.hpp"
#include "desk/anim/marquee.hpp"
#include "desk/anim/moook.hpp"
#include "desk/scene/layout.hpp"
#include "desk/ui/glyphs.hpp"
#include "desk/ui/theme.hpp"

namespace desk::ui {
namespace {

using domain::SessionState;
namespace layout = scene::layout;

constexpr float kPi = std::numbers::pi_v<float>;

const lgfx::IFont* const kNameFont = &fonts::lgfxJapanGothicP_16;
const lgfx::IFont* const kSmallFont = &fonts::lgfxJapanGothicP_12;

// Row geometry, relative to the row's top-left.
constexpr float kBarX = 10.0f;
constexpr float kGlyphX = 26.0f;
constexpr float kNameX = 40.0f;
constexpr float kNameWidth = 182.0f;
constexpr float kDetailX = 228.0f;
constexpr float kTimeRight = 310.0f;

constexpr Millis kSpinPeriod{1'200};
constexpr Millis kHopEvery{3'000};

int px(float v) { return static_cast<int>(std::lround(v)); }

/// Fraction of a repeating period, in [0, 1).
float phase(Millis elapsed, Millis period) {
  const auto rem = ((elapsed.count() % period.count()) + period.count()) % period.count();
  return static_cast<float>(rem) / static_cast<float>(period.count());
}

/// Vertical offset of the NEEDS YOU glyph's hop: squash, spring up, land with Pebble's peek bounce.
struct Hop {
  float y = 0.0f;
  float squash = 1.0f;  // vertical scale
};

Hop hopAt(Millis sinceStateChange) {
  const auto t = Millis{sinceStateChange.count() % kHopEvery.count()};
  constexpr Millis kSquash{anim::kPebbleFrame};
  constexpr Millis kRise{120};
  constexpr float kHeight = -5.0f;
  if (t < kSquash) {
    return {.y = 0.0f, .squash = 0.75f};
  }
  if (t < kSquash + kRise) {
    const float k = anim::ease(anim::Curve::EaseOut, progress(t - kSquash, kRise));
    return {.y = kHeight * k, .squash = 1.0f + 0.12f * (1.0f - k)};
  }
  const Millis fall = t - kSquash - kRise;
  if (fall < anim::duration(anim::kPeek)) {
    return {.y = anim::moook(anim::kPeek, kHeight, 0.0f, fall), .squash = 1.0f};
  }
  return {};
}

void formatDuration(char* out, std::size_t size, Millis d, SessionState state) {
  const long total = static_cast<long>(d.count() / 1000);
  if (state == SessionState::Done) {
    if (total < 60) {
      std::snprintf(out, size, "now");
    } else if (total < 3600) {
      std::snprintf(out, size, "%ldm", total / 60);
    } else {
      std::snprintf(out, size, "%ldh", total / 3600);
    }
    return;
  }
  if (total < 3600) {
    std::snprintf(out, size, "%ld:%02ld", total / 60, total % 60);
  } else {
    std::snprintf(out, size, "%ldh%02ld", total / 3600, (total / 60) % 60);
  }
}

/// Cuts UTF-8 `text` to fit `width` px, ending with an ellipsis when shortened.
void fitText(lgfx::LovyanGFX& g, const char* text, float width, char* out, std::size_t size) {
  std::snprintf(out, size, "%s", text);
  if (static_cast<float>(g.textWidth(out)) <= width) {
    return;
  }
  std::size_t len = std::strlen(out);
  constexpr const char* kEllipsis = "…";
  while (len > 0) {
    do {
      --len;  // step back over a whole UTF-8 sequence
    } while (len > 0 && (static_cast<unsigned char>(out[len]) & 0xC0) == 0x80);
    while (len > 0 && out[len - 1] == ' ') {
      --len;
    }
    std::snprintf(out + len, size - len, "%s", kEllipsis);
    if (static_cast<float>(g.textWidth(out)) <= width) {
      return;
    }
  }
}

}  // namespace

void Renderer::draw(const app::DeskApp& app, Instant now, std::optional<app::WallTime> wallTime) {
  canvas_.fillScreen(hex(theme::kBackground));
  if (app.board().empty() && app.scene().slots().empty()) {
    drawEmpty(now);
  } else {
    drawList(app, now);
  }
  drawHeader(app, wallTime);
}

void Renderer::drawList(const app::DeskApp& app, Instant now) {
  const scene::BoardScene& scene = app.scene();
  const float originY = layout::kHeaderHeight - scene.scroll() + scene.nudgeOffset();
  canvas_.setClipRect(0, px(layout::kHeaderHeight), px(layout::kScreenWidth),
                      px(layout::kViewportHeight));

  // Highlight under the selected row, with the long-press fill sweeping across it.
  const float highlight = scene.highlightOpacity();
  if (highlight > 0.01f) {
    const int top = px(originY + scene.highlightY()) + 2;
    canvas_.fillSmoothRoundRect(4, top, 312, 26, 8,
                                hex(mix(theme::kBackground, theme::kSurface, highlight)));
    if (const auto hold = app.gestures().hold(now)) {
      lastHold_ = hold->button;
    }
    const float fill = scene.holdFill();
    if (fill > 0.005f) {
      const Rgb tint = lastHold_ == app::Button::Boot ? theme::kJump : theme::kDone;
      const int width = std::max(16, px(312.0f * fill));
      canvas_.fillSmoothRoundRect(
          4, top, width, 26, 8, hex(mix(theme::kSurface, tint, 0.28f * std::min(1.0f, fill * 3))));
    }
  }

  // Leaving rows first, so rows arriving or reordering pass over them.
  for (const scene::RowSlot& slot : scene.slots()) {
    if (slot.exiting) {
      drawRow(app, slot, now);
    }
  }
  for (const scene::RowSlot& slot : scene.slots()) {
    if (!slot.exiting && slot.lift.value() < 0.01f) {
      drawRow(app, slot, now);
    }
  }
  for (const scene::RowSlot& slot : scene.slots()) {
    if (!slot.exiting && slot.lift.value() >= 0.01f) {
      drawRow(app, slot, now);
    }
  }

  // Scroll hint on the right edge when the list overflows.
  const float content = scene.contentHeight();
  if (content > layout::kViewportHeight) {
    const float ratio = layout::kViewportHeight / content;
    const float thumb = std::max(12.0f, layout::kViewportHeight * ratio);
    const float travel = layout::kViewportHeight - thumb;
    const float pos = scene.scroll() / (content - layout::kViewportHeight);
    canvas_.fillSmoothRoundRect(316, px(layout::kHeaderHeight + travel * pos) + 2, 2, px(thumb) - 4,
                                1, hex(theme::kTextFaint));
  }
  canvas_.clearClipRect();
}

void Renderer::drawRow(const app::DeskApp& app, const scene::RowSlot& slot, Instant now) {
  const scene::BoardScene& scene = app.scene();
  const domain::Session& session = slot.session;
  const float originY = layout::kHeaderHeight - scene.scroll() + scene.nudgeOffset();
  const float top = originY + slot.y.value();
  if (top > layout::kScreenHeight || top + layout::kRowHeight < layout::kHeaderHeight) {
    return;
  }
  const float left = slot.x.value();
  const float centerY = top + layout::kRowHeight / 2.0f;
  const float opacity = std::clamp(slot.opacity.value(), 0.0f, 1.0f);
  const float scale = slot.scale.value();
  const bool selected = scene.selectedId() == session.id && !slot.exiting;
  const auto fade = [&](Rgb c) { return hex(mix(theme::kBackground, c, opacity)); };

  // Lift: a brighter plate while held or hopping to a new place.
  const float lift = std::clamp(slot.lift.value(), 0.0f, 1.0f);
  if (lift > 0.01f) {
    const Rgb base = selected ? theme::kSurface : theme::kBackground;
    canvas_.fillSmoothRoundRect(px(left + 4), px(top + 2), 312, 26, 8,
                                fade(mix(base, theme::kLifted, lift)));
  }

  // State bar: crossfades between states; NEEDS YOU breathes.
  Rgb bar = mix(theme::stateColor(slot.previousState), theme::stateColor(session.state),
                slot.stateBlend.value());
  if (session.state == SessionState::NeedsYou) {
    bar = mix(mix(theme::kBackground, bar, 0.45f), bar, anim::breathe(now - slot.stateSince));
  }
  const float barHeight = 16.0f * scale;
  canvas_.fillSmoothRoundRect(px(left + kBarX), px(centerY - barHeight / 2), 4, px(barHeight), 2,
                              fade(bar));

  // Glyph: spins while working, hops when it needs you, pops into a check when done.
  const Millis inState = now - slot.stateSince;
  const std::uint32_t agentColor = fade(theme::agentColor(session.agent));
  const float glyphX = left + kGlyphX;
  if (session.state == SessionState::Done) {
    const float r = 6.5f * std::max(0.0f, slot.glyphScale.value()) * scale;
    glyphs::check(canvas_, glyphX, centerY, r, fade(theme::kDone));
  } else {
    float angle = 0.0f;
    float glyphY = centerY;
    float r = 6.0f * scale;
    if (session.state == SessionState::Working) {
      angle = 2.0f * kPi * phase(now.sinceBoot, kSpinPeriod);
    } else {
      const Hop hop = hopAt(inState);
      glyphY += hop.y + r * (1.0f - hop.squash);
      r *= hop.squash;
    }
    if (session.agent == domain::Agent::Claude) {
      glyphs::sparkle(canvas_, glyphX, glyphY, r, angle, agentColor);
    } else {
      glyphs::hexagon(canvas_, glyphX, glyphY, r * 0.85f, angle / 2.0f, agentColor);
    }
  }

  canvas_.setTextDatum(textdatum_t::middle_left);
  canvas_.setTextSize(scale);

  // Name: ellipsised, or scrolling through when selected and too long.
  canvas_.setFont(kNameFont);
  const float nameWidth = static_cast<float>(canvas_.textWidth(session.name.c_str()));
  canvas_.setTextColor(fade(theme::kText));
  if (selected && nameWidth > kNameWidth) {
    const float offset = anim::marqueeOffset(nameWidth - kNameWidth, now - scene.selectedSince());
    const int clipTop = std::max(px(top), px(layout::kHeaderHeight));
    canvas_.setClipRect(px(left + kNameX), clipTop, px(kNameWidth), px(layout::kRowHeight));
    canvas_.drawString(session.name.c_str(), px(left + kNameX - offset), px(centerY));
    canvas_.setClipRect(0, px(layout::kHeaderHeight), px(layout::kScreenWidth),
                        px(layout::kViewportHeight));
  } else {
    char name[domain::kMaxNameBytes + 4];
    fitText(canvas_, session.name.c_str(), kNameWidth, name, sizeof(name));
    canvas_.drawString(name, px(left + kNameX), px(centerY));
  }

  // Detail and time.
  canvas_.setFont(kSmallFont);
  canvas_.setTextColor(fade(theme::kTextDim));
  char detail[domain::kMaxDetailBytes + 8];
  if (session.subagents > 0) {
    std::snprintf(detail, sizeof(detail), "×%u", static_cast<unsigned>(session.subagents));
  } else {
    std::snprintf(detail, sizeof(detail), "%s", session.detail.c_str());
  }
  canvas_.drawString(detail, px(left + kDetailX), px(centerY));

  char time[12];
  formatDuration(time, sizeof(time), app.board().timeInState(session, now), session.state);
  canvas_.setTextDatum(textdatum_t::middle_right);
  canvas_.setTextColor(fade(theme::kTextFaint));
  canvas_.drawString(time, px(left + kTimeRight), px(centerY));
  canvas_.setTextSize(1.0f);
}

void Renderer::drawHeader(const app::DeskApp& app, std::optional<app::WallTime> wallTime) {
  const scene::BoardScene& scene = app.scene();
  canvas_.fillRect(0, 0, px(layout::kScreenWidth), px(layout::kHeaderHeight),
                   hex(theme::kBackground));
  if (scene.scroll() > 0.5f) {
    canvas_.drawFastHLine(0, px(layout::kHeaderHeight) - 1, px(layout::kScreenWidth),
                          hex(theme::kHairline));
  }

  canvas_.setFont(kSmallFont);
  canvas_.setTextDatum(textdatum_t::middle_left);
  const float midY = layout::kHeaderHeight / 2.0f;
  const float lineHeight = 14.0f;

  struct Counter {
    SessionState state;
    const char* label;
  };
  float x = 8.0f;
  for (const Counter counter :
       {Counter{SessionState::NeedsYou, "NEED"}, Counter{SessionState::Done, "DONE"},
        Counter{SessionState::Working, "WORK"}}) {
    const float value = std::max(0.0f, scene.count(counter.state));
    const bool any = value > 0.05f;
    const Rgb dot = any ? theme::stateColor(counter.state) : theme::kTextFaint;
    canvas_.dot(x + 3.0f, midY, 3.0f, hex(dot));

    // Odometer: the old number rolls up and out as the new one rolls in.
    const float whole = std::floor(value);
    const float frac = value - whole;
    const int digitX = px(x + 11.0f);
    canvas_.setClipRect(digitX, 2, 18, px(layout::kHeaderHeight) - 4);
    canvas_.setTextColor(hex(any ? theme::kText : theme::kTextFaint));
    char digits[4];
    std::snprintf(digits, sizeof(digits), "%d", static_cast<int>(whole));
    canvas_.drawString(digits, digitX, px(midY - frac * lineHeight));
    if (frac > 0.01f) {
      std::snprintf(digits, sizeof(digits), "%d", static_cast<int>(whole) + 1);
      canvas_.drawString(digits, digitX, px(midY + (1.0f - frac) * lineHeight));
    }
    canvas_.clearClipRect();

    canvas_.setTextColor(hex(theme::kTextDim));
    const int labelX = digitX + std::max(8, static_cast<int>(canvas_.textWidth(digits))) + 3;
    canvas_.drawString(counter.label, labelX, px(midY));
    x = static_cast<float>(labelX + canvas_.textWidth(counter.label)) + 12.0f;
  }

  if (wallTime) {
    char clock[6];
    std::snprintf(clock, sizeof(clock), "%02u:%02u", static_cast<unsigned>(wallTime->hour),
                  static_cast<unsigned>(wallTime->minute));
    canvas_.setTextDatum(textdatum_t::middle_right);
    canvas_.setTextColor(hex(theme::kTextDim));
    canvas_.drawString(clock, px(layout::kScreenWidth) - 8, px(midY));
  }
}

void Renderer::drawEmpty(Instant now) {
  // Placeholder until the "all quiet" character arrives.
  const float bob = 2.0f * std::sin(2.0f * kPi * phase(now.sinceBoot, Millis{2'400}));
  canvas_.setFont(kNameFont);
  canvas_.setTextDatum(textdatum_t::middle_center);
  canvas_.setTextColor(hex(theme::kTextDim));
  canvas_.drawString("All quiet", px(layout::kScreenWidth / 2), px(95.0f + bob));
}

}  // namespace desk::ui
