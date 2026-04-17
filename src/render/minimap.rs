use super::{
    frame::{draw_line, draw_pixel, Pixel},
    Minimap, RenderView, BACK_CLIP_1, BACK_CLIP_2, LEFT_CLIP_2, RIGHT_CLIP_1,
};
use crate::game::PLAYER_RADIUS_METERS;
use crate::{
    Position2, Sector, FRUSTUM_COLOR, MINIMAP_PORTAL_COLOR, MINIMAP_WALL_COLOR, PLAYER_COLOR,
};

use bevy::{math::vec2, prelude::*};
use std::collections::HashSet;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct PortalEdgeKey {
    start: (u32, u32),
    end: (u32, u32),
}

pub(crate) fn render_minimap(
    frame: &mut [u8],
    view: &RenderView,
    sectors: &[&Sector],
    minimap: Minimap,
) {
    if minimap == Minimap::Off {
        return;
    }

    let view_matrix = Mat3::from_rotation_z(-view.direction)
        * Mat3::from_translation(-vec2(view.position.0.x, view.position.0.y));
    let reverse_view_matrix = Mat3::from_translation(vec2(view.position.0.x, view.position.0.y))
        * Mat3::from_rotation_z(view.direction);
    let mut drawn_portals = HashSet::new();

    for sector in sectors {
        for wall in sector.wall_segments() {
            if wall.portal_sector.is_some()
                && !drawn_portals.insert(portal_edge_key(wall.left, wall.right))
            {
                continue;
            }

            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            if let Some((left, right)) = match minimap {
                Minimap::Off => None,
                Minimap::FirstPerson => Some((view_left.into(), view_right.into())),
                Minimap::Absolute => Some((wall.left.into(), wall.right.into())),
            } {
                if wall.portal_sector.is_some() {
                    draw_line(frame, left, right, *MINIMAP_PORTAL_COLOR);
                    continue;
                }

                draw_line(frame, left, right, *MINIMAP_WALL_COLOR);
            }
        }
    }

    let view_player = Position2(vec2(0.0, 0.0));
    let view_near_left = Position2(*BACK_CLIP_2);
    let view_near_right = Position2(*BACK_CLIP_1);
    let view_far_left = Position2(*LEFT_CLIP_2);
    let view_far_right = Position2(*RIGHT_CLIP_1);

    if let Some((player_pixel, near_left, near_right, far_left, far_right)) = match minimap {
        Minimap::Off => None,
        Minimap::FirstPerson => Some((
            view_player.into(),
            view_near_left.into(),
            view_near_right.into(),
            view_far_left.into(),
            view_far_right.into(),
        )),
        Minimap::Absolute => {
            let abs_player = view.position.truncate();
            let abs_near_left = view_near_left.transform(reverse_view_matrix);
            let abs_near_right = view_near_right.transform(reverse_view_matrix);
            let abs_far_left = view_far_left.transform(reverse_view_matrix);
            let abs_far_right = view_far_right.transform(reverse_view_matrix);

            Some((
                abs_player.into(),
                abs_near_left.into(),
                abs_near_right.into(),
                abs_far_left.into(),
                abs_far_right.into(),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_disc(
            frame,
            player_pixel,
            (super::MINIMAP_SCALE * PLAYER_RADIUS_METERS)
                .round()
                .max(1.0) as isize,
            *PLAYER_COLOR,
        );
    }
}

fn draw_disc(frame: &mut [u8], center: Pixel, radius: isize, color: crate::RawColor) {
    let radius_squared = radius * radius;
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y <= radius_squared {
                draw_pixel(frame, Pixel::new(center.x + x, center.y + y), color);
            }
        }
    }
}

fn portal_edge_key(left: Position2, right: Position2) -> PortalEdgeKey {
    let left = (left.0.x.to_bits(), left.0.y.to_bits());
    let right = (right.0.x.to_bits(), right.0.y.to_bits());
    let (start, end) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };

    PortalEdgeKey { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Length, Position3, RawColor, SectorId};

    fn sector(id: u32, vertices: &[(f32, f32)], portal_sectors: &[Option<u32>]) -> Sector {
        Sector {
            id: SectorId(id),
            vertices: vertices
                .iter()
                .map(|(x, y)| Position2(vec2(*x, *y)))
                .collect(),
            portal_sectors: portal_sectors
                .iter()
                .map(|portal| portal.map(SectorId))
                .collect(),
            colors: vec![RawColor([0, 0, 0]); vertices.len()],
            floor: Length(0.0),
            ceil: Length(4.0),
        }
    }

    #[test]
    fn portal_edge_keys_ignore_wall_direction() {
        let a = Position2(vec2(-1.0, 1.0));
        let b = Position2(vec2(1.0, 1.0));
        assert_eq!(portal_edge_key(a, b), portal_edge_key(b, a));
    }

    #[test]
    fn absolute_minimap_draws_solid_walls_in_uniform_grey() {
        let sectors = [sector(
            0,
            &[(-6.0, 10.0), (6.0, 10.0), (6.0, -10.0), (-6.0, -10.0)],
            &[None, None, None, None],
        )];
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let mut frame = super::super::FrameBuffer::new();

        render_minimap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Minimap::Absolute,
        );

        let wall = [
            MINIMAP_WALL_COLOR.0[0],
            MINIMAP_WALL_COLOR.0[1],
            MINIMAP_WALL_COLOR.0[2],
            255,
        ];
        let center_x = super::super::WIDTH as usize / 2;
        let front_y = (super::super::HEIGHT as usize / 2).saturating_sub(80);
        let back_y = super::super::HEIGHT as usize / 2 + 80;

        assert_eq!(frame.pixel(center_x, front_y), wall);
        assert_eq!(frame.pixel(center_x, back_y), wall);
    }

    #[test]
    fn absolute_minimap_draws_portals_red_once() {
        let sectors = [
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
            ),
        ];
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let mut frame = super::super::FrameBuffer::new();

        render_minimap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Minimap::Absolute,
        );

        assert_eq!(
            frame.pixel(
                super::super::WIDTH as usize / 2,
                super::super::HEIGHT as usize / 2 - 8,
            ),
            [
                MINIMAP_PORTAL_COLOR.0[0],
                MINIMAP_PORTAL_COLOR.0[1],
                MINIMAP_PORTAL_COLOR.0[2],
                255,
            ]
        );
    }

    #[test]
    fn absolute_minimap_draws_player_as_collision_radius_disc() {
        let sectors = [sector(
            0,
            &[(-6.0, 10.0), (6.0, 10.0), (6.0, -10.0), (-6.0, -10.0)],
            &[None, None, None, None],
        )];
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let mut frame = super::super::FrameBuffer::new();

        render_minimap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Minimap::Absolute,
        );

        let radius = (super::super::MINIMAP_SCALE * PLAYER_RADIUS_METERS)
            .round()
            .max(1.0) as usize;
        let center_x = super::super::WIDTH as usize / 2;
        let center_y = super::super::HEIGHT as usize / 2;
        let player = [PLAYER_COLOR.0[0], PLAYER_COLOR.0[1], PLAYER_COLOR.0[2], 255];

        assert_eq!(frame.pixel(center_x, center_y), player);
        assert_eq!(frame.pixel(center_x + radius, center_y), player);
    }
}
