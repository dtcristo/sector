use bevy::prelude::Reflect;
use lazy_static::lazy_static;
use palette::{named::*, FromColor, Hsv, Pixel, Srgb};

#[derive(Reflect, Debug, Copy, Clone, Default, PartialEq, Eq)]
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

lazy_static! {
    pub static ref CEILING_COLOR: RawColor = SILVER.into();
    pub static ref FLOOR_COLOR: RawColor = GRAY.into();
    pub static ref AUTOMAP_WALL_COLOR: RawColor = DARKGRAY.into();
    pub static ref AUTOMAP_VISIBLE_WALL_COLOR: RawColor = *AUTOMAP_WALL_COLOR;
    pub static ref AUTOMAP_HIDDEN_WALL_COLOR: RawColor = *AUTOMAP_WALL_COLOR;
    pub static ref AUTOMAP_PORTAL_COLOR: RawColor = RED.into();
    pub static ref WALL_CLIPPED_COLOR: RawColor = WHITE.into();
    pub static ref FRUSTUM_COLOR: RawColor = WHITE.into();
    pub static ref PLAYER_COLOR: RawColor = BLUE.into();
    pub static ref MISSING_WALL_COLOR: RawColor = RED.into();
}
