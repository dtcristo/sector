use super::{
    frame::{draw_line, draw_pixel},
    math::clip_wall,
    Minimap, RenderView, BACK_CLIP_1, BACK_CLIP_2, LEFT_CLIP_2, RIGHT_CLIP_1,
};
use crate::{Position2, Sector, FRUSTUM_COLOR, PLAYER_COLOR, WALL_CLIPPED_COLOR};

use bevy::{math::vec2, prelude::*};

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

    for sector in sectors {
        for wall in sector.wall_segments() {
            let color = wall.color;
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            let mut view_left_after_clip = view_left;
            let mut view_right_after_clip = view_right;

            let clipping = clip_wall(view_left, view_right);
            if let Some((l, r)) = clipping {
                view_left_after_clip = l;
                view_right_after_clip = r;
            }

            if let Some((left, right, left_after_clip, right_after_clip)) = match minimap {
                Minimap::Off => None,
                Minimap::FirstPerson => Some((
                    view_left.into(),
                    view_right.into(),
                    view_left_after_clip.into(),
                    view_right_after_clip.into(),
                )),
                Minimap::Absolute => {
                    let abs_left = wall.left;
                    let abs_right = wall.right;
                    let abs_left_after_clip = view_left_after_clip.transform(reverse_view_matrix);
                    let abs_right_after_clip = view_right_after_clip.transform(reverse_view_matrix);

                    Some((
                        abs_left.into(),
                        abs_right.into(),
                        abs_left_after_clip.into(),
                        abs_right_after_clip.into(),
                    ))
                }
            } {
                if clipping.is_none() {
                    draw_line(frame, left, right, *WALL_CLIPPED_COLOR);
                    continue;
                }
                if left_after_clip != left {
                    draw_line(frame, left, left_after_clip, *WALL_CLIPPED_COLOR);
                }
                if right_after_clip != right {
                    draw_line(frame, right_after_clip, right, *WALL_CLIPPED_COLOR);
                }
                draw_line(frame, left_after_clip, right_after_clip, color);
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
        draw_pixel(frame, player_pixel, *PLAYER_COLOR);
    }
}
