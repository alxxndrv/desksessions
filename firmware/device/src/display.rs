//! The T190's 1.9" ST7789 on SPI2 with DMA, 3-wire (one bidirectional data line), 80 MHz.
//!
//! Frames go out without tearing although the board doesn't wire the panel's TE pin:
//!  - frames are kept in the panel's native portrait layout, so a transfer runs top to bottom in
//!    the same direction as the panel's scan;
//!  - the scanline is read back (command 0x45) and a transfer only starts where the faster
//!    80 MHz write can't catch up with the scan, so the two never cross;
//!  - one frame per panel refresh: the next frame is released when the scan wraps, so motion is
//!    sampled at the display's own steady 60 Hz;
//!  - the frame streams out through two DMA bounce buffers, five 64-row bands, so drawing the
//!    next frame waits only for the copy, not the wire. (One SPI DMA transfer is at most 32 KB.)
//!
//! Falls back to clock pacing if the scanline read doesn't work.

use embassy_time::{Duration, Instant, Timer};
use esp_hal::delay::Delay;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::Output;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Address, Command, Config, DataMode, SpiDma, SpiDmaTransfer};
use esp_hal::time::Rate;
use esp_hal::{Async, Blocking};

use crate::board::{PANEL_HEIGHT, PANEL_OFFSET_X, PANEL_WIDTH};

const BAND_ROWS: usize = 64;
pub const BAND_BYTES: usize = BAND_ROWS * PANEL_WIDTH * 2;
const BANDS: usize = PANEL_HEIGHT / BAND_ROWS;

/// The write chases nothing as long as it starts with the scan far enough down: the DMA fills
/// 320 rows in ~11 ms while the panel scans ~330 lines in ~16.5 ms, so a write that starts when
/// the scan is past this line finishes before it could catch up with it.
const SAFE_FROM_LINE: u16 = 120;
const FRAME: Duration = Duration::from_micros(16_667);

/// Writes go at 80 MHz, a full frame in ~11 ms; ST7789 reads top out around 6.6 MHz.
pub fn write_config() -> Config {
    Config::default().with_frequency(Rate::from_mhz(80)).with_mode(Mode::_0)
}

fn read_config() -> Config {
    Config::default().with_frequency(Rate::from_mhz(6)).with_mode(Mode::_0)
}

// ST7789 commands.
const SLPOUT: u8 = 0x11;
const INVON: u8 = 0x21;
const DISPON: u8 = 0x29;
const CASET: u8 = 0x2A;
const RASET: u8 = 0x2B;
const RAMWR: u8 = 0x2C;
const IDMOFF: u8 = 0x38;
const MADCTL: u8 = 0x36;
const COLMOD: u8 = 0x3A;
const GSCAN: u8 = 0x45;
const MAD_BGR: u8 = 0x08;
const RGB565: u8 = 0x55;

/// LovyanGFX's ST7789 setup, step for step: gate, VCOM, power and gamma, then wake.
const INIT: &[(u8, &[u8])] = &[
    (0xB7, &[0x35]),
    (0xBB, &[0x28]),
    (0xC0, &[0x0C]),
    (0xC2, &[0x01, 0xFF]),
    (0xC3, &[0x10]),
    (0xC4, &[0x20]),
    (0xD0, &[0xA4, 0xA1]),
    (0xB0, &[0x00, 0xC0]),
    (0xE0, &[0xD0, 0x00, 0x02, 0x07, 0x0A, 0x28, 0x32, 0x44, 0x42, 0x06, 0x0E, 0x12, 0x14, 0x17]),
    (0xE1, &[0xD0, 0x00, 0x02, 0x07, 0x0A, 0x28, 0x31, 0x54, 0x47, 0x0E, 0x1C, 0x17, 0x1B, 0x1E]),
];

#[derive(Debug)]
pub enum Error {
    Spi,
}

/// Timing of the last few seconds of frames, for the `stats` build.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub frames: u32,
    pub send_us: u64,
    pub scan_wait_us: u64,
}

/// The panel's wiring, before it is set up: what crosses to the core that drives it.
pub struct Panel {
    pub spi: SpiDma<'static, Blocking>,
    pub cs: Output<'static>,
    pub dc: Output<'static>,
    pub reset: Output<'static>,
    pub small_tx: DmaTxBuf,
    pub small_rx: DmaRxBuf,
    pub bands: [DmaTxBuf; 2],
}

