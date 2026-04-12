use super::*;

use rust_bresenham::Bresenham;

pub fn draw_vertical_line(
    frame: &mut [u8],
    x: isize,
    y_top: isize,
    y_bottom: isize,
    color: RawColor,
) {
    for y in y_top..y_bottom {
        draw_pixel_unchecked(frame, Pixel::new(x, y), color);
    }
}

pub fn draw_line(frame: &mut [u8], a: Pixel, b: Pixel, color: RawColor) {
    for (x, y) in Bresenham::new(a.to_tuple(), b.to_tuple()) {
        draw_pixel(frame, Pixel::new(x, y), color);
    }
}

pub fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: RawColor) {
    if let Some(offset) = pixel.to_offset() {
        frame[offset..offset + 3].copy_from_slice(&color.0);
    }
}

pub fn draw_pixel_unchecked(frame: &mut [u8], pixel: Pixel, color: RawColor) {
    let offset = pixel.to_offset_unchecked();
    frame[offset..offset + 3].copy_from_slice(&color.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_vertical_line_colors_expected_span() {
        let mut frame = FrameBuffer::new();
        let color = RawColor([1, 2, 3]);

        draw_vertical_line(frame.as_mut_slice(), 10, 20, 23, color);

        assert_eq!(frame.pixel(10, 19), [0, 0, 0, 255]);
        assert_eq!(frame.pixel(10, 20), [1, 2, 3, 255]);
        assert_eq!(frame.pixel(10, 21), [1, 2, 3, 255]);
        assert_eq!(frame.pixel(10, 22), [1, 2, 3, 255]);
        assert_eq!(frame.pixel(10, 23), [0, 0, 0, 255]);
    }

    #[test]
    fn draw_pixel_colors_expected_location() {
        let mut frame = FrameBuffer::new();
        let color = RawColor([8, 9, 10]);

        draw_pixel(frame.as_mut_slice(), Pixel::new(5, 5), color);

        assert_eq!(frame.pixel(5, 5), [8, 9, 10, 255]);
        assert_eq!(frame.count_color(color), 1);
    }
}
