use crate::{InitialSector, Length, Position2, RawColor, Sector, SectorId};

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    math::{vec2, Vec2},
    prelude::*,
    reflect::TypePath,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Asset, TypePath, Debug, Clone, Serialize, Deserialize)]
pub struct SectorMap {
    pub initial_sector: usize,
    pub initial_position: MapVertex,
    #[serde(default)]
    pub initial_direction_degrees: f32,
    pub sectors: Vec<MapSector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSector {
    pub floor: f32,
    pub ceil: f32,
    pub vertices: Vec<MapVertex>,
    pub walls: Vec<MapWall>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MapVertex(pub f32, pub f32);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MapWall {
    pub color: [u8; 3],
    #[serde(default)]
    pub portal: Option<usize>,
}

impl SectorMap {
    pub fn initial_direction_radians(&self) -> f32 {
        self.initial_direction_degrees.to_radians()
    }
}

#[derive(Default, TypePath)]
pub struct SectorMapLoader;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum SectorMapError {
    #[error("Could not load map file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse RON map: {0}")]
    RonSpanned(#[from] ron::error::SpannedError),
    #[error("Could not serialize RON map: {0}")]
    RonSerialize(#[from] ron::Error),
    #[error("Map initial sector {initial_sector} is out of bounds for {sector_count} sectors")]
    InvalidInitialSector {
        initial_sector: usize,
        sector_count: usize,
    },
    #[error("Sector {sector_index} must have at least one vertex")]
    EmptySector { sector_index: usize },
    #[error("Initial position ({x}, {y}) must be inside initial sector {initial_sector}")]
    InitialPositionOutsideInitialSector {
        initial_sector: usize,
        x: f32,
        y: f32,
    },
    #[error(
        "Sector {sector_index} has {vertex_count} vertices but {wall_count} walls; counts must match"
    )]
    MismatchedWallCount {
        sector_index: usize,
        vertex_count: usize,
        wall_count: usize,
    },
    #[error(
        "Sector {sector_index} wall {wall_index} references portal sector {target_sector}, but only {sector_count} sectors exist"
    )]
    InvalidPortalTarget {
        sector_index: usize,
        wall_index: usize,
        target_sector: usize,
        sector_count: usize,
    },
    #[error("Sectors must have contiguous ids starting at 0; expected {expected}, found {found}")]
    NonContiguousSectorIds { expected: u32, found: u32 },
}

impl AssetLoader for SectorMapLoader {
    type Asset = SectorMap;
    type Settings = ();
    type Error = SectorMapError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let map = ron::de::from_bytes::<SectorMap>(&bytes)?;
        validate_map(&map)?;
        Ok(map)
    }

    fn extensions(&self) -> &[&str] {
        &["map.ron", "sector.ron"]
    }
}

pub fn load_map_from_path(path: impl AsRef<Path>) -> Result<SectorMap, SectorMapError> {
    let bytes = fs::read(path)?;
    let map = ron::de::from_bytes::<SectorMap>(&bytes)?;
    validate_map(&map)?;
    Ok(map)
}

pub fn save_map_to_path(map: &SectorMap, path: impl AsRef<Path>) -> Result<(), SectorMapError> {
    validate_map(map)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let pretty = ron::ser::PrettyConfig::default()
        .indentor("  ".to_owned())
        .new_line("\n".to_owned());
    let ron = ron::ser::to_string_pretty(map, pretty)?;
    fs::write(path, ron)?;
    Ok(())
}

pub fn map_to_sectors(map: &SectorMap) -> Result<(InitialSector, Vec<Sector>), SectorMapError> {
    validate_map(map)?;

    let sectors = map
        .sectors
        .iter()
        .enumerate()
        .map(|(sector_index, sector)| Sector {
            id: SectorId(sector_index as u32),
            vertices: sector
                .vertices
                .iter()
                .map(|vertex| Position2(vec2(vertex.0, vertex.1)))
                .collect(),
            portal_sectors: sector
                .walls
                .iter()
                .map(|wall| wall.portal.map(|portal| SectorId(portal as u32)))
                .collect(),
            colors: sector
                .walls
                .iter()
                .map(|wall| RawColor(wall.color))
                .collect(),
            floor: Length(sector.floor),
            ceil: Length(sector.ceil),
        })
        .collect();

    Ok((InitialSector(SectorId(map.initial_sector as u32)), sectors))
}

pub fn sectors_to_map(
    initial_sector: SectorId,
    sectors: &[Sector],
) -> Result<SectorMap, SectorMapError> {
    let initial_position = sector_spawn_position(initial_sector, sectors);
    sectors_to_map_with_spawn(initial_sector, initial_position, 0.0, sectors)
}

