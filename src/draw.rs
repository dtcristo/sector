use crate::*;

use rust_bresenham::Bresenham;

pub fn draw_vertical_line(frame: &mut [u8], x: isize, y_top: isize, y_bottom: isize, color: Color) {
    for y in y_top..y_bottom {
        draw_pixel_unchecked(frame, Pixel::new(x, y), color);
    }
}

// pub fn draw_image(frame: &mut [u8], location: Pixel, image: &RgbaImage) {
//     let frame_offset = location.to_offset().unwrap();
//     for (row_index, row) in image
//         .as_raw()
//         .chunks(image.dimensions().1 as usize * 4)
//         .enumerate()
//     {
//         frame[frame_offset + row_index * WIDTH as usize * 4
//             ..frame_offset + row_index * WIDTH as usize * 4 + image.dimensions().1 as usize * 4]
//             .copy_from_slice(row);
//     }
// }

pub fn draw_line(frame: &mut [u8], a: Pixel, b: Pixel, color: Color) {
    for (x, y) in Bresenham::new(a.to_tuple(), b.to_tuple()) {
        draw_pixel(frame, Pixel::new(x, y), color);
    }
}

pub fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: Color) {
    if let Some(offset) = pixel.to_offset() {
        frame[offset..offset + 4].copy_from_slice(&color.as_rgba_u32().to_le_bytes());
    }
}

pub fn draw_pixel_unchecked(frame: &mut [u8], pixel: Pixel, color: Color) {
    let offset = pixel.to_offset_unchecked();
    frame[offset..offset + 4].copy_from_slice(&color.as_rgba_u32().to_le_bytes());
}