pub struct Display {
    spi: Option<SpiDma<'static, Async>>,
    cs: Output<'static>,
    dc: Output<'static>,
    small_tx: Option<DmaTxBuf>,
    small_rx: Option<DmaRxBuf>,
    bands: [Option<DmaTxBuf>; 2],
    /// The frame's last band, still on the wire, and the slot its buffer goes back to.
    pending: Option<(SpiDmaTransfer<'static, Async, DmaTxBuf>, usize)>,
    scanline_works: bool,
    last_sent: Instant,
    pub stats: Stats,
}

impl Display {
    /// Takes over `panel` on the current core. Returns the reset line for [`Display::init`].
    #[must_use]
    pub fn new(panel: Panel) -> (Self, Output<'static>) {
        let [a, b] = panel.bands;
        let display = Self {
            spi: Some(panel.spi.into_async()),
            cs: panel.cs,
            dc: panel.dc,
            small_tx: Some(panel.small_tx),
            small_rx: Some(panel.small_rx),
            bands: [Some(a), Some(b)],
            pending: None,
            scanline_works: false,
            last_sent: Instant::now(),
            stats: Stats::default(),
        };
        (display, panel.reset)
    }

    /// Resets and configures the panel, shows `blank` (a whole frame), and checks the scanline
    /// read.
    pub async fn init(&mut self, reset: &mut Output<'static>, blank: &[u16]) -> Result<(), Error> {
        let delay = Delay::new();
        reset.set_low();
        delay.delay_millis(8);
        reset.set_high();
        Timer::after_millis(64).await;

        for (cmd, params) in INIT {
            self.command(*cmd, params).await?;
        }
        self.command(SLPOUT, &[]).await?;
        Timer::after_millis(130).await;
        self.command(IDMOFF, &[]).await?;
        self.command(DISPON, &[]).await?;
        self.command(INVON, &[]).await?;
        self.command(COLMOD, &[RGB565]).await?;
        self.command(MADCTL, &[MAD_BGR]).await?;

        self.send_frame(blank).await?;
        self.scanline_works = self.probe_scanline().await;
        esp_println::println!(
            "[vsync] scanline {}",
            if self.scanline_works { "ok" } else { "unavailable: pacing by the clock" }
        );
        Ok(())
    }

    /// Lets the last frame's final band finish and releases the chip select it held.
    async fn settle(&mut self) {
        if let Some((transfer, slot)) = self.pending.take() {
            self.finish(transfer, slot).await;
            self.cs.set_high();
        }
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let mut buf = self.small_tx.take().ok_or(Error::Spi)?;
        buf.fill(bytes);
        let spi = self.spi.take().ok_or(Error::Spi)?;
        let result = spi.half_duplex_write_buffer(
            DataMode::Single,
            Command::None,
            Address::None,
            0,
            bytes.len(),
            buf,
        );
        let mut transfer = match result {
            Ok(t) => t,
            Err((_, spi, buf)) => {
                self.spi = Some(spi);
                self.small_tx = Some(buf);
                return Err(Error::Spi);
            }
        };
        transfer.wait_for_done().await;
        let (spi, buf) = transfer.wait();
        self.spi = Some(spi);
        self.small_tx = Some(buf);
        Ok(())
    }

    /// One command byte, then its parameters, with the chip selected throughout.
    async fn command(&mut self, cmd: u8, params: &[u8]) -> Result<(), Error> {
        self.settle().await;
        self.cs.set_low();
        self.dc.set_low();
        let result = self.write(&[cmd]).await;
        self.dc.set_high();
        let result = match result {
            Ok(()) if !params.is_empty() => self.write(params).await,
            other => other,
        };
        self.cs.set_high();
        result
    }

    /// The line the panel is scanning, if it answers: command 0x45, then (data/command high, at
    /// read speed) one dummy clock and two bytes back on the same data line.
    async fn scanline(&mut self) -> Option<u16> {
        self.settle().await;
        self.cs.set_low();
        self.dc.set_low();
        let sent = self.write(&[GSCAN]).await;
        self.dc.set_high();
        let line = if sent.is_ok() { self.read_two().await } else { None };
        self.cs.set_high();
        line
    }

    async fn read_two(&mut self) -> Option<u16> {
        let mut spi = self.spi.take()?;
        let Some(mut rx) = self.small_rx.take() else {
            self.spi = Some(spi);
            return None;
        };
        let _ = spi.apply_config(&read_config());
        rx.set_length(2);
        let (mut spi, rx, line) = match spi.half_duplex_read_buffer(
            DataMode::Single,
            Command::None,
            Address::None,
            1,
            2,
            rx,
        ) {
            Ok(mut transfer) => {
                transfer.wait_for_done().await;
                let (spi, rx) = transfer.wait();
                let bytes = rx.as_slice();
                let line = u16::from(bytes[0]) << 8 | u16::from(bytes[1]);
                (spi, rx, Some(line))
            }
            Err((_, spi, rx)) => (spi, rx, None),
        };
        let _ = spi.apply_config(&write_config());
        self.spi = Some(spi);
        self.small_rx = Some(rx);
        line
    }

