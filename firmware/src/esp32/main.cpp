// Composition root for the Heltec Vision Master T190.
//
// Built with DESK_DEMO (env t190_demo), a scripted pretend Mac stands in for Bluetooth so the
// clock can be tried on its own.

#include <Arduino.h>
#include <esp_timer.h>

#include <variant>

#include "ble_link.hpp"
#include "board.hpp"
#include "desk/app/desk_app.hpp"
#ifdef DESK_DEMO
#include "desk/demo/scenario.hpp"
#endif
#include "desk/protocol/wire.hpp"
#include "desk/ui/canvas.hpp"
#include "desk/ui/renderer.hpp"
#include "lgfx_t190.hpp"

namespace {

using namespace desk;

/// Backlight through the panel's PWM, with a perceptual curve so low levels stay visible.
class PanelBacklight final : public app::Backlight {
 public:
  explicit PanelBacklight(lgfx::LGFX_Device& lcd) : lcd_(lcd) {}

  void set(float level) override {
    const float clamped = std::clamp(level, 0.0f, 1.0f);
    const auto duty = static_cast<std::uint8_t>(4.0f + 251.0f * clamped * clamped);
    if (duty != last_) {
      lcd_.setBrightness(duty);
      last_ = duty;
    }
  }

 private:
  lgfx::LGFX_Device& lcd_;
  std::uint8_t last_ = 0;
};

Instant monotonicNow() { return Instant{Millis{esp_timer_get_time() / 1000}}; }

LgfxT190 lcd;
ui::Canvas canvas(&lcd);
PanelBacklight backlight(lcd);
#ifdef DESK_DEMO
demo::Scenario scenario(monotonicNow());
app::DeskApp deskApp(scenario, backlight);
#else
esp32::BleLink macLink;
app::DeskApp deskApp(macLink, backlight);
#endif
ui::Renderer renderer(canvas);
protocol::Reassembler reassembler;

constexpr Millis kFrame{16};
Instant nextFrame;

#ifdef DESK_DEMO
void drainLink(Instant now) {
  if (scenario.advance(now)) {
    deskApp.receive(scenario.snapshot(now), now);
  }
}
#else
void drainLink(Instant now) {
  esp32::LinkMessage message;
  while (macLink.pop(message)) {
    if (const auto* event = std::get_if<app::LinkEvent>(&message)) {
      deskApp.linkChanged(*event, now);
      continue;
    }
    const auto& chunk = std::get<esp32::Chunk>(message);
    if (const auto frame = reassembler.push(std::span(chunk.bytes.data(), chunk.size))) {
      if (const auto decoded = protocol::decodeFrame(*frame)) {
        deskApp.receive(*decoded, now);
      }
    }
  }
}
#endif

}  // namespace

void setup() {
  pinMode(board::kVextEnable, OUTPUT);
  digitalWrite(board::kVextEnable, HIGH);
  pinMode(board::kTftPower, OUTPUT);
  digitalWrite(board::kTftPower, LOW);
  pinMode(board::kBootButton, INPUT_PULLUP);
  pinMode(board::kAuxButton, INPUT_PULLUP);
  delay(20);

  lcd.init();
  lcd.setRotation(board::kRotation);
  lcd.setBrightness(0);
  canvas.setPsram(false);  // internal RAM so the frame can go out by DMA
  canvas.setColorDepth(16);
  canvas.createSprite(320, 170);

#ifdef DESK_DEMO
  deskApp.linkChanged({.kind = app::LinkEvent::Kind::Connected}, monotonicNow());
  deskApp.setTime(12 * 3600, monotonicNow());  // daytime, so the demo isn't dimmed
#else
  macLink.begin();
#endif
  nextFrame = monotonicNow();
}

void loop() {
  const Instant now = monotonicNow();
  drainLink(now);
  deskApp.tick({.bootDown = digitalRead(board::kBootButton) == LOW,
                .auxDown = digitalRead(board::kAuxButton) == LOW},
               now);
  renderer.draw(deskApp, now);
  canvas.pushSprite(0, 0);

  nextFrame = nextFrame + kFrame;
  const Millis wait = nextFrame - monotonicNow();
  if (wait > Millis{0}) {
    delay(static_cast<std::uint32_t>(wait.count()));
  } else {
    nextFrame = monotonicNow();  // fell behind: don't try to catch up
  }
}
