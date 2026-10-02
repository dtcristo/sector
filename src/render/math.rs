use super::{Normalized, RenderMetrics, NEAR};
use crate::{Length, Position2};

#[cfg(test)]
use bevy::math::vec2;
use bevy::math::vec3;

#[cfg(test)]
pub(crate) fn clip_wall(
    view_left: Position2,
    view_right: Position2,
) -> Option<(Position2, Position2)> {
    clip_wall_with_metrics(&RenderMetrics::base(), view_left, view_right)
}

pub(crate) fn clip_wall_with_metrics(
    metrics: &RenderMetrics,
    view_left: Position2,
    view_right: Position2,
) -> Option<(Position2, Position2)> {
    let left = view_left.0;
    let right = view_right.0;
    let tan_half_fov = metrics.back_clip_1.x / NEAR;
    let mut start = 0.0_f32;
    let mut end = 1.0_f32;

    // Clip the segment parameter against three half-planes. Computing a line
    // intersection and then checking its XY bounds loses vertical/horizontal
    // walls to rounding, allowing endpoints behind the camera to be projected.
    for (left_distance, right_distance) in [
        (left.y - NEAR, right.y - NEAR),
        (
            left.x + left.y * tan_half_fov,
            right.x + right.y * tan_half_fov,
        ),
        (
            left.y * tan_half_fov - left.x,
            right.y * tan_half_fov - right.x,
        ),
    ] {
        if left_distance < 0.0 && right_distance < 0.0 {
            return None;
        }
        if left_distance < 0.0 {
            start = start.max(left_distance / (left_distance - right_distance));
        } else if right_distance < 0.0 {
            end = end.min(left_distance / (left_distance - right_distance));
        }
        if start >= end {
            return None;
        }
    }

    let mut clipped_left = left.lerp(right, start);
    let mut clipped_right = left.lerp(right, end);
    clipped_left.y = clipped_left.y.max(NEAR);
    clipped_right.y = clipped_right.y.max(NEAR);
    Some((Position2(clipped_left), Position2(clipped_right)))
}

#[cfg(test)]
pub(crate) fn project(position: Position2, height: Length) -> Normalized {
    project_with_metrics(&RenderMetrics::base(), position, height)
}

pub(crate) fn project_with_metrics(
    metrics: &RenderMetrics,
    position: Position2,
    height: Length,
) -> Normalized {
    Normalized(metrics.perspective_matrix.project_point3(vec3(
        position.0.x,
        height.0,
        -position.0.y,
    )))
}

pub(crate) fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start * (1.0 - t) + end * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_wall_removes_segment_behind_camera() {
        assert!(clip_wall(Position2(vec2(-1.0, -1.0)), Position2(vec2(1.0, -2.0))).is_none());
    }

    #[test]
    fn clip_wall_trims_segment_crossing_near_plane() {
        let clipped = clip_wall(Position2(vec2(-0.05, 0.05)), Position2(vec2(0.05, 1.0))).unwrap();
        assert!(clipped.0 .0.y >= NEAR);
        assert!(clipped.1 .0.y >= NEAR);
    }

    #[test]
    fn clip_wall_trims_vertical_e1m1_edge_crossing_camera_plane() {
        let left = Position2(vec2(-3.1500015, 4.0499954));
        let right = Position2(vec2(-3.1500015, -0.15000153));
        for (left, right) in [(left, right), (right, left)] {
            let (left, right) = clip_wall(left, right).unwrap();
            for endpoint in [left.0, right.0] {
                assert!(endpoint.y >= NEAR);
                assert!(endpoint.x.abs() <= endpoint.y + 0.00001);
            }
        }
    }
}
