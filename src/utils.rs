use crate::*;

pub fn clip_wall(
    mut view_left: Position2,
    mut view_right: Position2,
) -> Option<(Position2, Position2)> {
    // Skip entirely behind back
    if view_left.0.y < NEAR && view_right.0.y < NEAR {
        return None;
    }

    // Clip left side
    if let Some(intersection) = intersect(view_left.0, view_right.0, *LEFT_CLIP_1, *LEFT_CLIP_2) {
        if intersection.x < -*X_NEAR {
            if point_behind(view_left.0, *LEFT_CLIP_1, *LEFT_CLIP_2) {
                view_left = Position2(intersection);
            } else {
                view_right = Position2(intersection);
            }
        }
    }

    // Clip right side
    if let Some(intersection) = intersect(view_left.0, view_right.0, *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
        if intersection.x > *X_NEAR {
            if point_behind(view_left.0, *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
                view_left = Position2(intersection);
            } else {
                view_right = Position2(intersection);
            }
        }
    }

    // Clip behind back
    if view_left.0.y < NEAR || view_right.0.y < NEAR {
        if let Some(intersection) = intersect(view_left.0, view_right.0, *BACK_CLIP_1, *BACK_CLIP_2)
        {
            if point_behind(view_left.0, *BACK_CLIP_1, *BACK_CLIP_2) {
                view_left = Position2(intersection);
            } else {
                view_right = Position2(intersection);
            }
        }
    }

    // Skip entirely behind left side
    if point_behind(view_right.0, *LEFT_CLIP_1, *LEFT_CLIP_2) {
        return None;
    }

    // Skip entirely behind right side
    if point_behind(view_left.0, *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
        return None;
    }

    Some((view_left, view_right))
}

pub fn project(position: Position2, height: Length) -> Normalized {
    Normalized(PERSPECTIVE_MATRIX.project_point3(vec3(position.0.x, height.0, -position.0.y)))
}

pub fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start * (1.0 - t) + end * t
}

pub fn lerpi(start: isize, end: isize, t: f32) -> isize {
    (start as f32 * (1.0 - t) + end as f32 * t).round() as isize
}

pub fn intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let a_perp_dot = a1.perp_dot(a2);
    let b_perp_dot = b1.perp_dot(b2);

    let divisor = vec2(a1.x - a2.x, a1.y - a2.y).perp_dot(vec2(b1.x - b2.x, b1.y - b2.y));
    if divisor == 0.0 {
        return None;
    };

    let result = vec2(
        vec2(a_perp_dot, a1.x - a2.x).perp_dot(vec2(b_perp_dot, b1.x - b2.x)) / divisor,
        vec2(a_perp_dot, a1.y - a2.y).perp_dot(vec2(b_perp_dot, b1.y - b2.y)) / divisor,
    );

    if between(result.x, a1.x, a2.x) && between(result.y, a1.y, a2.y) {
        Some(result)
    } else {
        None
    }
}

pub fn between(test: f32, a: f32, b: f32) -> bool {
    test >= a.min(b) && test <= a.max(b)
}

pub fn point_behind(point: Vec2, a: Vec2, b: Vec2) -> bool {
    vec2(b.x - a.x, b.y - a.y).perp_dot(vec2(point.x - a.x, point.y - a.y)) > 0.0
}
