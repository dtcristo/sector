use crate::*;

// Pixel has origin at top left of screen.
//  .---> +x
//  |
//  v
//  +y
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub x: isize,
    pub y: isize,
}

impl Pixel {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    // From absolute
    pub fn from_abs(v: Vec2) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (MINIMAP_SCALE * v.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (MINIMAP_SCALE * v.y).round() as isize,
        }
    }

    // From normalized
    pub fn from_norm(v: Vec2) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * v.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * v.y).round() as isize,
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

    pub fn to_offset_unchecked(self) -> usize {
        (self.y as u32 * WIDTH * 4 + self.x as u32 * 4) as usize
    }
}
