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
    pub colors: Vec<RawColor>,
    pub floor: Length,
    pub ceil: Length,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WallSegment {
    pub left: Position2,
    pub right: Position2,
    pub portal_sector: Option<SectorId>,
    pub color: RawColor,
}

impl Sector {
    pub fn wall_segments(&self) -> Vec<WallSegment> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut portal_sector_iter = self.portal_sectors.iter();
        let mut color_iter = self.colors.iter();

        let Some(&initial) = vertex_iter.next() else {
            return walls;
        };

        let mut add_wall = |left: Position2, right: Position2| {
            walls.push(WallSegment {
                left,
                right,
                portal_sector: *portal_sector_iter.next().unwrap_or(&None),
                color: *color_iter.next().unwrap_or(&MISSING_WALL_COLOR),
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
            colors: vec![
                RawColor([1, 2, 3]),
                RawColor([4, 5, 6]),
                RawColor([7, 8, 9]),
            ],
            floor: Length(0.0),
            ceil: Length(1.0),
        };

        let walls = sector.wall_segments();

        assert_eq!(walls.len(), 3);
        assert_eq!(walls[2].left, Position2(vec2(1.0, 1.0)));
        assert_eq!(walls[2].right, Position2(vec2(0.0, 0.0)));
        assert_eq!(walls[2].color, RawColor([7, 8, 9]));
    }
}