    /// Watches ~3 refreshes: a working read climbs through the lines and wraps around.
    async fn probe_scanline(&mut self) -> bool {
        let Some(mut last) = self.scanline().await else { return false };
        let (mut climbs, mut wraps) = (0, 0);
        let until = Instant::now() + Duration::from_millis(50);
        while Instant::now() < until {
            Timer::after_micros(100).await;
            let Some(line) = self.scanline().await else { continue };
            if line > 400 {
                continue;
            }
            climbs += i32::from(line > last);
            wraps += i32::from(line < last);
            last = line;
        }
        esp_println::println!("[vsync] scanline probe: {climbs} climbs, {wraps} wraps");
        climbs > 20 && wraps >= 1
    }

    /// Waits while the scan is in the band where starting a write would tear.
    async fn wait_for_safe_scanline(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(10);
        while Instant::now() < deadline {
            match self.scanline().await {
                Some(line) if line == 0 || line >= SAFE_FROM_LINE => return,
                Some(_) => {}
                None => return,
            }
        }
    }

    /// Waits until the scan wraps back to the top (or a refresh's worth of time passes).
    async fn wait_for_wrap(&mut self) {
        let Some(mut last) = self.scanline().await else { return };
        let deadline = Instant::now() + Duration::from_millis(20);
        while Instant::now() < deadline {
            let Some(line) = self.scanline().await else { return };
            if line < last {
                return;
            }
            last = line;
        }
    }

    /// Sends one frame (portrait, pixels already in the panel's byte order) at the panel's
    /// cadence. Returns once the frame has been copied out, while its last band is still on the
    /// wire, so the caller can start drawing the next one.
    pub async fn present(&mut self, frame: &[u16]) -> Result<(), Error> {
        let wait_start = Instant::now();
        if self.scanline_works {
            self.wait_for_wrap().await;
            self.wait_for_safe_scanline().await;
        } else {
            // No scanline: hold ~60 fps by the clock.
            Timer::at(self.last_sent + FRAME).await;
        }
        let sent = Instant::now();
        self.stats.scan_wait_us += (sent - wait_start).as_micros();
        self.last_sent = sent;
        self.send_frame(frame).await?;
        self.stats.send_us += sent.elapsed().as_micros();
        self.stats.frames += 1;
        Ok(())
    }

    async fn send_frame(&mut self, frame: &[u16]) -> Result<(), Error> {
        let x0 = PANEL_OFFSET_X;
        let x1 = PANEL_OFFSET_X + PANEL_WIDTH as u16 - 1;
        let y1 = PANEL_HEIGHT as u16 - 1;
        self.command(CASET, &[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8]).await?;
        self.command(RASET, &[0, 0, (y1 >> 8) as u8, y1 as u8]).await?;

        self.settle().await;
        self.cs.set_low();
        self.dc.set_low();
        self.write(&[RAMWR]).await?;
        self.dc.set_high();
        let bytes: &[u8] = bytemuck::cast_slice(frame);
        let mut in_flight = None;
        for band in 0..BANDS {
            let mut buf = self.bands[band % 2].take().ok_or(Error::Spi)?;
            buf.fill(&bytes[band * BAND_BYTES..(band + 1) * BAND_BYTES]);
            if let Some(transfer) = in_flight.take() {
                self.finish(transfer, (band + 1) % 2).await;
            }
            let spi = self.spi.take().ok_or(Error::Spi)?;
            match spi.half_duplex_write_buffer(
                DataMode::Single,
                Command::None,
                Address::None,
                0,
                BAND_BYTES,
                buf,
            ) {
                Ok(transfer) => in_flight = Some(transfer),
                Err((_, spi, buf)) => {
                    self.spi = Some(spi);
                    self.bands[band % 2] = Some(buf);
                    self.cs.set_high();
                    return Err(Error::Spi);
                }
            }
        }
        // The last band keeps going while the next frame is drawn; `settle` collects it.
        if let Some(transfer) = in_flight {
            self.pending = Some((transfer, (BANDS - 1) % 2));
        }
        Ok(())
    }

    async fn finish(
        &mut self,
        mut transfer: SpiDmaTransfer<'static, Async, DmaTxBuf>,
        slot: usize,
    ) {
        transfer.wait_for_done().await;
        let (spi, buf) = transfer.wait();
        self.spi = Some(spi);
        self.bands[slot] = Some(buf);
    }

    #[must_use]
    pub fn scanline_works(&self) -> bool {
        self.scanline_works
    }
}
