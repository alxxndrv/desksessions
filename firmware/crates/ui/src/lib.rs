//! Draws the app's scenes into an RGB565 frame buffer. Shared by the device and the simulator.

#![no_std]

pub mod canvas;
pub mod fonts;
pub mod glyphs;
pub mod meter;
pub mod renderer;
pub mod theme;
pub mod vlw;

pub use canvas::{Canvas, Datum, Rgb888, TextStyle, expand565, rgb565};
pub use fonts::Fonts;
pub use meter::FrameMeter;
pub use renderer::{Renderer, dim_for_backlight};
