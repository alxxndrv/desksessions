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

const Fonts& Renderer::fonts() {
  if (!fonts_) {
    fonts_.emplace();
  }
  return *fonts_;
}

void Renderer::draw(const app::DeskApp& app, Instant now, Canvas& target) {
  target_ = &target;
  draw(app, now);
}

void Renderer::draw(const app::DeskApp& app, Instant now) {
  const scene::StageScene& stage = app.stage();
  canvas().fillScreen(hex(theme::kBackground));

  if (stage.pairing() < 0.999f) {
    if (stage.quiet() < 0.999f && stage.asleep() < 0.999f) {
      drawList(app, now);
      drawPortHole(stage.quiet());
    }
    if (stage.quiet() > 0.001f && stage.asleep() < 0.5f) {
      drawQuiet(app, now);
    }
    if (stage.asleep() > 0.001f) {
      drawAsleep(app, now);
    }
    drawHeader(app, now, app.wallTime(now));
  }
  if (stage.pairing() > 0.001f) {
    drawPairing(app, now);
  }
}

void Renderer::drawList(const app::DeskApp& app, Instant now) {
  const scene::BoardScene& scene = app.scene();
  const float originY =
      layout::kHeaderHeight - scene.scroll() + scene.nudgeOffset() + app.stage().wake();
  canvas().setClipRect(0, px(layout::kHeaderHeight), px(layout::kScreenWidth),
                       px(layout::kViewportHeight));

  // Highlight under the selected row, with the long-press fill sweeping across it.
  const float highlight =
      scene.highlightOpacity() * (1.0f - app.stage().offline()) * (1.0f - app.stage().asleep());
  if (highlight > 0.01f) {
    const int top = px(originY + scene.highlightY()) + 2;
    canvas().fillSmoothRoundRect(4, top, 312, 26, 8,
                                 hex(mix(theme::kBackground, theme::kSurface, highlight)));
    if (const auto hold = app.gestures().hold(now)) {
      lastHold_ = hold->button;
    }
    const float fill = scene.holdFill();
    if (fill > 0.005f) {
      const Rgb tint = lastHold_ == app::Button::Boot ? theme::kJump : theme::kDone;
      const int width = std::max(16, px(312.0f * fill));
      canvas().fillSmoothRoundRect(
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
    canvas().fillSmoothRoundRect(316, px(layout::kHeaderHeight + travel * pos) + 2, 2,
                                 px(thumb) - 4, 1, hex(theme::kTextFaint));
  }
  canvas().clearClipRect();
}

void Renderer::drawRow(const app::DeskApp& app, const scene::RowSlot& slot, Instant now) {
  const scene::BoardScene& scene = app.scene();
  const domain::Session& session = slot.session;
  const scene::StageScene& stage = app.stage();
  const float originY = layout::kHeaderHeight - scene.scroll() + scene.nudgeOffset() + stage.wake();

  // Falling asleep: rows gather towards the middle one after another and fade.
  const auto index = static_cast<std::size_t>(std::max(0.0f, slot.y.target() / layout::kRowHeight));
  const float collapse = anim::ease(anim::Curve::EaseIn, stage.collapse(index));
  const float middle = scene.scroll() + layout::kViewportHeight / 2.0f - layout::kRowHeight / 2.0f;
  const float top = originY + slot.y.value() + (middle - slot.y.value()) * collapse;
  if (top > layout::kScreenHeight || top + layout::kRowHeight < layout::kHeaderHeight) {
    return;
  }
  const float left = slot.x.value();
  const float centerY = top + layout::kRowHeight / 2.0f;
  const float opacity = std::clamp(slot.opacity.value(), 0.0f, 1.0f) *
                        (1.0f - 0.5f * stage.offline()) * (1.0f - collapse);
  const float scale = slot.scale.value();
  const bool selected = scene.selectedId() == session.id && !slot.exiting;
  const auto fade = [&](Rgb c) { return hex(mix(theme::kBackground, c, opacity)); };

  // Lift: a brighter plate while held or hopping to a new place.
  const float lift = std::clamp(slot.lift.value(), 0.0f, 1.0f);
  if (lift > 0.01f) {
    const Rgb base = selected ? theme::kSurface : theme::kBackground;
    canvas().fillSmoothRoundRect(px(left + 4), px(top + 2), 312, 26, 8,
                                 fade(mix(base, theme::kLifted, lift)));
  }

  // State bar: crossfades between states; NEEDS YOU breathes.
  Rgb bar = mix(theme::stateColor(slot.previousState), theme::stateColor(session.state),
                slot.stateBlend.value());
  if (session.state == SessionState::NeedsYou) {
    bar = mix(mix(theme::kBackground, bar, 0.45f), bar, anim::breathe(now - slot.stateSince));
  }
  const float barHeight = 16.0f * scale;
  canvas().fillSmoothRoundRect(px(left + kBarX), px(centerY - barHeight / 2), 4, px(barHeight), 2,
                               fade(bar));

  // Glyph: spins while working, hops when it needs you, pops into a check when done.
  const Millis inState = now - slot.stateSince;
  const std::uint32_t agentColor = fade(theme::agentColor(session.agent));
  const float glyphX = left + kGlyphX;
  if (session.state == SessionState::Done) {
    const float r = 6.5f * std::max(0.0f, slot.glyphScale.value()) * scale;
    glyphs::check(canvas(), glyphX, centerY, r, fade(theme::kDone));
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
      glyphs::sparkle(canvas(), glyphX, glyphY, r, angle, agentColor);
    } else {
      glyphs::hexagon(canvas(), glyphX, glyphY, r * 0.85f, angle / 2.0f, agentColor);
    }
  }

  canvas().setTextDatum(textdatum_t::middle_left);
  canvas().setTextSize(scale);

  // Name: ellipsised, or scrolling through when selected and too long.
  canvas().setFont(fonts().name());
  const float nameWidth = static_cast<float>(canvas().textWidth(session.name.c_str()));
  canvas().setTextColor(fade(theme::kText));
  if (selected && nameWidth > kNameWidth) {
    const float offset = anim::marqueeOffset(nameWidth - kNameWidth, now - scene.selectedSince());
    const int clipTop = std::max(px(top), px(layout::kHeaderHeight));
    canvas().setClipRect(px(left + kNameX), clipTop, px(kNameWidth), px(layout::kRowHeight));
    canvas().drawString(session.name.c_str(), px(left + kNameX - offset), px(centerY));
    canvas().setClipRect(0, px(layout::kHeaderHeight), px(layout::kScreenWidth),
                         px(layout::kViewportHeight));
  } else {
    char name[domain::kMaxNameBytes + 4];
    fitText(canvas(), session.name.c_str(), kNameWidth, name, sizeof(name));
    canvas().drawString(name, px(left + kNameX), px(centerY));
  }

  // Detail and time.
  canvas().setFont(fonts().small());
  canvas().setTextColor(fade(theme::kTextDim));
  char detail[domain::kMaxDetailBytes + 8];
  if (session.subagents > 0) {
    std::snprintf(detail, sizeof(detail), "×%u", static_cast<unsigned>(session.subagents));
  } else {
    std::snprintf(detail, sizeof(detail), "%s", session.detail.c_str());
  }
  canvas().drawString(detail, px(left + kDetailX), px(centerY));

  char time[12];
  formatDuration(time, sizeof(time), app.board().timeInState(session, now), session.state);
  canvas().setTextDatum(textdatum_t::middle_right);
  canvas().setTextColor(fade(theme::kTextFaint));
  canvas().drawString(time, px(left + kTimeRight), px(centerY));
  canvas().setTextSize(1.0f);
}

void Renderer::drawHeader(const app::DeskApp& app, Instant now,
                          std::optional<app::WallTime> wallTime) {
  const scene::BoardScene& scene = app.scene();
  canvas().fillRect(0, 0, px(layout::kScreenWidth), px(layout::kHeaderHeight),
                    hex(theme::kBackground));
  if (scene.scroll() > 0.5f && app.stage().quiet() < 0.5f) {
    canvas().drawFastHLine(0, px(layout::kHeaderHeight) - 1, px(layout::kScreenWidth),
                           hex(theme::kHairline));
  }

  // Counters lift away and fade as the "Mac asleep" status rises in to replace them.
  constexpr float kTravel = 7.0f;
  const float away = anim::ease(anim::Curve::EaseInOut, app.stage().offline());
  if (away < 0.5f) {
    drawCounters(app, -away * 2.0f * kTravel, 1.0f - away * 2.0f);
  } else {
    drawAwayStatus(now, (1.0f - away) * 2.0f * kTravel, (away - 0.5f) * 2.0f);
  }

  if (wallTime) {
    char clock[6];
    std::snprintf(clock, sizeof(clock), "%02u:%02u", static_cast<unsigned>(wallTime->hour),
                  static_cast<unsigned>(wallTime->minute));
    canvas().setFont(fonts().small());
    canvas().setTextDatum(textdatum_t::middle_right);
    canvas().setTextColor(hex(theme::kTextDim));
    canvas().drawString(clock, px(layout::kScreenWidth) - 8, px(layout::kHeaderHeight / 2.0f));
  }
}

void Renderer::drawCounters(const app::DeskApp& app, float offsetY, float alpha) {
  const scene::BoardScene& scene = app.scene();
  canvas().setFont(fonts().label());
  canvas().setTextDatum(textdatum_t::middle_left);
  const float midY = layout::kHeaderHeight / 2.0f + offsetY;
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
    const auto faded = [&](Rgb c) { return hex(mix(theme::kBackground, c, alpha)); };
    const Rgb dot = any ? theme::stateColor(counter.state) : theme::kTextFaint;
    canvas().dot(x + 3.0f, midY, 3.0f, faded(dot));

    // Odometer: the old number rolls up and out as the new one rolls in.
    const float whole = std::floor(value);
    const float frac = value - whole;
    const int digitX = px(x + 11.0f);
    canvas().setClipRect(digitX, 2, 18, px(layout::kHeaderHeight) - 4);
    canvas().setTextColor(faded(any ? theme::kText : theme::kTextFaint));
    char digits[4];
    std::snprintf(digits, sizeof(digits), "%d", static_cast<int>(whole));
    canvas().drawString(digits, digitX, px(midY - frac * lineHeight));
    if (frac > 0.01f) {
      std::snprintf(digits, sizeof(digits), "%d", static_cast<int>(whole) + 1);
      canvas().drawString(digits, digitX, px(midY + (1.0f - frac) * lineHeight));
    }
    canvas().clearClipRect();

    canvas().setTextColor(faded(theme::kTextDim));
    const int labelX = digitX + std::max(8, static_cast<int>(canvas().textWidth(digits))) + 3;
    canvas().drawString(counter.label, labelX, px(midY));
    x = static_cast<float>(labelX + canvas().textWidth(counter.label)) + 12.0f;
  }
}

