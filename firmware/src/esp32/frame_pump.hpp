#pragma once

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

#include <LovyanGFX.hpp>
#include <array>
#include <atomic>
#include <cstdint>

#include "desk/ui/canvas.hpp"

namespace desk::esp32 {

/// Gets frames onto the panel without tearing, although the T190 doesn't wire the panel's TE
/// pin, and without making drawing wait for the wire:
///  - frames live in the panel's native portrait layout, so a transfer runs top to bottom in
///    the same direction as the panel's scan;
///  - the scanline is read back (ST7789 command 0x45) and a transfer only starts where its
///    faster 80 MHz write can't catch up with the scan, so the two never cross;
///  - two buffers and a sender task on the other core: the UI draws frame N+1 while frame N
///    goes out. The second buffer is in PSRAM, which the ESP32-S3's DMA reads too;
///  - drawing is released once per panel refresh, so motion is sampled at the display's own
///    steady 60 Hz instead of beating against it.
/// Falls back to one buffer, or to clock pacing, if memory or the scanline read isn't there.
class FramePump {
 public:
  explicit FramePump(lgfx::LGFX_Device& lcd)
      : lcd_(lcd), buffers_{ui::Canvas(&lcd), ui::Canvas(&lcd)} {}

  void begin(std::uint8_t rotation) {
    Serial.printf("[heap] internal free %u, largest DMA block %u, psram free %u\n",
                  heap_caps_get_free_size(MALLOC_CAP_INTERNAL),
                  heap_caps_get_largest_free_block(MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL),
                  heap_caps_get_free_size(MALLOC_CAP_SPIRAM));
    for (auto& buffer : buffers_) {
      buffer.setColorDepth(16);
    }
    // Two 108 KB frames don't fit in internal RAM next to Bluetooth.
    buffers_[0].setPsram(false);
    buffers_[0].createSprite(170, 320);
    buffers_[1].setPsram(true);
    doubleBuffered_ = buffers_[1].createSprite(170, 320) != nullptr;
    setRotation(rotation);
    scanlineWorks_ = probeScanline();

    go_ = xSemaphoreCreateBinary();
    idle_ = xSemaphoreCreateBinary();
    xSemaphoreGive(idle_);
    // Core 0 beside the BLE host, below it in priority so Bluetooth always wins.
    xTaskCreatePinnedToCore(&FramePump::senderTask, "frames", 4096, this, 1, nullptr, 0);
  }

  void setRotation(std::uint8_t rotation) {
    for (auto& buffer : buffers_) {
      buffer.setRotation(rotation);
    }
  }

  /// The buffer to draw the next frame into (landscape coordinates).
  ui::Canvas& back() { return buffers_[doubleBuffered_ ? backIndex_ : 0]; }

  /// Marks the start of drawing the next frame (for the timing report).
  void beginFrame() { frameStart_ = micros(); }

  /// Hands the finished back buffer to the sender and moves on to the other one.
  void present() {
    renderMicros_ += micros() - frameStart_;
    const std::uint32_t waitStart = micros();
    xSemaphoreTake(idle_, portMAX_DELAY);  // the previous frame has left the building
    blockedMicros_ += micros() - waitStart;

    sending_ = doubleBuffered_ ? backIndex_ : 0;
    xSemaphoreGive(go_);
    if (doubleBuffered_) {
      backIndex_ ^= 1;
    } else {
      // The only buffer must not be drawn over mid-transfer.
      xSemaphoreTake(idle_, portMAX_DELAY);
      xSemaphoreGive(idle_);
    }
    ++frames_;
    report();
  }

 private:
  /// The write chases nothing as long as it starts with the scan far enough down: the DMA
  /// fills 320 rows in ~11 ms while the panel scans ~330 lines in ~16.5 ms, so a write that
  /// starts when the scan is past this line finishes before it could catch up with it.
  static constexpr std::int32_t kSafeFromLine = 120;

  static void senderTask(void* self) { static_cast<FramePump*>(self)->sendForever(); }

