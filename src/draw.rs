use crate::*;

use rust_bresenham::Bresenham;

pub fn draw_wall(
    frame: &mut [u8],
    view_z_left: f32,
    view_z_right: f32,
    normalized_left_top: Vec3,
    normalized_left_bottom: Vec3,
    normalized_right_top: Vec3,
    normalized_right_bottom: Vec3,
    color: Color,
    _texture: &RgbaImage,
) {
    let left_top = Pixel::from_normalized(normalized_left_top);
    let left_bottom = Pixel::from_normalized(normalized_left_bottom);
    let right_top = Pixel::from_normalized(normalized_right_top);
    let right_bottom = Pixel::from_normalized(normalized_right_bottom);

    // dbg!(left_top);
    // dbg!(left_bottom);
    // dbg!(right_top);
    // dbg!(right_bottom);

    let x_left = left_top.x;
    let z_left = view_z_left;
    let z_right = view_z_right;
    let dz = z_right - z_left;

    let color_hsla_raw = BevyColor::rgba_u8(color.0, color.1, color.2, color.3).as_hsla_f32();
    let ceiling_color = BevyColor::rgba_u8(0xc4, 0xc4, 0xc4, 0xff);
    let floor_color = BevyColor::rgba_u8(0x80, 0x80, 0x80, 0xff);

    if left_top.x != left_bottom.x || right_top.x != right_bottom.x {
        panic!("top of wall is not directly above bottom of wall");
    }

    let dy_top = right_top.y - left_top.y;
    let dy_bottom = right_bottom.y - left_bottom.y;
    let mut dx = right_top.x - left_top.x;
    if dx == 0 {
        dx = 1;
    }
    let dx_f32 = dx as f32;
    let xs = left_top.x;

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

    // dbg!(z_left);
    // dbg!(z_right);
    // dbg!(dz);
    // dbg!(dx_f32);

    for x in x1..(x2 - JOIN_GAP) {
        let progress = (x - x_left) as f32 / dx_f32;
        let z = progress * dz + z_left;

        let x_lightness = if z < Z_FAR {
            LIGHTNESS_FAR
        } else {
            z * (LIGHTNESS_FAR - LIGHTNESS_NEAR) / (Z_FAR - Z_NEAR)
                + (LIGHTNESS_NEAR * Z_FAR + LIGHTNESS_FAR * Z_NEAR) / (Z_FAR - Z_NEAR)
        };
        let x_lightness_rounded = (x_lightness * 100.0).ceil() / 100.0;

        let x_color = BevyColor::hsla(
            color_hsla_raw[0],
            color_hsla_raw[1],
            x_lightness_rounded,
            color_hsla_raw[3],
        );

        let x_minus_xs = x - xs;
        let y_top = dy_top * x_minus_xs / dx + left_top.y;
        let y_bottom = dy_bottom * x_minus_xs / dx + left_bottom.y;

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

        for y in EDGE_GAP..ceiling_bottom {
            draw_pixel_unchecked(frame, Pixel::new(x, y), ceiling_color);
        }

        // Wall
        for y in y1..(y2 - JOIN_GAP) {
            draw_pixel_unchecked(frame, Pixel::new(x, y), x_color);
        }

        // Floor
        let mut floor_top = y2;
        floor_top = if floor_top > EDGE_GAP {
            floor_top
        } else {
            EDGE_GAP
        };

        for y in floor_top..HEIGHT_MINUS_EDGE_GAP {
            draw_pixel_unchecked(frame, Pixel::new(x, y), floor_color);
        }
    }

    // Draw wall outline
    // draw_line(
    //     frame,
    //     Pixel::new(left_top.x, left_top.y),
    //     Pixel::new(right_top.x, right_top.y),
    //     Color(0xff, 0xff, 0xff, 0xff),
    // );
    // draw_line(
    //     frame,
    //     Pixel::new(left_bottom.x, left_bottom.y),
    //     Pixel::new(right_bottom.x, right_bottom.y),
    //     Color(0xff, 0xff, 0xff, 0xff),
    // );

    // draw_pixel(frame, left_top, Color(0x00, 0xff, 0x00, 0xff));
    // draw_pixel(frame, left_bottom, Color(0x00, 0xff, 0x00, 0xff));
    // draw_pixel(frame, right_top, Color(0x00, 0xff, 0x00, 0xff));
    // draw_pixel(frame, right_bottom, Color(0x00, 0xff, 0x00, 0xff));
}

pub fn draw_image(frame: &mut [u8], location: Pixel, image: &RgbaImage) {
    let frame_offset = location.to_offset().unwrap();
    for (row_index, row) in image
        .as_raw()
        .chunks(image.dimensions().1 as usize * 4)
        .enumerate()
    {
        frame[frame_offset + row_index * WIDTH as usize * 4
            ..frame_offset + row_index * WIDTH as usize * 4 + image.dimensions().1 as usize * 4]
            .copy_from_slice(row);
    }
}

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
