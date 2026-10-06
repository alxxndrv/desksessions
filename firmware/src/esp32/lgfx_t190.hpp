#pragma once

#include <LovyanGFX.hpp>

#include "board.hpp"

namespace desk {

/// The T190's 1.9" ST7789: a 240×320 controller showing a 170×320 window, on SPI2 with DMA.
/// The backlight is driven separately (backlight_pwm.hpp) for finer dimming.
class LgfxT190 : public lgfx::LGFX_Device {
 public:
  LgfxT190() {
    {
      auto cfg = bus_.config();
      cfg.spi_host = SPI2_HOST;
      cfg.spi_mode = 0;
      cfg.freq_write = 80'000'000;  // a full frame in ~11 ms, faster than the panel scans
      cfg.freq_read = 6'000'000;  // ST7789 reads top out around 6.6 MHz
      cfg.spi_3wire = true;
      cfg.use_lock = true;
      cfg.dma_channel = SPI_DMA_CH_AUTO;
      cfg.pin_sclk = board::kTftSck;
      cfg.pin_mosi = board::kTftMosi;
      cfg.pin_miso = -1;
      cfg.pin_dc = board::kTftDc;
      bus_.config(cfg);
      panel_.setBus(&bus_);
    }
    {
      auto cfg = panel_.config();
      cfg.pin_cs = board::kTftCs;
      cfg.pin_rst = board::kTftReset;
      cfg.pin_busy = -1;
      cfg.memory_width = 240;
      cfg.memory_height = 320;
      cfg.panel_width = 170;
      cfg.panel_height = 320;
      cfg.offset_x = 35;
      cfg.offset_y = 0;
      cfg.offset_rotation = 0;
      cfg.readable = true;  // for the scanline read that stands in for the missing TE pin
      cfg.invert = true;
      cfg.rgb_order = false;
      cfg.dlen_16bit = false;
      cfg.bus_shared = false;
      panel_.config(cfg);
    }
    setPanel(&panel_);
  }

 private:
  lgfx::Bus_SPI bus_;
  lgfx::Panel_ST7789 panel_;
};

}  // namespace desk
