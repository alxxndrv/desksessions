//! Heltec Vision Master T190 wiring (from Heltec's schematic, as used by Meshtastic's variant).
//!
//!   BOOT button    GPIO0     Aux button      GPIO21
//!   Vext enable    GPIO5     (HIGH powers the external rail)
//!   TFT power      GPIO7     (LOW powers the panel)
//!   TFT SCK        GPIO38    TFT MOSI/MISO   GPIO48 (3-wire SPI)
//!   TFT DC         GPIO47    TFT CS          GPIO39
//!   TFT reset      GPIO40    TFT backlight   GPIO17 (HIGH = on, PWM for brightness)

/// The panel is driven in its native portrait order (so writes follow its scan); the frame
/// buffer is rotated to landscape. 3 is the right way up in the Moon Phase Clock case.
pub const CANVAS_ROTATION: u8 = 3;
pub const CANVAS_ROTATION_FLIPPED: u8 = 1;

/// The ST7789 is a 240×320 controller showing a 170×320 window.
pub const PANEL_WIDTH: usize = 170;
pub const PANEL_HEIGHT: usize = 320;
pub const PANEL_OFFSET_X: u16 = 35;