  [[noreturn]] void sendForever() {
    std::uint32_t lastSent = 0;
    for (;;) {
      xSemaphoreTake(go_, portMAX_DELAY);
      const std::uint32_t waitStart = micros();
      if (scanlineWorks_) {
        waitForSafeScanline();
      } else {
        while (micros() - lastSent < 16'667) {  // no scanline: hold ~60 fps by the clock
          vTaskDelay(1);
        }
      }
      scanWaitMicros_ += micros() - waitStart;
      lastSent = micros();

      auto& frame = buffers_[sending_];
      lcd_.startWrite();
      lcd_.pushImageDMA(0, 0, 170, 320, static_cast<const lgfx::swap565_t*>(frame.getBuffer()));
      lcd_.waitDMA();
      lcd_.endWrite();
      sendMicros_ += micros() - lastSent;
      if (scanlineWorks_) {
        // One frame per refresh: release the next one when the panel starts its next pass, so
        // frames are drawn at the panel's own steady cadence and none is drawn only to be lost.
        waitForWrap();
      }
      xSemaphoreGive(idle_);
    }
  }

  /// Blocks while the scan is in the band where starting a write would tear.
  void waitForSafeScanline() {
    const std::uint32_t deadline = micros() + 10'000;
    while (static_cast<std::int32_t>(deadline - micros()) > 0) {
      const std::int32_t line = lcd_.getScanLine();
      if (line == 0 || line >= kSafeFromLine) {
        return;
      }
    }
  }

  /// Blocks until the scan wraps back to the top (or a refresh's worth of time passes).
  void waitForWrap() {
    std::int32_t last = lcd_.getScanLine();
    const std::uint32_t deadline = micros() + 20'000;
    while (static_cast<std::int32_t>(deadline - micros()) > 0) {
      const std::int32_t line = lcd_.getScanLine();
      if (line < last) {
        return;
      }
      last = line;
    }
  }

  /// Watches ~3 refreshes: a working read climbs through the lines and wraps around.
  bool probeScanline() {
    std::int32_t last = lcd_.getScanLine();
    int climbs = 0;
    int wraps = 0;
    const std::uint32_t until = micros() + 50'000;
    while (static_cast<std::int32_t>(until - micros()) > 0) {
      delayMicroseconds(100);
      const std::int32_t line = lcd_.getScanLine();
      if (line < 0 || line > 400) {
        continue;
      }
      climbs += line > last ? 1 : 0;
      wraps += line < last ? 1 : 0;
      last = line;
    }
    Serial.printf("[vsync] scanline probe: %d climbs, %d wraps\n", climbs, wraps);
    return climbs > 20 && wraps >= 1;
  }

  /// Timing report over USB serial every 5 s, in the t190_stats build only.
  void report() {
#ifndef DESK_FRAME_STATS
    return;
#endif
    const std::uint32_t now = millis();
    if (now - reportedAt_ < 5'000) {
      return;
    }
    const float seconds = static_cast<float>(now - reportedAt_) / 1000.0f;
    const float perFrame = 1000.0f * static_cast<float>(frames_ ? frames_ : 1);
    Serial.printf(
        "[frames] %.1f fps, render %.1f ms, blocked %.1f ms, send %.1f ms, scan wait %.1f ms, "
        "scanline %s, %s buffer\n",
        static_cast<float>(frames_) / seconds, static_cast<float>(renderMicros_) / perFrame,
        static_cast<float>(blockedMicros_) / perFrame, static_cast<float>(sendMicros_) / perFrame,
        static_cast<float>(scanWaitMicros_) / perFrame, scanlineWorks_ ? "ok" : "unavailable",
        doubleBuffered_ ? "double" : "single");
    frames_ = 0;
    renderMicros_ = 0;
    blockedMicros_ = 0;
    sendMicros_ = 0;
    scanWaitMicros_ = 0;
    reportedAt_ = now;
  }

  lgfx::LGFX_Device& lcd_;
  std::array<ui::Canvas, 2> buffers_;
  bool doubleBuffered_ = false;
  bool scanlineWorks_ = false;
  std::uint8_t backIndex_ = 0;
  std::atomic<std::uint8_t> sending_{0};
  SemaphoreHandle_t go_ = nullptr;
  SemaphoreHandle_t idle_ = nullptr;

  // Timing report (the sender's counters are written on core 0, read approximately on core 1).
  std::uint32_t frames_ = 0;
  std::uint32_t frameStart_ = 0;
  std::uint32_t renderMicros_ = 0;
  std::uint32_t blockedMicros_ = 0;
  std::atomic<std::uint32_t> sendMicros_{0};
  std::atomic<std::uint32_t> scanWaitMicros_{0};
  std::uint32_t reportedAt_ = 0;
};

}  // namespace desk::esp32
