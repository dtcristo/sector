use super::{
    frame::{draw_line, draw_pixel, Pixel},
    math::clip_wall,
    Automap, RenderView, BACK_CLIP_1, BACK_CLIP_2, LEFT_CLIP_2, RIGHT_CLIP_1,
};
use crate::game::PLAYER_RADIUS_METERS;
use crate::{
    Position2, Sector, AUTOMAP_PORTAL_COLOR, AUTOMAP_WALL_COLOR, FRUSTUM_COLOR, PLAYER_COLOR,
};

use bevy::{math::vec2, prelude::*};
use std::collections::HashSet;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct PortalEdgeKey {
    start: (u32, u32),
    end: (u32, u32),
}

pub(crate) fn render_automap(
    frame: &mut [u8],
    view: &RenderView,
    sectors: &[&Sector],
    automap: Automap,
) {
    if automap == Automap::Off {
        return;
    }

    let view_matrix = Mat3::from_rotation_z(-view.direction)
        * Mat3::from_translation(-vec2(view.position.0.x, view.position.0.y));
    let inverse_view_rotation = Mat3::from_rotation_z(view.direction);
    let player_xy = view.position.truncate().0;
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
            let clipped = clip_wall(view_left, view_right);

            let Some((left, right)) = automap_segment(
                automap,
                wall.left,
                wall.right,
                view_left,
                view_right,
                clipped,
                player_xy,
                inverse_view_rotation,
            ) else {
                continue;
            };

            if wall.portal_sector.is_some() {
                draw_line(frame, left, right, *AUTOMAP_PORTAL_COLOR);
            } else {
                draw_line(frame, left, right, *AUTOMAP_WALL_COLOR);
            }
        }
    }

    let view_player = Position2(vec2(0.0, 0.0));
    let view_near_left = Position2(*BACK_CLIP_2);
    let view_near_right = Position2(*BACK_CLIP_1);
    let view_far_left = Position2(*LEFT_CLIP_2);
    let view_far_right = Position2(*RIGHT_CLIP_1);

    if let Some((player_pixel, near_left, near_right, far_left, far_right)) = automap_overlay(
        automap,
        view_player,
        view_near_left,
        view_near_right,
        view_far_left,
        view_far_right,
        inverse_view_rotation,
    ) {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_disc(
            frame,
            player_pixel,
            (super::AUTOMAP_SCALE * PLAYER_RADIUS_METERS)
                .round()
                .max(1.0) as isize,
            *PLAYER_COLOR,
        );
    }
}

fn automap_segment(
    automap: Automap,
    world_left: Position2,
    world_right: Position2,
    view_left: Position2,
    view_right: Position2,
    clipped: Option<(Position2, Position2)>,
    player_xy: Vec2,
    inverse_view_rotation: Mat3,
) -> Option<(Pixel, Pixel)> {
    match automap {
        Automap::Off => None,
        Automap::RotateFull => Some((view_left.into(), view_right.into())),
        Automap::RotateVisible => clipped.map(|(left, right)| (left.into(), right.into())),
        Automap::NorthUpFull => Some((
            Position2(world_left.0 - player_xy).into(),
            Position2(world_right.0 - player_xy).into(),
        )),
        Automap::NorthUpVisible => clipped.map(|(left, right)| {
            (
                left.transform(inverse_view_rotation).into(),
                right.transform(inverse_view_rotation).into(),
            )
        }),
    }
}

