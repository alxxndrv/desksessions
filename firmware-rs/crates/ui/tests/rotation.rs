//! The device draws into a portrait, byte-swapped buffer turned to landscape; the simulator (and
//! the golden stills) into a plain landscape one. Both must hold the same picture.

use desk_core::app::{DeskApp, Inputs, LinkEvent};
use desk_core::time::{Instant, Millis};
use desk_demo::Scenario;
use desk_ui::{Canvas, Renderer};

#[test]
fn portrait_device_buffer_holds_the_simulators_picture() {
    let mut now = Instant::from_millis(100_000);
    let mut scenario = Scenario::new(now);
    let mut app = DeskApp::default();
    app.set_time(12 * 3600, now);
    app.link_changed(LinkEvent::Connected, now);
    let mut landscape = Canvas::new(vec![0u16; 320 * 170], 320, 170);
    let mut portrait = Canvas::new(vec![0u16; 170 * 320], 170, 320).byte_swapped();
    let mut renderer = Renderer::new();
    let mut device_renderer = Renderer::new();

    for rotation in [3, 1] {
        portrait.set_rotation(rotation);
        for frame in 0..400 {
            now += Millis(16);
            if scenario.advance(now) {
                app.receive_snapshot(&scenario.snapshot(now), now);
            }
            let held = frame % 100 < 30;
            app.tick(Inputs { boot_down: false, aux_down: held }, now, &mut scenario);
            if frame % 7 != 0 {
                continue;
            }
            renderer.draw(&app, now, &mut landscape);
            device_renderer.draw(&app, now, &mut portrait);
            for y in 0..170 {
                for x in 0..320 {
                    assert_eq!(
                        portrait.get(x, y),
                        landscape.get(x, y),
                        "rotation {rotation}, frame {frame}, pixel ({x}, {y})"
                    );
                }
            }
        }
    }
}
