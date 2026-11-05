//! Composition root for the Heltec Vision Master T190.
//!
//! Built with the `demo` feature, a scripted pretend Mac stands in for Bluetooth so the clock can
//! be tried on its own. With `stats`, frame timing goes to USB serial every 5 s.

#![no_std]
#![no_main]

extern crate alloc;

mod backlight;
#[cfg(not(feature = "demo"))]
mod ble;
mod board;
mod display;
mod pump;
mod storage;

use allocator_api2::boxed::Box;
use desk_core::app::{DeskApp, DeviceSettings, Inputs};
use desk_core::time::{Instant, Millis};
use desk_ui::{Canvas, Renderer};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use esp_alloc::ExternalMemory;
use esp_hal::clock::CpuClock;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::ledc::channel::{self, ChannelIFace as _};
use esp_hal::ledc::timer::{self, TimerIFace as _};
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_storage::FlashStorage;
use static_cell::{ConstStaticCell, StaticCell};
use {esp_alloc as _, esp_backtrace as _};

use crate::backlight::Backlight;
use crate::board::{CANVAS_ROTATION, CANVAS_ROTATION_FLIPPED, PANEL_HEIGHT, PANEL_WIDTH};
use crate::display::{BAND_BYTES, Panel};
use crate::storage::Store;

esp_bootloader_esp_idf::esp_app_desc!();

type SharedStore = Mutex<CriticalSectionRawMutex, Store>;

/// A slider being dragged shouldn't write flash every frame.
const SAVE_AFTER: Millis = Millis(2_000);

fn now() -> Instant {
    Instant::from_millis(embassy_time::Instant::now().as_millis() as i64)
}

fn rotation(settings: &DeviceSettings) -> u8 {
    if settings.flip_screen { CANVAS_ROTATION_FLIPPED } else { CANVAS_ROTATION }
}

#[cfg(not(feature = "demo"))]
#[embassy_executor::task]
async fn ble_task(
    controller: trouble_host::prelude::ExternalController<
        esp_radio::ble::controller::BleConnector<'static>,
        20,
    >,
    address: [u8; 6],
    store: &'static SharedStore,
) {
    ble::run(controller, address, store).await;
}