fn automap_overlay(
    automap: Automap,
    view_player: Position2,
    view_near_left: Position2,
    view_near_right: Position2,
    view_far_left: Position2,
    view_far_right: Position2,
    inverse_view_rotation: Mat3,
) -> Option<(Pixel, Pixel, Pixel, Pixel, Pixel)> {
    match automap {
        Automap::Off => None,
        Automap::RotateFull | Automap::RotateVisible => Some((
            view_player.into(),
            view_near_left.into(),
            view_near_right.into(),
            view_far_left.into(),
            view_far_right.into(),
        )),
        Automap::NorthUpFull | Automap::NorthUpVisible => {
            let player = Position2(vec2(0.0, 0.0));
            Some((
                player.into(),
                view_near_left.transform(inverse_view_rotation).into(),
                view_near_right.transform(inverse_view_rotation).into(),
                view_far_left.transform(inverse_view_rotation).into(),
                view_far_right.transform(inverse_view_rotation).into(),
            ))
        }
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
    fn north_up_automap_draws_solid_walls_in_uniform_grey() {
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

        render_automap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Automap::NorthUpFull,
        );

        let wall = [
            AUTOMAP_WALL_COLOR.0[0],
            AUTOMAP_WALL_COLOR.0[1],
            AUTOMAP_WALL_COLOR.0[2],
            255,
        ];
        let center_x = super::super::WIDTH as usize / 2;
        let front_y = (super::super::HEIGHT as usize / 2).saturating_sub(80);
        let back_y = super::super::HEIGHT as usize / 2 + 80;

        assert_eq!(frame.pixel(center_x, front_y), wall);
        assert_eq!(frame.pixel(center_x, back_y), wall);
    }

    #[test]
    fn north_up_visible_automap_omits_back_wall() {
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

        render_automap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Automap::NorthUpVisible,
        );

        let center_x = super::super::WIDTH as usize / 2;
        let front_y = (super::super::HEIGHT as usize / 2).saturating_sub(80);
        let back_y = super::super::HEIGHT as usize / 2 + 80;
        let wall = [
            AUTOMAP_WALL_COLOR.0[0],
            AUTOMAP_WALL_COLOR.0[1],
            AUTOMAP_WALL_COLOR.0[2],
            255,
        ];

        assert_eq!(frame.pixel(center_x, front_y), wall);
        assert_eq!(frame.pixel(center_x, back_y), [0, 0, 0, 255]);
    }

    #[test]
    fn north_up_automap_draws_portals_red_once() {
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

        render_automap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Automap::NorthUpFull,
        );

        assert_eq!(
            frame.pixel(
                super::super::WIDTH as usize / 2,
                super::super::HEIGHT as usize / 2 - 8,
            ),
            [
                AUTOMAP_PORTAL_COLOR.0[0],
                AUTOMAP_PORTAL_COLOR.0[1],
                AUTOMAP_PORTAL_COLOR.0[2],
                255,
            ]
        );
    }

    #[test]
    fn north_up_automap_keeps_player_centered_when_view_moves() {
        let sectors = [sector(
            0,
            &[(-6.0, 10.0), (6.0, 10.0), (6.0, -10.0), (-6.0, -10.0)],
            &[None, None, None, None],
        )];
        let view = RenderView::new(
            Position3(Vec3::new(3.0, -4.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.4,
            None,
        );
        let mut frame = super::super::FrameBuffer::new();

        render_automap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Automap::NorthUpFull,
        );

        let radius = (super::super::AUTOMAP_SCALE * PLAYER_RADIUS_METERS)
            .round()
            .max(1.0) as usize;
        let center_x = super::super::WIDTH as usize / 2;
        let center_y = super::super::HEIGHT as usize / 2;
        let player = [PLAYER_COLOR.0[0], PLAYER_COLOR.0[1], PLAYER_COLOR.0[2], 255];

        assert_eq!(frame.pixel(center_x, center_y), player);
        assert_eq!(frame.pixel(center_x + radius, center_y), player);
    }

    #[test]
    fn rotate_visible_automap_omits_back_wall() {
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

        render_automap(
            frame.as_mut_slice(),
            &view,
            &sectors.iter().collect::<Vec<_>>(),
            Automap::RotateVisible,
        );

        let center_x = super::super::WIDTH as usize / 2;
        let wall = [
            AUTOMAP_WALL_COLOR.0[0],
            AUTOMAP_WALL_COLOR.0[1],
            AUTOMAP_WALL_COLOR.0[2],
            255,
        ];

        assert_eq!(frame.pixel(center_x, 40), wall);
        assert_eq!(frame.pixel(center_x, 200), [0, 0, 0, 255]);
    }
}
