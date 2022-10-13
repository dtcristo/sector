use crate::{FRAC_HEIGHT_2, FRAC_WIDTH_2, HEIGHT, WIDTH};
use bevy::math::Vec3;

#[derive(Debug, Copy, Clone)]
pub struct Pixel {
    pub x: isize,
    pub y: isize,
}

impl Pixel {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    pub fn from_absolute(v: Vec3) -> Self {
        Self {
            x: v.x.floor() as isize + FRAC_WIDTH_2 as isize,
            y: v.z.floor() as isize + FRAC_HEIGHT_2 as isize,
        }
    }

    pub fn from_normalized(v: Vec3) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * v.x).floor() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * v.y).floor() as isize,
        }
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
