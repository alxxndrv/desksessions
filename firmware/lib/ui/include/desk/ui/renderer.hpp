#pragma once

#include <LovyanGFX.hpp>
#include <optional>

#include "desk/app/button_gestures.hpp"
#include "desk/app/desk_app.hpp"
#include "desk/app/ports.hpp"
#include "desk/ui/canvas.hpp"
#include "desk/util/time.hpp"

namespace desk::ui {

/// Draws one frame of the app into a full-screen sprite.
class Renderer {
 public:
  explicit Renderer(Canvas& canvas) : canvas_(canvas) {}

  void draw(const app::DeskApp& app, Instant now, std::optional<app::WallTime> wallTime);

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

  Canvas& canvas_;
  app::Button lastHold_ = app::Button::Aux;
};

}  // namespace desk::ui
