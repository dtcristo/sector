use super::{HEIGHT, WIDTH};
use crate::RawColor;

pub const FRAME_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub x: isize,
    pub y: isize,
}

impl Pixel {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    pub fn to_tuple(self) -> (isize, isize) {
        (self.x, self.y)
    }

    pub fn to_offset(self) -> Option<usize> {
        if self.x >= 0 && self.x < WIDTH as isize && self.y >= 0 && self.y < HEIGHT as isize {
            Some((self.y as u32 * WIDTH * 4 + self.x as u32 * 4) as usize)
        } else {
            None
        }
    }
}

pub struct FrameBuffer {
    bytes: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        let mut buffer = Self {
            bytes: vec![0; FRAME_BYTES],
        };
        clear_frame(buffer.as_mut_slice());
        buffer
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let offset = (y * WIDTH as usize + x) * 4;
        [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
        ]
    }

    pub fn count_color(&self, color: RawColor) -> usize {
        self.bytes
            .chunks_exact(4)
            .filter(|chunk| {
                chunk[0] == color.0[0] && chunk[1] == color.0[1] && chunk[2] == color.0[2]
            })
            .count()
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn clear_frame(frame: &mut [u8]) {
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

pub(crate) fn draw_line(frame: &mut [u8], a: Pixel, b: Pixel, color: RawColor) {
    let mut x = a.x;
    let mut y = a.y;
    let dx = (b.x - a.x).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let dy = -(b.y - a.y).abs();
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        draw_pixel(frame, Pixel::new(x, y), color);
        if x == b.x && y == b.y {
            break;
        }

        let doubled_error = error * 2;
        if doubled_error >= dy {
            error += dy;
            x += sx;
        }
        if doubled_error <= dx {
            error += dx;
            y += sy;
        }
    }
}

pub(crate) fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: RawColor) {
    if let Some(offset) = pixel.to_offset() {
        frame[offset..offset + 3].copy_from_slice(&color.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_pixel_colors_expected_location() {
        let mut frame = FrameBuffer::new();
        let color = RawColor([8, 9, 10]);

        draw_pixel(frame.as_mut_slice(), Pixel::new(5, 5), color);

        assert_eq!(frame.pixel(5, 5), [8, 9, 10, 255]);
        assert_eq!(frame.count_color(color), 1);
    }

    #[test]
    fn draw_line_covers_every_row_for_steep_slopes() {
        let mut frame = FrameBuffer::new();
        let color = RawColor([5, 6, 7]);

        draw_line(
            frame.as_mut_slice(),
            Pixel::new(10, 10),
            Pixel::new(12, 18),
            color,
        );

        for y in 10..=18 {
            assert!(
                (10..=12).any(|x| frame.pixel(x as usize, y as usize) == [5, 6, 7, 255]),
                "expected row {y} to contain part of the line"
            );
        }
    }
}
