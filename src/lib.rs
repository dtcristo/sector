pub mod color;
#[cfg(feature = "sector")]
pub mod game;
pub mod geometry;
pub mod map;
pub mod player;
#[cfg(feature = "sector")]
pub mod render;
pub mod world;

pub use color::{
    RawColor, AUTOMAP_HIDDEN_WALL_COLOR, AUTOMAP_PORTAL_COLOR, AUTOMAP_VISIBLE_WALL_COLOR,
    AUTOMAP_WALL_COLOR, CEILING_COLOR, FLOOR_COLOR, FRUSTUM_COLOR, MISSING_WALL_COLOR,
    PLAYER_COLOR, WALL_CLIPPED_COLOR,
};
pub use geometry::{Length, Position2, Position3};
pub use world::{InitialSector, Sector, SectorId, WallSegment};

pub const DEFAULT_MAP_FILE_PATH: &str = "maps/default.map.ron";
