//! The application: buttons, connection phases, backlight and the [`DeskApp`] that ties them to
//! the board and its scenes.

mod backlight;
mod button_gestures;
mod connection;
mod desk_app;
mod wall_clock;

pub use backlight::{BacklightController, NightHours};
pub use button_gestures::{Button, ButtonGestures, Gesture, GestureKind, Hold};
pub use connection::{Connection, LinkEvent, Phase};
pub use desk_app::{DeskApp, Inputs};
pub use wall_clock::{WallClock, WallTime};

use crate::domain::Command;

/// Outbound side of the BLE link to the Mac.
pub trait MacLink {
    fn send(&mut self, command: Command);
    /// Wipes the bond and starts advertising for a new pairing.
    fn forget_pairing(&mut self);
}

/// How the clock sits on the desk. Set from the Mac app; the device keeps the last values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceSettings {
    /// Turn the picture 180°.
    pub flip_screen: bool,
    /// BOOT moves down, GPIO21 up.
    pub swap_buttons: bool,
    /// Percent.
    pub day_brightness: u8,
    /// Percent, during quiet hours.
    pub night_brightness: u8,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self { flip_screen: false, swap_buttons: false, day_brightness: 100, night_brightness: 10 }
    }
}