pub fn sectors_to_map_with_spawn(
    initial_sector: SectorId,
    initial_position: MapVertex,
    initial_direction_degrees: f32,
    sectors: &[Sector],
) -> Result<SectorMap, SectorMapError> {
    let mut ordered = sectors.to_vec();
    ordered.sort_by_key(|sector| sector.id.0);

    for (expected, sector) in ordered.iter().enumerate() {
        let expected = expected as u32;
        if sector.id.0 != expected {
            return Err(SectorMapError::NonContiguousSectorIds {
                expected,
                found: sector.id.0,
            });
        }
    }

    let map = SectorMap {
        initial_sector: initial_sector.0 as usize,
        initial_position,
        initial_direction_degrees,
        sectors: ordered
            .into_iter()
            .map(|sector| MapSector {
                floor: sector.floor.0,
                ceil: sector.ceil.0,
                vertices: sector
                    .vertices
                    .into_iter()
                    .map(|vertex| MapVertex(vertex.0.x, vertex.0.y))
                    .collect(),
                walls: sector
                    .portal_sectors
                    .into_iter()
                    .zip(sector.colors)
                    .map(|(portal, color)| MapWall {
                        color: color.0,
                        portal: portal.map(|sector| sector.0 as usize),
                    })
                    .collect(),
            })
            .collect(),
    };

    validate_map(&map)?;
    Ok(map)
}

pub fn validate_map(map: &SectorMap) -> Result<(), SectorMapError> {
    if map.initial_sector >= map.sectors.len() {
        return Err(SectorMapError::InvalidInitialSector {
            initial_sector: map.initial_sector,
            sector_count: map.sectors.len(),
        });
    }

    if !sector_contains_horizontal_point(
        &map.sectors[map.initial_sector],
        Vec2::new(map.initial_position.0, map.initial_position.1),
    ) {
        return Err(SectorMapError::InitialPositionOutsideInitialSector {
            initial_sector: map.initial_sector,
            x: map.initial_position.0,
            y: map.initial_position.1,
        });
    }

    for (sector_index, sector) in map.sectors.iter().enumerate() {
        if sector.vertices.is_empty() {
            return Err(SectorMapError::EmptySector { sector_index });
        }

        if sector.vertices.len() != sector.walls.len() {
            return Err(SectorMapError::MismatchedWallCount {
                sector_index,
                vertex_count: sector.vertices.len(),
                wall_count: sector.walls.len(),
            });
        }

        for (wall_index, wall) in sector.walls.iter().enumerate() {
            if let Some(target_sector) = wall.portal {
                if target_sector >= map.sectors.len() {
                    return Err(SectorMapError::InvalidPortalTarget {
                        sector_index,
                        wall_index,
                        target_sector,
                        sector_count: map.sectors.len(),
                    });
                }
            }
        }
    }

    Ok(())
}

fn sector_spawn_position(initial_sector: SectorId, sectors: &[Sector]) -> MapVertex {
    sectors
        .iter()
        .find(|sector| sector.id == initial_sector)
        .map(|sector| {
            let sum = sector
                .vertices
                .iter()
                .fold(Vec2::ZERO, |acc, vertex| acc + vertex.0);
            let center = sum / sector.vertices.len() as f32;
            MapVertex(center.x, center.y)
        })
        .unwrap_or_default()
}

fn sector_contains_horizontal_point(sector: &MapSector, point: Vec2) -> bool {
    for index in 0..sector.vertices.len() {
        let current = Vec2::new(sector.vertices[index].0, sector.vertices[index].1);
        let next = Vec2::new(
            sector.vertices[(index + 1) % sector.vertices.len()].0,
            sector.vertices[(index + 1) % sector.vertices.len()].1,
        );
        if point_on_segment(point, current, next) {
            return true;
        }
    }

    let mut inside = false;
    for index in 0..sector.vertices.len() {
        let current = Vec2::new(sector.vertices[index].0, sector.vertices[index].1);
        let next = Vec2::new(
            sector.vertices[(index + 1) % sector.vertices.len()].0,
            sector.vertices[(index + 1) % sector.vertices.len()].1,
        );
        let crosses_scanline = (current.y > point.y) != (next.y > point.y);
        if !crosses_scanline {
            continue;
        }

        let intersect_x =
            ((next.x - current.x) * (point.y - current.y) / (next.y - current.y)) + current.x;
        if point.x <= intersect_x + f32::EPSILON {
            inside = !inside;
        }
    }

    inside
}

