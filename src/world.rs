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
    pub no_ceiling: bool,
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

impl Sector {
    pub fn wall_segments(&self) -> Vec<WallSegment> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut portal_sector_iter = self.portal_sectors.iter();
        let mut portal_walkable_iter = self.portal_walkable.iter().copied();
        let mut color_iter = self.colors.iter();
        let mut portal_upper_color_iter = self.portal_upper_colors.iter();
        let mut portal_lower_color_iter = self.portal_lower_colors.iter();

        let Some(&initial) = vertex_iter.next() else {
            return walls;
        };

        let mut add_wall = |left: Position2, right: Position2| {
            walls.push(WallSegment {
                left,
                right,
                portal_sector: *portal_sector_iter.next().unwrap_or(&None),
                portal_walkable: portal_walkable_iter.next().unwrap_or(true),
                color: *color_iter.next().unwrap_or(&MISSING_WALL_COLOR),
                portal_upper_color: *portal_upper_color_iter.next().unwrap_or(&None),
                portal_lower_color: *portal_lower_color_iter.next().unwrap_or(&None),
            });
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

#[cfg(test)]
mod tests {
    use super::*;
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
            no_ceiling: false,
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
}
