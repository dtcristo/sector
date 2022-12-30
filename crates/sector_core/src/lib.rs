use bevy::prelude::*;
use palette::{named::*, FromColor, Hsv, IntoColor, Pixel, Srgb};

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref MISSING_WALL_COLOR: RawColor = RED.into();
}

#[derive(Reflect, FromReflect, Debug, Copy, Clone, Default)]
pub struct RawColor(pub [u8; 3]);

impl From<Srgb<u8>> for RawColor {
    fn from(srgb: Srgb<u8>) -> Self {
        Self(srgb.into_raw())
    }
}

impl From<Hsv> for RawColor {
    fn from(hsv: Hsv) -> Self {
        Self(Srgb::from_color(hsv).into_format().into_raw())
    }
}

impl From<RawColor> for Srgb<u8> {
    fn from(raw_color: RawColor) -> Self {
        *Self::from_raw(&raw_color.0)
    }
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct InitialSector(pub Entity);

impl Default for InitialSector {
    fn default() -> Self {
        InitialSector(Entity::from_raw(u32::MAX))
    }
}

#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
pub struct Sector {
    pub vertices: Vec<Position2>,
    pub portal_sectors: Vec<Option<Entity>>,
    pub colors: Vec<RawColor>,
    pub floor: Length,
    pub ceil: Length,
}

impl Sector {
    pub fn to_walls(&self) -> Vec<Wall> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut portal_sector_iter = self.portal_sectors.iter();
        let mut color_iter = self.colors.iter();

        let Some(&initial) = vertex_iter.next() else { return walls };

        let mut add_wall = |left: Position2, right: Position2| {
            let raw_color = *color_iter.next().unwrap_or(&MISSING_WALL_COLOR);
            let hsv_color: Hsv = Srgb::<u8>::from(raw_color).into_format().into_color();
            walls.push(Wall {
                left,
                right,
                portal_sector: *portal_sector_iter.next().unwrap_or(&None),
                color: hsv_color,
            })
        };

        let mut previous = initial;
        for &vertex in vertex_iter {
            add_wall(previous, vertex);
            previous = vertex;
        }
        add_wall(previous, initial);

        walls
    }
}

pub struct Portal<'a> {
    pub sector: &'a Sector,
    pub x_min: isize,
    pub x_max: isize,
}

pub struct Wall {
    pub left: Position2,
    pub right: Position2,
    pub portal_sector: Option<Entity>,
    pub color: Hsv,
}

#[derive(Reflect, Debug, Copy, Clone, Default)]
pub struct Length(pub f32);

/// World position in 3D, right-handed coordinate system with z up.
///
///   +y
///   ^
///   |
/// +z.---> +x
#[derive(Debug, Copy, Clone)]
pub struct Position3(pub Vec3);

impl Position3 {
    pub fn truncate(self: Self) -> Position2 {
        Position2(self.0.truncate())
    }
}

/// World position in 2D.
///
///  +y
///  ^
///  |
///  .---> +x
#[derive(Reflect, FromReflect, Debug, Copy, Clone, Default)]
pub struct Position2(pub Vec2);

impl Position2 {
    pub fn transform(self: Self, matrix: Mat3) -> Self {
        Position2(matrix.transform_point2(self.0))
    }
}
