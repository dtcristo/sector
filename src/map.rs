use crate::{
    player::{PLAYER_HEIGHT_METERS, PLAYER_RADIUS_METERS},
    InitialSector, Length, Position2, RawColor, Sector, SectorId,
};

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

const MAP_EPSILON: f32 = 0.0001;

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
    #[error("Sector {sector_index} must have at least three vertices")]
    TooFewVertices { sector_index: usize },
    #[error("Sector {sector_index} floor {floor} must be below ceil {ceil}")]
    InvalidHeights {
        sector_index: usize,
        floor: f32,
        ceil: f32,
    },
    #[error("Initial position ({x}, {y}) must be inside initial sector {initial_sector}")]
    InitialPositionOutsideInitialSector {
        initial_sector: usize,
        x: f32,
        y: f32,
    },
    #[error(
        "Initial position ({x}, {y}) does not leave enough wall clearance for the player radius in sector {initial_sector}"
    )]
    InitialPositionLacksClearance {
        initial_sector: usize,
        x: f32,
        y: f32,
    },
    #[error(
        "Initial sector {initial_sector} only has {headroom}m of headroom; at least {required_headroom}m is required"
    )]
    InitialSectorInsufficientHeadroom {
        initial_sector: usize,
        headroom: f32,
        required_headroom: f32,
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
    #[error("Sector {sector_index} must wind clockwise for stable rendering")]
    SectorNotClockwise { sector_index: usize },
    #[error("Sector {sector_index} must be convex")]
    SectorNotConvex { sector_index: usize },
    #[error("Sector {sector_index} wall {wall_index} has zero length")]
    ZeroLengthWall {
        sector_index: usize,
        wall_index: usize,
    },
    #[error(
        "Sector {sector_index} wall {wall_index} portal to sector {target_sector} must match a reversed wall back to sector {sector_index}"
    )]
    NonReciprocalPortal {
        sector_index: usize,
        wall_index: usize,
        target_sector: usize,
    },
    #[error(
        "Sector {sector_index} wall {wall_index} portal to sector {target_sector} has no overlapping vertical opening"
    )]
    PortalHasNoOpening {
        sector_index: usize,
        wall_index: usize,
        target_sector: usize,
    },
    #[error(
        "Sector {sector_index} wall {wall_index} shares a zero-thickness solid boundary with sector {other_sector}; add a small gap or make it a portal"
    )]
    ZeroThicknessSolidWall {
        sector_index: usize,
        wall_index: usize,
        other_sector: usize,
    },
    #[error(
        "Sector {sector_a} overlaps sector {sector_b} in plan view while their vertical ranges also overlap"
    )]
    OverlappingSectors { sector_a: usize, sector_b: usize },
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

    let initial_sector = &map.sectors[map.initial_sector];
    let initial_headroom = initial_sector.ceil - initial_sector.floor;
    if initial_headroom + MAP_EPSILON < PLAYER_HEIGHT_METERS {
        return Err(SectorMapError::InitialSectorInsufficientHeadroom {
            initial_sector: map.initial_sector,
            headroom: initial_headroom,
            required_headroom: PLAYER_HEIGHT_METERS,
        });
    }

    let spawn = Vec2::new(map.initial_position.0, map.initial_position.1);
    if minimum_wall_distance(initial_sector, spawn) + MAP_EPSILON < PLAYER_RADIUS_METERS {
        return Err(SectorMapError::InitialPositionLacksClearance {
            initial_sector: map.initial_sector,
            x: map.initial_position.0,
            y: map.initial_position.1,
        });
    }

    for (sector_index, sector) in map.sectors.iter().enumerate() {
        if sector.vertices.is_empty() {
            return Err(SectorMapError::EmptySector { sector_index });
        }
        if sector.vertices.len() < 3 {
            return Err(SectorMapError::TooFewVertices { sector_index });
        }
        if sector.floor + MAP_EPSILON >= sector.ceil {
            return Err(SectorMapError::InvalidHeights {
                sector_index,
                floor: sector.floor,
                ceil: sector.ceil,
            });
        }

        if sector.vertices.len() != sector.walls.len() {
            return Err(SectorMapError::MismatchedWallCount {
                sector_index,
                vertex_count: sector.vertices.len(),
                wall_count: sector.walls.len(),
            });
        }

        for wall_index in 0..sector.vertices.len() {
            let start = map_vertex_vec2(sector.vertices[wall_index]);
            let end = map_vertex_vec2(sector.vertices[(wall_index + 1) % sector.vertices.len()]);
            if start.distance_squared(end) <= MAP_EPSILON * MAP_EPSILON {
                return Err(SectorMapError::ZeroLengthWall {
                    sector_index,
                    wall_index,
                });
            }
        }

        if polygon_signed_area(&sector.vertices) >= -MAP_EPSILON {
            return Err(SectorMapError::SectorNotClockwise { sector_index });
        }
        if !sector_is_convex(sector) {
            return Err(SectorMapError::SectorNotConvex { sector_index });
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

                let target = &map.sectors[target_sector];
                if !has_matching_reverse_portal(map, sector_index, wall_index, target_sector) {
                    return Err(SectorMapError::NonReciprocalPortal {
                        sector_index,
                        wall_index,
                        target_sector,
                    });
                }

                let overlap_floor = sector.floor.max(target.floor);
                let overlap_ceil = sector.ceil.min(target.ceil);
                if overlap_floor + MAP_EPSILON >= overlap_ceil {
                    return Err(SectorMapError::PortalHasNoOpening {
                        sector_index,
                        wall_index,
                        target_sector,
                    });
                }
            } else if let Some(other_sector) = find_shared_solid_wall(map, sector_index, wall_index)
            {
                return Err(SectorMapError::ZeroThicknessSolidWall {
                    sector_index,
                    wall_index,
                    other_sector,
                });
            }
        }
    }

    for sector_a in 0..map.sectors.len() {
        for sector_b in (sector_a + 1)..map.sectors.len() {
            if sectors_overlap_in_2d(&map.sectors[sector_a], &map.sectors[sector_b])
                && vertical_ranges_overlap(&map.sectors[sector_a], &map.sectors[sector_b])
            {
                return Err(SectorMapError::OverlappingSectors { sector_a, sector_b });
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

fn map_vertex_vec2(vertex: MapVertex) -> Vec2 {
    Vec2::new(vertex.0, vertex.1)
}

fn polygon_signed_area(vertices: &[MapVertex]) -> f32 {
    let mut area = 0.0;
    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        area += current.0 * next.1 - next.0 * current.1;
    }
    area * 0.5
}

fn sector_is_convex(sector: &MapSector) -> bool {
    let mut turn_sign: f32 = 0.0;

    for index in 0..sector.vertices.len() {
        let previous = map_vertex_vec2(
            sector.vertices[(index + sector.vertices.len() - 1) % sector.vertices.len()],
        );
        let current = map_vertex_vec2(sector.vertices[index]);
        let next = map_vertex_vec2(sector.vertices[(index + 1) % sector.vertices.len()]);
        let cross = (current - previous).perp_dot(next - current);

        if cross.abs() <= MAP_EPSILON {
            continue;
        }
        if turn_sign.abs() <= MAP_EPSILON {
            turn_sign = cross;
            continue;
        }
        if cross.signum() != turn_sign.signum() {
            return false;
        }
    }

    turn_sign < 0.0
}

fn minimum_wall_distance(sector: &MapSector, point: Vec2) -> f32 {
    (0..sector.vertices.len())
        .map(|index| {
            let start = map_vertex_vec2(sector.vertices[index]);
            let end = map_vertex_vec2(sector.vertices[(index + 1) % sector.vertices.len()]);
            distance_to_segment(point, start, end)
        })
        .fold(f32::INFINITY, f32::min)
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= MAP_EPSILON * MAP_EPSILON {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

fn has_matching_reverse_portal(
    map: &SectorMap,
    sector_index: usize,
    wall_index: usize,
    target_sector_index: usize,
) -> bool {
    let sector = &map.sectors[sector_index];
    let start = sector.vertices[wall_index];
    let end = sector.vertices[(wall_index + 1) % sector.vertices.len()];
    let target_sector = &map.sectors[target_sector_index];

    target_sector
        .walls
        .iter()
        .enumerate()
        .any(|(target_wall_index, target_wall)| {
            target_wall.portal == Some(sector_index)
                && target_sector.vertices[target_wall_index] == end
                && target_sector.vertices[(target_wall_index + 1) % target_sector.vertices.len()]
                    == start
        })
}

fn find_shared_solid_wall(
    map: &SectorMap,
    sector_index: usize,
    wall_index: usize,
) -> Option<usize> {
    let sector = &map.sectors[sector_index];
    let start = sector.vertices[wall_index];
    let end = sector.vertices[(wall_index + 1) % sector.vertices.len()];

    map.sectors
        .iter()
        .enumerate()
        .find_map(|(other_index, other_sector)| {
            if other_index == sector_index {
                return None;
            }

            other_sector
                .walls
                .iter()
                .enumerate()
                .find(|(other_wall_index, other_wall)| {
                    other_wall.portal.is_none()
                        && other_sector.vertices[*other_wall_index] == end
                        && other_sector.vertices
                            [(*other_wall_index + 1) % other_sector.vertices.len()]
                            == start
                })
                .map(|_| other_index)
        })
}

fn sectors_overlap_in_2d(a: &MapSector, b: &MapSector) -> bool {
    let axes = polygon_axes(a)
        .into_iter()
        .chain(polygon_axes(b))
        .collect::<Vec<_>>();

    axes.into_iter().all(|axis| {
        let (a_min, a_max) = project_polygon(a, axis);
        let (b_min, b_max) = project_polygon(b, axis);
        let overlap = a_max.min(b_max) - a_min.max(b_min);
        overlap > MAP_EPSILON
    })
}

fn polygon_axes(sector: &MapSector) -> Vec<Vec2> {
    (0..sector.vertices.len())
        .filter_map(|index| {
            let start = map_vertex_vec2(sector.vertices[index]);
            let end = map_vertex_vec2(sector.vertices[(index + 1) % sector.vertices.len()]);
            let edge = end - start;
            let normal = Vec2::new(-edge.y, edge.x);
            (normal.length_squared() > MAP_EPSILON * MAP_EPSILON).then_some(normal.normalize())
        })
        .collect()
}

fn project_polygon(sector: &MapSector, axis: Vec2) -> (f32, f32) {
    sector
        .vertices
        .iter()
        .map(|vertex| map_vertex_vec2(*vertex).dot(axis))
        .fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min, max), projection| (min.min(projection), max.max(projection)),
        )
}

fn vertical_ranges_overlap(a: &MapSector, b: &MapSector) -> bool {
    a.ceil.min(b.ceil) - a.floor.max(b.floor) > MAP_EPSILON
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
    use crate::player::{PLAYER_CROUCH_HEIGHT_METERS, PLAYER_HEIGHT_METERS};

    fn sample_map() -> SectorMap {
        SectorMap {
            initial_sector: 0,
            initial_position: MapVertex(2.0, 2.0),
            initial_direction_degrees: -20.0,
            sectors: vec![
                MapSector {
                    floor: 0.0,
                    ceil: 4.0,
                    vertices: vec![
                        MapVertex(0.0, 4.0),
                        MapVertex(4.0, 4.0),
                        MapVertex(4.0, 0.0),
                        MapVertex(0.0, 0.0),
                    ],
                    walls: vec![
                        MapWall {
                            color: [0, 0, 255],
                            portal: Some(1),
                        },
                        MapWall {
                            color: [0, 128, 0],
                            portal: None,
                        },
                        MapWall {
                            color: [255, 0, 0],
                            portal: None,
                        },
                        MapWall {
                            color: [255, 0, 255],
                            portal: None,
                        },
                    ],
                },
                MapSector {
                    floor: 0.25,
                    ceil: 3.75,
                    vertices: vec![
                        MapVertex(0.0, 8.0),
                        MapVertex(4.0, 8.0),
                        MapVertex(4.0, 4.0),
                        MapVertex(0.0, 4.0),
                    ],
                    walls: vec![
                        MapWall {
                            color: [255, 255, 0],
                            portal: None,
                        },
                        MapWall {
                            color: [255, 0, 255],
                            portal: None,
                        },
                        MapWall {
                            color: [0, 128, 0],
                            portal: Some(0),
                        },
                        MapWall {
                            color: [0, 255, 255],
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
        assert_eq!(round_tripped.sectors[0].walls[0].portal, Some(1));
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
    fn rejects_counter_clockwise_sector() {
        let mut map = sample_map();
        map.sectors[0].vertices.reverse();
        map.sectors[0].walls.reverse();

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::SectorNotClockwise { .. })
        ));
    }

    #[test]
    fn rejects_non_convex_sector() {
        let mut map = sample_map();
        map.initial_position = MapVertex(1.0, 3.0);
        map.sectors[0].vertices = vec![
            MapVertex(0.0, 4.0),
            MapVertex(4.0, 4.0),
            MapVertex(4.0, 2.0),
            MapVertex(2.0, 2.0),
            MapVertex(2.0, 0.0),
            MapVertex(0.0, 0.0),
        ];
        map.sectors[0].walls = vec![
            MapWall {
                color: [0, 0, 255],
                portal: None,
            },
            MapWall {
                color: [0, 128, 0],
                portal: None,
            },
            MapWall {
                color: [255, 0, 0],
                portal: None,
            },
            MapWall {
                color: [255, 255, 0],
                portal: None,
            },
            MapWall {
                color: [255, 0, 255],
                portal: None,
            },
            MapWall {
                color: [0, 255, 255],
                portal: None,
            },
        ];

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::SectorNotConvex { .. })
        ));
    }

    #[test]
    fn rejects_initial_position_without_clearance() {
        let mut map = sample_map();
        map.initial_position = MapVertex(0.1, 0.1);

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::InitialPositionLacksClearance { .. })
        ));
    }

    #[test]
    fn rejects_nonreciprocal_portal() {
        let mut map = sample_map();
        map.sectors[1].walls[2].portal = None;

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::NonReciprocalPortal { .. })
        ));
    }

    #[test]
    fn rejects_zero_thickness_shared_solid_wall() {
        let mut map = sample_map();
        map.sectors[0].walls[0].portal = None;
        map.sectors[1].walls[2].portal = None;

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::ZeroThicknessSolidWall { .. })
        ));
    }

    #[test]
    fn rejects_overlapping_sectors_with_overlapping_heights() {
        let mut map = sample_map();
        map.sectors[0].walls[0].portal = None;
        map.sectors[1].floor = 1.0;
        map.sectors[1].ceil = 3.0;
        map.sectors[1].vertices = vec![
            MapVertex(1.0, 3.0),
            MapVertex(3.0, 3.0),
            MapVertex(3.0, 1.0),
            MapVertex(1.0, 1.0),
        ];
        map.sectors[1].walls = vec![
            MapWall {
                color: [255, 255, 0],
                portal: None,
            },
            MapWall {
                color: [255, 0, 255],
                portal: None,
            },
            MapWall {
                color: [0, 128, 0],
                portal: None,
            },
            MapWall {
                color: [0, 255, 255],
                portal: None,
            },
        ];

        assert!(matches!(
            validate_map(&map),
            Err(SectorMapError::OverlappingSectors { .. })
        ));
    }

    #[test]
    fn parses_default_map_asset() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();
        validate_map(&map).unwrap();
    }

    #[test]
    fn default_map_has_walkable_steps_and_many_rooms() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        assert!(map.sectors.len() >= 16);

        let walkable_step_count = map
            .sectors
            .iter()
            .flat_map(|sector| {
                sector.walls.iter().filter_map(|wall| {
                    let portal_index = wall.portal?;
                    let target_sector = &map.sectors[portal_index];
                    let floor_delta = target_sector.floor - sector.floor;
                    (floor_delta > 0.0 && floor_delta <= 0.45).then_some(())
                })
            })
            .count();
        assert!(walkable_step_count >= 4);
    }

    #[test]
    fn default_map_uses_dark_grey_portal_trims_for_stairs_and_windows() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        const STAIR_PORTAL_GREY: [u8; 3] = [56, 56, 56];

        for sector_index in [3_usize, 4, 5, 6] {
            let sector = &map.sectors[sector_index];
            assert!(sector
                .walls
                .iter()
                .filter(|wall| wall.portal.is_some())
                .all(|wall| wall.color == STAIR_PORTAL_GREY));
        }

        assert!(map.sectors[10]
            .walls
            .iter()
            .filter(|wall| wall.portal.is_some())
            .all(|wall| wall.color == STAIR_PORTAL_GREY));
    }

    #[test]
    fn default_map_spawn_faces_staircase() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();
        let initial_sector = &map.sectors[map.initial_sector];
        let stair_wall_index = initial_sector
            .walls
            .iter()
            .position(|wall| wall.portal == Some(1))
            .expect("default map should have a portal from the initial room to the staircase");
        let wall_start = initial_sector.vertices[stair_wall_index];
        let wall_end =
            initial_sector.vertices[(stair_wall_index + 1) % initial_sector.vertices.len()];
        let target_x = (wall_start.0 + wall_end.0) * 0.5;
        let target_y = (wall_start.1 + wall_end.1) * 0.5;
        let to_stair_x = target_x - map.initial_position.0;
        let to_stair_y = target_y - map.initial_position.1;
        let to_stair_len = (to_stair_x * to_stair_x + to_stair_y * to_stair_y).sqrt();
        let forward_x = -map.initial_direction_radians().sin();
        let forward_y = map.initial_direction_radians().cos();
        let alignment = (forward_x * to_stair_x + forward_y * to_stair_y) / to_stair_len;

        assert!(
            alignment > 0.99,
            "spawn should face staircase, alignment was {alignment}"
        );
    }

    #[test]
    fn default_map_has_high_window_sector_between_rooms() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        let has_high_window_sector = map.sectors.iter().any(|sector| {
            let portal_targets = sector
                .walls
                .iter()
                .filter_map(|wall| wall.portal)
                .collect::<Vec<_>>();

            portal_targets.len() >= 2
                && portal_targets.iter().all(|portal_index| {
                    let target_sector = &map.sectors[*portal_index];
                    sector.floor > target_sector.floor + 0.45
                        && sector.ceil < target_sector.ceil - 0.45
                })
        });

        assert!(has_high_window_sector);
    }

    #[test]
    fn default_map_has_crouch_only_connector() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        let has_crouch_connector = map.sectors.iter().any(|sector| {
            let headroom = sector.ceil - sector.floor;
            headroom >= PLAYER_CROUCH_HEIGHT_METERS
                && headroom < PLAYER_HEIGHT_METERS
                && sector.walls.iter().any(|wall| wall.portal.is_some())
        });

        assert!(has_crouch_connector);
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
    fn default_map_has_large_drop_and_only_bidirectional_portals() {
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../assets/maps/default.map.ron")).unwrap();

        let has_large_drop = map
            .sectors
            .iter()
            .enumerate()
            .any(|(sector_index, sector)| {
                sector
                    .walls
                    .iter()
                    .filter_map(|wall| wall.portal)
                    .any(|portal_index| {
                        portal_index != sector_index
                            && sector.floor - map.sectors[portal_index].floor > 0.45
                    })
            });
        assert!(has_large_drop);

        for (sector_index, sector) in map.sectors.iter().enumerate() {
            for (wall_index, wall) in sector.walls.iter().enumerate() {
                let Some(target_sector_index) = wall.portal else {
                    continue;
                };
                let start = sector.vertices[wall_index];
                let end = sector.vertices[(wall_index + 1) % sector.vertices.len()];
                let target_sector = &map.sectors[target_sector_index];

                let has_reverse_portal = target_sector.walls.iter().enumerate().any(
                    |(target_wall_index, target_wall)| {
                        if target_wall.portal != Some(sector_index) {
                            return false;
                        }

                        let target_start = target_sector.vertices[target_wall_index];
                        let target_end = target_sector.vertices
                            [(target_wall_index + 1) % target_sector.vertices.len()];

                        target_start == end && target_end == start
                    },
                );

                assert!(
                    has_reverse_portal,
                    "sector {sector_index} wall {wall_index} portal to sector {target_sector_index} must be bidirectional"
                );
            }
        }
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