#[allow(clippy::too_many_lines)]
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    // The ESP32-S3R8's 8 MB of octal PSRAM holds the frame: it doesn't fit in internal RAM next to
    // Bluetooth. Drawing goes through the cache; only the DMA bounce buffers must be internal.
    esp_alloc::psram_allocator!(
        peripherals.PSRAM,
        esp_hal::psram,
        esp_hal::psram::PsramConfig {
            mode: esp_hal::psram::PsramMode::OctalSpi,
            ram_frequency: esp_hal::psram::SpiRamFreq::Freq80m,
            ..Default::default()
        }
    );
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);
    #[cfg(feature = "stats")]
    embassy_time::Timer::after_millis(1_500).await; // let the port come up so the boot report is visible

    // Power: the external rail on, the panel on.
    let _vext = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let _tft_power = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let pull_up = InputConfig::default().with_pull(Pull::Up);
    let boot = Input::new(peripherals.GPIO0, pull_up);
    let aux = Input::new(peripherals.GPIO21, pull_up);
    embassy_time::Timer::after_millis(20).await;

    // Backlight: 12-bit LEDC at 8 kHz.
    static LEDC: StaticCell<Ledc<'static>> = StaticCell::new();
    let ledc = LEDC.init(Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    static LEDC_TIMER: StaticCell<timer::Timer<'static, LowSpeed>> = StaticCell::new();
    let ledc_timer = LEDC_TIMER.init(ledc.timer::<LowSpeed>(timer::Number::Timer0));
    let timer_ok = ledc_timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty12Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(8),
        })
        .is_ok();
    let mut channel = ledc.channel(channel::Number::Channel0, peripherals.GPIO17);
    let channel_ok = channel
        .configure(channel::config::Config {
            timer: ledc_timer,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .is_ok();
    if !(timer_ok && channel_ok) {
        esp_println::println!("[backlight] PWM unavailable");
    }
    let mut backlight = Backlight::new(channel);

    // Settings and the bond, from flash.
    static STORE: StaticCell<SharedStore> = StaticCell::new();
    let store: &'static SharedStore =
        STORE.init(Mutex::new(Store::new(FlashStorage::new(peripherals.FLASH))));
    let mut applied = store.lock().await.settings().await;

    // The panel: SPI2, 80 MHz, 3-wire, DMA.
    let Ok(spi) = Spi::new(peripherals.SPI2, display::write_config()) else {
        panic!("SPI");
    };
    let spi = spi
        .with_sck(peripherals.GPIO38)
        .with_sio0(peripherals.GPIO48)
        .with_dma(peripherals.DMA_CH0);
    let cs = Output::new(peripherals.GPIO39, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO47, Level::High, OutputConfig::default());
    let reset = Output::new(peripherals.GPIO40, Level::High, OutputConfig::default());
    let small_rx: DmaRxBuf = esp_hal::dma_rx_buffer!(32).expect("rx buffer");
    let small_tx: DmaTxBuf = esp_hal::dma_tx_buffer!(64).expect("tx buffer");
    let band_a = esp_hal::dma_tx_buffer!(BAND_BYTES).expect("band buffer");
    let band_b = esp_hal::dma_tx_buffer!(BAND_BYTES).expect("band buffer");
    let panel = Panel { spi, cs, dc, reset, small_tx, small_rx, bands: [band_a, band_b] };

    // The app, and whoever plays the Mac.
    // The app state and the renderer live in PSRAM too: in the main task they would crowd out the
    // stack, which gets whatever internal RAM is left.
    let mut app = Box::new_in(DeskApp::default(), ExternalMemory);
    app.apply_settings(applied);
    #[cfg(feature = "demo")]
    let mut link = {
        let mut scenario = Box::new_in(desk_demo::Scenario::new(now()), ExternalMemory);
        app.link_changed(desk_core::app::LinkEvent::Connected, now());
        app.set_time(12 * 3600, now()); // daytime, so the demo isn't dimmed
        scenario.advance(now());
        app.receive_snapshot(&scenario.snapshot(now()), now());
        scenario
    };
    #[cfg(not(feature = "demo"))]
    let mut link = {
        use esp_hal::efuse::{InterfaceMacAddress, interface_mac_address};
        let mac = interface_mac_address(InterfaceMacAddress::Bluetooth);
        let mut address = [0; 6];
        address.copy_from_slice(mac.as_bytes());
        let Ok(connector) =
            esp_radio::ble::controller::BleConnector::new(peripherals.BT, Default::default())
        else {
            panic!("BLE controller");
        };
        let controller = trouble_host::prelude::ExternalController::new(connector);
        spawner.spawn(ble_task(controller, address, store).expect("BLE task"));
        ble::UiLink
    };
    #[cfg(feature = "demo")]
    let _ = spawner;
    #[cfg(not(feature = "demo"))]
    let mut reassembler = desk_core::protocol::Reassembler::default();

    // The frame itself stays in internal RAM: drawing into PSRAM through the cache was twice as
    // slow, the rotated layout touching a new cache line on every pixel along a row.
    // (A const cell: building 108 KB of zeros on the stack first would overflow it.)
    static FRAME: ConstStaticCell<[u16; PANEL_WIDTH * PANEL_HEIGHT]> =
        ConstStaticCell::new([0; PANEL_WIDTH * PANEL_HEIGHT]);
    let frame = &mut FRAME.take()[..];
    let mut canvas = Canvas::new(frame, PANEL_WIDTH as i32, PANEL_HEIGHT as i32).byte_swapped();
    canvas.set_rotation(rotation(&applied));
    pump::start(panel, peripherals.CPU_CTRL, peripherals.FROM_CPU_INTR1);
    let mut renderer = Box::new_in(Renderer::new(), ExternalMemory);
    let mut unsaved_since: Option<Instant> = None;

    loop {
        let t = now();

        // What the Mac said.
        #[cfg(feature = "demo")]
        if link.advance(t) {
            app.receive_snapshot(&link.snapshot(t), t);
        }
        #[cfg(not(feature = "demo"))]
        while let Ok(message) = ble::INBOX.try_receive() {
            match message {
                ble::LinkMessage::Event(event) => app.link_changed(event, t),
                ble::LinkMessage::Chunk(chunk) => {
                    if let Some(frame) = reassembler.push(&chunk)
                        && let Some(decoded) = desk_core::protocol::decode_frame(frame)
                    {
                        app.receive(&decoded, t);
                    }
                }
            }
        }

        let inputs = Inputs { boot_down: boot.is_low(), aux_down: aux.is_low() };
        #[cfg(feature = "demo")]
        let level = app.tick(inputs, t, &mut *link);
        #[cfg(not(feature = "demo"))]
        let level = app.tick(inputs, t, &mut link);
        backlight.set(level);

        // Device settings: turn the picture at once, write flash once they stop changing.
        if *app.settings() != applied {
            if app.settings().flip_screen != applied.flip_screen {
                canvas.set_rotation(rotation(app.settings()));
            }
            applied = *app.settings();
            unsaved_since = Some(t);
        }
        if unsaved_since.is_some_and(|since| t - since >= SAVE_AFTER) {
            store.lock().await.save_settings(&applied).await;
            unsaved_since = None;
        }

        let drawing = embassy_time::Instant::now();
        renderer.draw(&app, t, &mut canvas);
        pump::hand_over(canvas.pixels(), drawing.elapsed().as_micros() as u32).await;
    }
}
