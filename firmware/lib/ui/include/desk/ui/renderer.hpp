#pragma once

#include <LovyanGFX.hpp>
#include <optional>

#include "desk/app/button_gestures.hpp"
#include "desk/app/desk_app.hpp"
#include "desk/app/ports.hpp"
#include "desk/ui/canvas.hpp"
#include "desk/ui/fonts.hpp"
#include "desk/util/time.hpp"

namespace desk::ui {

/// Draws one frame of the app into a full-screen sprite.
class Renderer {
 public:
  explicit Renderer(Canvas& canvas) : target_(&canvas) {}

  /// Draws into the canvas given at construction.
  void draw(const app::DeskApp& app, Instant now);
  /// Draws into `target` and keeps using it (for double buffering).
  void draw(const app::DeskApp& app, Instant now, Canvas& target);

 private:
  void drawList(const app::DeskApp& app, Instant now);
  void drawRow(const app::DeskApp& app, const scene::RowSlot& slot, Instant now);
  void drawHeader(const app::DeskApp& app, Instant now, std::optional<app::WallTime> wallTime);
  void drawCounters(const app::DeskApp& app, float offsetY, float alpha);
  void drawAwayStatus(Instant now, float offsetY, float alpha);
  void drawPortHole(float progress);
  void drawQuiet(const app::DeskApp& app, Instant now);
  void drawAsleep(const app::DeskApp& app, Instant now);
  void drawPairing(const app::DeskApp& app, Instant now);
  void drawZzz(float x, float y, Instant now, float strength);

  /// Parsed on first use, after the platform's heap is up.
  const Fonts& fonts();

  Canvas& canvas() { return *target_; }

  Canvas* target_;
  std::optional<Fonts> fonts_;
  app::Button lastHold_ = app::Button::Aux;
};

}  // namespace desk::ui
