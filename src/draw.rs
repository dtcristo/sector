use crate::*;

use rust_bresenham::Bresenham;

pub fn draw_wall(
    frame: &mut [u8],
    view_left_top: Vec3,
    view_left_bottom: Vec3,
    view_right_top: Vec3,
    view_right_bottom: Vec3,
    color: Color,
) {
    let normalized_left_top = PERSPECTIVE_MATRIX.project_point3(view_left_top);
    let normalized_left_bottom = PERSPECTIVE_MATRIX.project_point3(view_left_bottom);
    let normalized_right_top = PERSPECTIVE_MATRIX.project_point3(view_right_top);
    let normalized_right_bottom = PERSPECTIVE_MATRIX.project_point3(view_right_bottom);

    // dbg!(normalized_left_top);
    // dbg!(normalized_left_bottom);
    // dbg!(normalized_right_top);
    // dbg!(normalized_right_bottom);

    let left_top = Pixel::from_normalized(normalized_left_top);
    let left_bottom = Pixel::from_normalized(normalized_left_bottom);
    let right_top = Pixel::from_normalized(normalized_right_top);
    let right_bottom = Pixel::from_normalized(normalized_right_bottom);

    // dbg!(left_top);
    // dbg!(left_bottom);
    // dbg!(right_top);
    // dbg!(right_bottom);

    if left_top.x != left_bottom.x || right_top.x != right_bottom.x {
        panic!("top of wall is not directly above bottom of wall");
    }

    let dx = right_top.x - left_top.x;
    if dx <= 0 {
        // Right of wall side is on the left of left of wall, looking at back of wall, skip drawing
        return;
    }
    let dy_top = right_top.y - left_top.y;
    let dy_bottom = right_bottom.y - left_bottom.y;

    // Clip x
    let x1 = if left_top.x > EDGE_GAP {
        left_top.x
    } else {
        EDGE_GAP
    };
    let x2 = if right_top.x < WIDTH_MINUS_EDGE_GAP {
        right_top.x
    } else {
        WIDTH_MINUS_EDGE_GAP
    };

    let view_dz = view_right_top.z - view_left_top.z;
    // let view_y_middle = view_left_bottom.y + (view_y_top - view_left_bottom.y) / 2.0;

    let color_hsla_raw = BevyColor::rgba_u8(color.0, color.1, color.2, color.3).as_hsla_f32();
    let ceiling_color = BevyColor::rgba_u8(0xc4, 0xc4, 0xc4, 0xff);
    let floor_color = BevyColor::rgba_u8(0x80, 0x80, 0x80, 0xff);

    for x in x1..(x2 - JOIN_GAP) {
        let progress = (x - left_top.x) as f32 / dx as f32;
        let view_z = progress * view_dz + view_left_top.z;

        let distance = view_z.abs();

        let lightness = if distance > LIGHTNESS_DISTANCE_FAR {
            LIGHTNESS_FAR
        } else if distance < LIGHTNESS_DISTANCE_NEAR {
            LIGHTNESS_NEAR
        } else {
            distance * (LIGHTNESS_FAR - LIGHTNESS_NEAR) / DELTA_LIGHTNESS_DISTANCE
                + (LIGHTNESS_NEAR * LIGHTNESS_DISTANCE_FAR
                    + LIGHTNESS_FAR * LIGHTNESS_DISTANCE_NEAR)
                    / DELTA_LIGHTNESS_DISTANCE
        };
        let lightness_rounded = (lightness * 100.0).ceil() / 100.0;

        let x_color = BevyColor::hsla(
            color_hsla_raw[0],
            color_hsla_raw[1],
            lightness_rounded,
            color_hsla_raw[3],
        );

        let x_minus_x_left = x - left_top.x;
        let y_top = dy_top * x_minus_x_left / dx + left_top.y;
        let y_bottom = dy_bottom * x_minus_x_left / dx + left_bottom.y;

        // Clip y
        let y1 = if y_top > EDGE_GAP { y_top } else { EDGE_GAP };
        let y2 = if y_bottom < HEIGHT_MINUS_EDGE_GAP {
            y_bottom
        } else {
            HEIGHT_MINUS_EDGE_GAP
        };

        // Ceiling
        let mut ceiling_bottom = y1 - JOIN_GAP;
        ceiling_bottom = if ceiling_bottom < HEIGHT_MINUS_EDGE_GAP {
            ceiling_bottom
        } else {
            HEIGHT_MINUS_EDGE_GAP
        };

        // Floor
        let mut floor_top = y2;
        floor_top = if floor_top > EDGE_GAP {
            floor_top
        } else {
            EDGE_GAP
        };

        draw_vertical_line(frame, x, EDGE_GAP, ceiling_bottom, ceiling_color);
        draw_vertical_line(frame, x, y1, y2 - JOIN_GAP, x_color);
        draw_vertical_line(frame, x, floor_top, HEIGHT_MINUS_EDGE_GAP, floor_color);
    }
}

pub fn draw_vertical_line(
    frame: &mut [u8],
    x: isize,
    y_top: isize,
    y_bottom: isize,
    color: BevyColor,
) {
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
        frame[offset..offset + 4].copy_from_slice(&[color.0, color.1, color.2, color.3]);
    }
}

pub fn draw_pixel_unchecked(frame: &mut [u8], pixel: Pixel, color: BevyColor) {
    let offset = pixel.to_offset_unchecked();
    frame[offset..offset + 4].copy_from_slice(&color.as_rgba_u32().to_le_bytes());
}