fn point_on_segment(point: Vec2, left: Vec2, right: Vec2) -> bool {
    let segment = right - left;
    let from_left = point - left;
    let cross = segment.perp_dot(from_left).abs();
    if cross > f32::EPSILON {
        return false;
    }

    let dot = from_left.dot(segment);
    dot >= -f32::EPSILON && dot <= segment.length_squared() + f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map() -> SectorMap {
        SectorMap {
            initial_sector: 0,
            initial_position: MapVertex(5.7, 4.0),
            initial_direction_degrees: -20.0,
            sectors: vec![
                MapSector {
                    floor: 0.0,
                    ceil: 4.0,
                    vertices: vec![
                        MapVertex(2.0, 10.0),
                        MapVertex(4.0, 10.0),
                        MapVertex(11.0, -8.0),
                    ],
                    walls: vec![
                        MapWall {
                            color: [0, 0, 255],
                            portal: None,
                        },
                        MapWall {
                            color: [0, 128, 0],
                            portal: Some(1),
                        },
                        MapWall {
                            color: [255, 0, 0],
                            portal: None,
                        },
                    ],
                },
                MapSector {
                    floor: 0.25,
                    ceil: 3.75,
                    vertices: vec![
                        MapVertex(4.0, 10.0),
                        MapVertex(6.0, 10.0),
                        MapVertex(6.0, 14.0),
                    ],
                    walls: vec![
                        MapWall {
                            color: [255, 255, 0],
                            portal: Some(0),
                        },
                        MapWall {
                            color: [255, 0, 255],
                            portal: None,
                        },
                        MapWall {
                            color: [0, 128, 0],
                            portal: None,
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn converts_map_round_trip() {
        let map = sample_map();
        let (initial_sector, sectors) = map_to_sectors(&map).unwrap();
        let round_tripped = sectors_to_map_with_spawn(
            initial_sector.0,
            map.initial_position,
            map.initial_direction_degrees,
            &sectors,
        )
        .unwrap();
        assert_eq!(round_tripped.initial_sector, map.initial_sector);
        assert_eq!(round_tripped.initial_position, map.initial_position);
        assert_eq!(
            round_tripped.initial_direction_degrees,
            map.initial_direction_degrees
        );
        assert_eq!(round_tripped.sectors.len(), map.sectors.len());
        assert_eq!(round_tripped.sectors[0].walls[1].portal, Some(1));
    }

    #[test]
    fn rejects_mismatched_wall_counts() {
        let mut map = sample_map();
        map.sectors[0].walls.pop();
        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::MismatchedWallCount { .. })
        ));
    }

    #[test]
    fn parses_default_map_asset() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();
        validate_map(&map).unwrap();
    }

    #[test]
    fn default_map_has_walkable_steps_and_composite_rooms() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        assert!(map.sectors.len() >= 7);

        let has_walkable_step = map.sectors.iter().any(|sector| {
            sector
                .walls
                .iter()
                .filter_map(|wall| wall.portal)
                .any(|portal_index| {
                    let target_sector = &map.sectors[portal_index];
                    let floor_delta = target_sector.floor - sector.floor;
                    floor_delta > 0.0 && floor_delta <= 0.45
                })
        });
        assert!(has_walkable_step);

        let has_same_height_portal_pair =
            map.sectors
                .iter()
                .enumerate()
                .any(|(sector_index, sector)| {
                    sector
                        .walls
                        .iter()
                        .filter_map(|wall| wall.portal)
                        .any(|portal_index| {
                            portal_index != sector_index
                                && (map.sectors[portal_index].floor - sector.floor).abs()
                                    < f32::EPSILON
                                && (map.sectors[portal_index].ceil - sector.ceil).abs()
                                    < f32::EPSILON
                        })
                });
        assert!(has_same_height_portal_pair);
    }

    #[test]
    fn default_map_has_angled_walls_and_explicit_spawn() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        assert_ne!(map.initial_position, MapVertex::default());
        assert!(map.initial_direction_degrees.abs() > f32::EPSILON);

        let has_angled_wall = map.sectors.iter().any(|sector| {
            sector.vertices.iter().enumerate().any(|(index, vertex)| {
                let next = sector.vertices[(index + 1) % sector.vertices.len()];
                let dx = next.0 - vertex.0;
                let dy = next.1 - vertex.1;
                dx.abs() > f32::EPSILON && dy.abs() > f32::EPSILON
            })
        });
        assert!(has_angled_wall);
    }

    #[test]
    fn rejects_initial_position_outside_initial_sector() {
        let mut map = sample_map();
        map.initial_position = MapVertex(-100.0, -100.0);
        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::InitialPositionOutsideInitialSector { .. })
        ));
    }
}
