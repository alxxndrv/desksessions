//! Captures frames: PNG stills, or a GIF or MP4 (by extension) through ffmpeg.

use std::io::Write as _;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::{HEIGHT, WIDTH};

/// Stills and clips are scaled up 3× with nearest-neighbour so pixels stay crisp.
pub const SCALE: usize = 3;

/// `rgb` (packed, `WIDTH × HEIGHT`) scaled up by [`SCALE`].
#[must_use]
pub fn upscale(rgb: &[u8]) -> Vec<u8> {
    let (w, h) = (WIDTH * SCALE, HEIGHT * SCALE);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let i = ((y / SCALE) * WIDTH + x / SCALE) * 3;
            out.extend_from_slice(&rgb[i..i + 3]);
        }
    }
    out
}

/// Writes packed RGB as a PNG.
///
/// # Errors
///
/// When the file can't be written.
pub fn write_png(path: &Path, width: usize, height: usize, rgb: &[u8]) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer.write_image_data(rgb).map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

/// Reads a PNG as packed RGB with its size.
///
/// # Errors
///
/// When the file is missing or isn't 8-bit RGB.
pub fn read_png(path: &Path) -> std::io::Result<(usize, usize, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info().map_err(std::io::Error::other)?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(std::io::Error::other)?;
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Eight {
        return Err(std::io::Error::other("want 8-bit RGB"));
    }
    buf.truncate(info.buffer_size());
    Ok((info.width as usize, info.height as usize, buf))
}

/// A clip being written by ffmpeg.
#[derive(Default)]
pub struct Recorder {
    child: Option<Child>,
}

impl Recorder {
    /// A 3× PNG still of `rgb`.
    ///
    /// # Errors
    ///
    /// When the file can't be written.
    pub fn still(rgb: &[u8], path: &Path) -> std::io::Result<()> {
        write_png(path, WIDTH * SCALE, HEIGHT * SCALE, &upscale(rgb))
    }

    /// Starts piping frames into ffmpeg: H.264 for `.mp4`, a per-clip palette GIF otherwise.
    ///
    /// # Errors
    ///
    /// When ffmpeg can't be started.
    pub fn start(&mut self, path: &str, fps: u32) -> std::io::Result<()> {
        self.stop();
        let scale = "scale=iw*3:ih*3:flags=neighbor";
        let input = format!(
            "ffmpeg -loglevel error -y -f rawvideo -pix_fmt rgb24 -s {WIDTH}x{HEIGHT} -r {fps} -i - -vf '{scale}"
        );
        let command = if path.ends_with(".mp4") && path.len() > 4 {
            format!(
                "{input}' -c:v libx264 -preset slow -crf 16 -pix_fmt yuv420p -movflags +faststart '{path}'"
            )
        } else {
            format!(
                "{input},split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=none' '{path}'"
            )
        };
        self.child = Some(Command::new("sh").arg("-c").arg(command).stdin(Stdio::piped()).spawn()?);
        Ok(())
    }

    #[must_use]
    pub fn recording(&self) -> bool {
        self.child.is_some()
    }

    /// Appends a frame of packed RGB.
    pub fn frame(&mut self, rgb: &[u8]) {
        if let Some(stdin) = self.child.as_mut().and_then(|c| c.stdin.as_mut())
            && stdin.write_all(rgb).is_err()
        {
            self.stop();
        }
    }

    /// Finishes the clip.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}
