//! Gets drawn frames to the panel from the second core, so drawing the next frame never waits
//! for the wire (the C++ firmware's sender task on the other core, kept).
//!
//! This core draws into the fast internal frame and copies it into a free PSRAM buffer; core 1
//! takes full buffers, waits for the panel's scan and streams them out, then hands them back.
//! Two buffers in a ring: neither side ever holds one the other needs.

use core::sync::atomic::{AtomicU32, Ordering};

use allocator_api2::boxed::Box;
use allocator_api2::vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use esp_alloc::ExternalMemory;
use esp_hal::peripherals::{CPU_CTRL, FROM_CPU_INTR1};
use esp_hal::system::Stack;
use static_cell::{ConstStaticCell, StaticCell};

use crate::board::{PANEL_HEIGHT, PANEL_WIDTH};
use crate::display::{Display, Panel};

type Buffer = Box<[u16], ExternalMemory>;

static FREE: Channel<CriticalSectionRawMutex, Buffer, 2> = Channel::new();
static FULL: Channel<CriticalSectionRawMutex, Buffer, 2> = Channel::new();

/// Drawing time, summed by the drawing core for the `stats` report.
static RENDER_US: AtomicU32 = AtomicU32::new(0);

/// Hands the panel to the second core, which sets it up and then sends whatever is drawn.
pub fn start(panel: Panel, cpu_control: CPU_CTRL<'static>, int1: FROM_CPU_INTR1<'static>) {
    for _ in 0..2 {
        let buffer = vec::from_elem_in(0u16, PANEL_WIDTH * PANEL_HEIGHT, ExternalMemory);
        let _ = FREE.try_send(buffer.into_boxed_slice());
    }
    static STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());
    esp_rtos::start_second_core(cpu_control, int1, STACK.take(), move || {
        static EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();
        EXECUTOR.init(esp_rtos::embassy::Executor::new()).run(|spawner| {
            spawner.spawn(sender(panel).expect("sender task"));
        });
    });
}

/// Passes a finished frame (portrait, panel byte order) to the second core. Waits only when both
/// buffers are queued, i.e. when drawing runs ahead of the panel.
pub async fn hand_over(frame: &[u16], render_us: u32) {
    RENDER_US.fetch_add(render_us, Ordering::Relaxed);
    let mut buffer = FREE.receive().await;
    buffer.copy_from_slice(frame);
    FULL.send(buffer).await;
}

#[embassy_executor::task]
async fn sender(panel: Panel) {
    // Async from here, so the SPI's interrupts are handled on this core.
    let (mut display, mut reset) = Display::new(panel);
    let blank = FREE.receive().await; // still zeros: black
    if display.init(&mut reset, &blank).await.is_err() {
        esp_println::println!("[display] init failed");
    }
    FREE.send(blank).await;

    let mut reported_at = embassy_time::Instant::now();
    loop {
        let buffer = FULL.receive().await;
        if display.present(&buffer).await.is_err() {
            esp_println::println!("[display] frame lost");
        }
        FREE.send(buffer).await;

        if cfg!(feature = "stats") && reported_at.elapsed().as_millis() >= 5_000 {
            let stats = core::mem::take(&mut display.stats);
            let render_us = RENDER_US.swap(0, Ordering::Relaxed);
            let frames = stats.frames.max(1);
            let seconds = reported_at.elapsed().as_millis() as f32 / 1000.0;
            esp_println::println!(
                "[frames] {:.1} fps, render {:.1} ms, send {:.1} ms, scan wait {:.1} ms, scanline {}",
                stats.frames as f32 / seconds,
                (render_us / frames) as f32 / 1000.0,
                (stats.send_us / u64::from(frames)) as f32 / 1000.0,
                (stats.scan_wait_us / u64::from(frames)) as f32 / 1000.0,
                if display.scanline_works() { "ok" } else { "unavailable" },
            );
            reported_at = embassy_time::Instant::now();
        }
    }
}