void Renderer::drawAwayStatus(Instant now, float offsetY, float alpha) {
  const float midY = layout::kHeaderHeight / 2.0f + offsetY;
  canvas().dot(11.0f, midY, 3.0f, hex(mix(theme::kBackground, theme::kTextFaint, alpha)));
  canvas().setFont(fonts().small());
  canvas().setTextDatum(textdatum_t::middle_left);
  canvas().setTextColor(hex(mix(theme::kBackground, theme::kTextDim, alpha)));
  const char* label = "Mac asleep";
  canvas().drawString(label, 19, px(midY));
  drawZzz(static_cast<float>(19 + canvas().textWidth(label)) + 6.0f, midY + 2.0f, now, alpha);
}

void Renderer::drawZzz(float x, float y, Instant now, float strength) {
  // Three z's drifting up and to the right, each fading in and out on its own beat.
  canvas().setFont(fonts().small());
  canvas().setTextDatum(textdatum_t::middle_center);
  for (int k = 0; k < 3; ++k) {
    const float p = phase(now.sinceBoot + Millis{k * 800}, Millis{2'400});
    const float alpha = std::sin(kPi * p) * strength;
    canvas().setTextSize(0.7f + 0.4f * p);
    canvas().setTextColor(hex(mix(theme::kBackground, theme::kTextDim, alpha)));
    canvas().drawString("z", px(x + 7.0f * p), px(y - 9.0f * p));
  }
  canvas().setTextSize(1.0f);
}

void Renderer::drawPortHole(float progress) {
  // Pebble's port hole: the list shrinks away through a closing circle.
  if (progress <= 0.001f) {
    return;
  }
  constexpr float kCx = layout::kScreenWidth / 2.0f;
  constexpr float kCy = layout::kHeaderHeight + layout::kViewportHeight / 2.0f;
  constexpr float kFar = 190.0f;
  const float radius = kFar * (1.0f - anim::ease(anim::Curve::EaseInOut, progress));
  canvas().fillArc(px(kCx), px(kCy), std::max(0, px(radius)), px(kFar), 0.0f, 360.0f,
                   hex(theme::kBackground));
}

void Renderer::drawQuiet(const app::DeskApp& app, Instant now) {
  const scene::StageScene& stage = app.stage();
  constexpr float kCx = layout::kScreenWidth / 2.0f;
  constexpr float kCy = 82.0f;
  // Grows out of the port hole with a little overshoot.
  constexpr anim::CubicBezier kBackOut{0.34f, 1.56f, 0.64f, 1.0f};
  const float grow = kBackOut(stage.quiet());
  const float radius = 20.0f * grow;
  if (radius < 0.5f) {
    return;
  }

  // Idle life: a slow breath, a blink every few seconds; sleepier while the Mac is away.
  const float breath = std::sin(2.0f * kPi * phase(now.sinceBoot, Millis{3'200}));
  const float blinkPhase = phase(now.sinceBoot, Millis{4'200});
  const float blink =
      blinkPhase > 0.95f ? std::fabs(std::cos(kPi * (blinkPhase - 0.95f) / 0.05f)) : 1.0f;
  glyphs::Face face;
  face.eyesOpen = blink * (1.0f - 0.55f * stage.offline());
  face.smile = 1.0f - 0.6f * stage.offline();
  face.squash = 1.0f + 0.035f * breath;
  glyphs::face(canvas(), kCx, kCy, radius, face, hex(theme::kSurface), hex(theme::kTextDim));

  canvas().setFont(fonts().name());
  canvas().setTextDatum(textdatum_t::middle_center);
  canvas().setTextColor(hex(mix(theme::kBackground, theme::kTextDim, stage.quiet())));
  canvas().drawString("All quiet", px(kCx), px(kCy + 40.0f + 4.0f * (1.0f - stage.quiet())));
  if (stage.offline() > 0.01f) {
    drawZzz(kCx + radius + 6.0f, kCy - radius * 0.6f, now, stage.offline());
  }
}

void Renderer::drawAsleep(const app::DeskApp& app, Instant now) {
  const scene::StageScene& stage = app.stage();
  const float appear = std::clamp((stage.asleep() - 0.5f) * 2.0f, 0.0f, 1.0f);
  if (appear <= 0.0f) {
    return;
  }
  constexpr float kCx = layout::kScreenWidth / 2.0f;
  constexpr float kCy = 86.0f;
  const float breath = std::sin(2.0f * kPi * phase(now.sinceBoot, Millis{4'000}));
  glyphs::Face face;
  face.eyesOpen = 0.0f;
  face.smile = 0.35f;
  face.squash = 1.0f + 0.05f * breath;
  const float radius = 20.0f * anim::ease(anim::Curve::EaseOut, appear);
  glyphs::face(canvas(), kCx, kCy, radius, face,
               hex(mix(theme::kBackground, theme::kSurface, appear)),
               hex(mix(theme::kBackground, theme::kTextFaint, appear)));
  drawZzz(kCx + radius + 6.0f, kCy - radius * 0.6f, now, appear);
}

void Renderer::drawPairing(const app::DeskApp& app, Instant now) {
  const scene::StageScene& stage = app.stage();
  const float x = (1.0f - stage.pairing()) * layout::kScreenWidth;
  canvas().fillRect(px(x), 0, px(layout::kScreenWidth) + 8, px(layout::kScreenHeight),
                    hex(theme::kBackground));
  canvas().drawFastVLine(px(x), 0, px(layout::kScreenHeight), hex(theme::kHairline));

  const float blinkPhase = phase(now.sinceBoot, Millis{3'600});
  glyphs::Face face;
  face.eyesOpen = blinkPhase > 0.95f ? 0.1f : 1.0f;
  face.squash = 1.0f + 0.03f * std::sin(2.0f * kPi * phase(now.sinceBoot, Millis{2'800}));

  if (app.phase() == app::Phase::Pairing) {
    canvas().setFont(fonts().small());
    canvas().setTextDatum(textdatum_t::middle_center);
    canvas().setTextColor(hex(theme::kTextDim));
    canvas().drawString("Confirm this code on your Mac", px(x + 160.0f), 30);

    // Digits drop in one after another with a moook.
    char code[8];
    std::snprintf(code, sizeof(code), "%06u", static_cast<unsigned>(app.connection().passkey()));
    canvas().setFont(fonts().digits());
    const Millis since = now - stage.pairingSince();
    float digitX = x + 160.0f - 3.0f * 30.0f - 8.0f;
    for (int i = 0; i < 6; ++i) {
      const Millis local = since - Millis{i * 60};
      const float dy = anim::moook(anim::kMoook, -24.0f, 0.0f, std::max(Millis{0}, local));
      const float alpha = progress(std::max(Millis{0}, local), Millis{120});
      const char digit[2] = {code[i], '\0'};
      canvas().setTextColor(hex(mix(theme::kBackground, theme::kText, alpha)));
      canvas().drawString(digit, px(digitX + 15.0f), px(92.0f + dy));
      digitX += 30.0f + (i == 2 ? 16.0f : 0.0f);
    }
    canvas().setFont(fonts().small());
    canvas().setTextColor(hex(theme::kTextFaint));
    canvas().drawString("Hold ↑ + ↓ for 5 s to start over", px(x + 160.0f), 150);
    return;
  }

  glyphs::face(canvas(), x + 62.0f, 78.0f, 22.0f, face, hex(theme::kSurface), hex(theme::kTextDim));
  canvas().setTextDatum(textdatum_t::middle_left);
  canvas().setFont(fonts().name());
  canvas().setTextColor(hex(theme::kText));
  canvas().drawString("Let's pair", px(x + 112.0f), 62);
  canvas().setFont(fonts().small());
  canvas().setTextColor(hex(theme::kTextDim));
  canvas().drawString("On your Mac: desksessions", px(x + 112.0f), 86);
  canvas().drawString("→ Pair New Clock", px(x + 112.0f), 102);
}

}  // namespace desk::ui
