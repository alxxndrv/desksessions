//! Retained motion state: what moves on screen and how far each motion has got. No drawing.

mod board_scene;
mod stage_scene;

pub use board_scene::{BoardScene, RowSlot, SLOTS};
pub use stage_scene::StageScene;

/// Screen geometry shared by the scenes and the renderer.
pub mod layout {
    pub const SCREEN_WIDTH: f32 = 320.0;
    pub const SCREEN_HEIGHT: f32 = 170.0;
    pub const HEADER_HEIGHT: f32 = 20.0;
    pub const ROW_HEIGHT: f32 = 30.0;
    /// Five rows.
    pub const VIEWPORT_HEIGHT: f32 = SCREEN_HEIGHT - HEADER_HEIGHT;

    /// Rows slide in from the right…
    pub const ENTER_OFFSET: f32 = 48.0;
    /// …and leave to the left.
    pub const EXIT_OFFSET: f32 = -SCREEN_WIDTH;
    /// Pebble action-bar press offset.
    pub const NUDGE: f32 = 5.0;
    /// Rubber band before wrapping around.
    pub const WRAP_STRETCH: f32 = 6.0;
    /// Row brightening at full hold.
    pub const HOLD_LIFT: f32 = 0.6;
    /// The first quarter of a hold (150 ms) shows nothing, so plain taps never flash the fill.
    pub const HOLD_REVEAL: f32 = 0.25;
}
