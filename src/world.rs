use crate::{Length, Position2, RawColor, MISSING_WALL_COLOR};

use bevy::prelude::*;

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component)]
pub struct InitialSector(pub SectorId);

#[derive(Reflect, Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SectorId(pub u32);

#[derive(Component, Reflect, Debug, Default, Clone, PartialEq)]
#[reflect(Component)]
pub struct Sector {
    pub id: SectorId,
    pub vertices: Vec<Position2>,
    pub portal_sectors: Vec<Option<SectorId>>,
    pub portal_walkable: Vec<bool>,
    pub colors: Vec<RawColor>,
    pub portal_upper_colors: Vec<Option<RawColor>>,
    pub portal_lower_colors: Vec<Option<RawColor>>,
    pub floor: Length,
    pub ceil: Length,
    pub floor_color: RawColor,
    pub ceil_color: RawColor,
    pub no_ceiling: bool,
    pub sky_color: Option<RawColor>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WallSegment {
    pub left: Position2,
    pub right: Position2,
    pub portal_sector: Option<SectorId>,
    pub portal_walkable: bool,
    pub color: RawColor,
    pub portal_upper_color: Option<RawColor>,
    pub portal_lower_color: Option<RawColor>,
}

#[derive(Debug, Clone)]
pub struct WallSegments<'a> {
    sector: &'a Sector,
    index: usize,
}

impl<'a> Iterator for WallSegments<'a> {
    type Item = WallSegment;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.sector.vertices.len() {
            return None;
        }

        let index = self.index;
        self.index += 1;
        let next_index = (index + 1) % self.sector.vertices.len();

        Some(WallSegment {
            left: self.sector.vertices[index],
            right: self.sector.vertices[next_index],
            portal_sector: self
                .sector
                .portal_sectors
                .get(index)
                .copied()
                .unwrap_or(None),
            portal_walkable: self
                .sector
                .portal_walkable
                .get(index)
                .copied()
                .unwrap_or(true),
            color: self
                .sector
                .colors
                .get(index)
                .copied()
                .unwrap_or(*MISSING_WALL_COLOR),
            portal_upper_color: self
                .sector
                .portal_upper_colors
                .get(index)
                .copied()
                .unwrap_or(None),
            portal_lower_color: self
                .sector
                .portal_lower_colors
                .get(index)
                .copied()
                .unwrap_or(None),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.sector.vertices.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for WallSegments<'_> {}

impl Sector {
    pub fn wall_segments_iter(&self) -> WallSegments<'_> {
        WallSegments {
            sector: self,
            index: 0,
        }
    }

    pub fn wall_segments(&self) -> Vec<WallSegment> {
        self.wall_segments_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CEILING_COLOR, FLOOR_COLOR};
    use bevy::math::vec2;

    #[test]
    fn wall_segments_wrap_last_vertex_back_to_start() {
        let sector = Sector {
            id: SectorId(0),
            vertices: vec![
                Position2(vec2(0.0, 0.0)),
                Position2(vec2(1.0, 0.0)),
                Position2(vec2(1.0, 1.0)),
            ],
            portal_sectors: vec![None, None, None],
            portal_walkable: vec![true, true, true],
            colors: vec![
                RawColor([1, 2, 3]),
                RawColor([4, 5, 6]),
                RawColor([7, 8, 9]),
            ],
            portal_upper_colors: vec![None, Some(RawColor([10, 11, 12])), None],
            portal_lower_colors: vec![None, Some(RawColor([13, 14, 15])), None],
            floor: Length(0.0),
            ceil: Length(1.0),
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            no_ceiling: false,
            sky_color: None,
        };

        let walls = sector.wall_segments();

        assert_eq!(walls.len(), 3);
        assert_eq!(walls[1].portal_upper_color, Some(RawColor([10, 11, 12])));
        assert_eq!(walls[1].portal_lower_color, Some(RawColor([13, 14, 15])));
        assert!(walls[1].portal_walkable);
        assert_eq!(walls[2].left, Position2(vec2(1.0, 1.0)));
        assert_eq!(walls[2].right, Position2(vec2(0.0, 0.0)));
        assert_eq!(walls[2].color, RawColor([7, 8, 9]));
        assert_eq!(walls[2].portal_upper_color, None);
        assert_eq!(walls[2].portal_lower_color, None);
    }

    #[test]
    fn wall_segment_iterator_matches_allocated_segments() {
        let sector = Sector {
            id: SectorId(4),
            vertices: vec![
                Position2(vec2(-1.0, 0.0)),
                Position2(vec2(2.0, 0.0)),
                Position2(vec2(1.0, 3.0)),
                Position2(vec2(-2.0, 2.0)),
            ],
            portal_sectors: vec![Some(SectorId(5)), None, Some(SectorId(8)), None],
            portal_walkable: vec![true, false, true, false],
            colors: vec![
                RawColor([1, 2, 3]),
                RawColor([4, 5, 6]),
                RawColor([7, 8, 9]),
                RawColor([10, 11, 12]),
            ],
            portal_upper_colors: vec![None, Some(RawColor([13, 14, 15])), None, None],
            portal_lower_colors: vec![Some(RawColor([16, 17, 18])), None, None, None],
            floor: Length(0.0),
            ceil: Length(2.0),
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            no_ceiling: false,
            sky_color: None,
        };

        assert_eq!(
            sector.wall_segments_iter().collect::<Vec<_>>(),
            sector.wall_segments(),
        );
    }
}
