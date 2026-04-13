use super::{
    frame::{draw_line, draw_vertical_line, Pixel},
    math::{clip_wall, lerp, lerpi, project},
    RenderView, BRIGHTNESS_FAR, BRIGHTNESS_NEAR, FAR, GAP, HEIGHT, NEAR, WIDTH,
};
use crate::{RawColor, Sector, CEILING_COLOR, FLOOR_COLOR};

use bevy::{math::vec2, prelude::*};
use palette::{Hsv, IntoColor, Srgb};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Copy, Clone)]
struct PortalSpan<'a> {
    sector: &'a Sector,
    x_min: isize,
    x_max: isize,
}

const FLUSH_HEIGHT_EPSILON: f32 = 0.001;
const OUTLINE_COLOR: RawColor = RawColor([0, 0, 0]);

pub(crate) fn render_world(frame: &mut [u8], view: &RenderView, sectors: &[&Sector]) {
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, *sector)).collect();
    let Some(current_sector) = view
        .current_sector
        .and_then(|id| sectors_by_id.get(&id).copied())
    else {
        return;
    };

    let view_matrix = Mat3::from_rotation_z(-view.direction)
        * Mat3::from_translation(-vec2(view.position.0.x, view.position.0.y));

    let mut portal_queue = VecDeque::<PortalSpan>::new();
    let mut y_min_vec = vec![GAP; WIDTH as usize];
    let mut y_max_vec = vec![HEIGHT as isize; WIDTH as usize];
    let ceiling_hsv: Hsv = Srgb::<u8>::from(*CEILING_COLOR).into_format().into_color();
    let floor_hsv: Hsv = Srgb::<u8>::from(*FLOOR_COLOR).into_format().into_color();

    portal_queue.push_back(PortalSpan {
        sector: current_sector,
        x_min: GAP,
        x_max: WIDTH as isize,
    });

    while let Some(self_portal) = portal_queue.pop_front() {
        let sector = self_portal.sector;
        let view_floor = crate::Length(sector.floor.0 - view.position.0.z);
        let view_ceil = crate::Length(sector.ceil.0 - view.position.0.z);

        'walls: for wall in sector.wall_segments() {
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            if let Some((view_left, view_right)) = clip_wall(view_left, view_right) {
                let norm_left_top = project(view_left, view_ceil);
                let norm_left_bottom = project(view_left, view_floor);
                let norm_right_top = project(view_right, view_ceil);
                let norm_right_bottom = project(view_right, view_floor);

                let left_top: Pixel = norm_left_top.into();
                let left_bottom: Pixel = norm_left_bottom.into();
                let right_top: Pixel = norm_right_top.into();
                let right_bottom: Pixel = norm_right_bottom.into();

                let dx = right_top.x - left_top.x;
                if dx <= 0 {
                    continue 'walls;
                }

                let x_left = left_top.x.clamp(self_portal.x_min, self_portal.x_max);
                let x_right = right_top.x.clamp(self_portal.x_min, self_portal.x_max);

                let portal_sector = wall
                    .portal_sector
                    .and_then(|id| sectors_by_id.get(&id).copied());
                let portal_ceiling_flush = portal_sector.is_some_and(|portal_sector| {
                    heights_match(portal_sector.ceil.0, sector.ceil.0)
                });
                let portal_floor_flush = portal_sector.is_some_and(|portal_sector| {
                    heights_match(portal_sector.floor.0, sector.floor.0)
                });

                let (y_portal_top, y_portal_bottom) = if let Some(portal_sector) = portal_sector {
                    portal_queue.push_back(PortalSpan {
                        sector: portal_sector,
                        x_min: x_left,
                        x_max: x_right,
                    });

                    let view_portal_ceil = crate::Length(portal_sector.ceil.0 - view.position.0.z);
                    let view_portal_floor =
                        crate::Length(portal_sector.floor.0 - view.position.0.z);

                    let y_portal_top = if view_portal_ceil.0 < view_ceil.0 {
                        let portal_ceil_t =
                            (view_portal_ceil.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                        Some((
                            lerpi(left_top.y, left_bottom.y, portal_ceil_t),
                            lerpi(right_top.y, right_bottom.y, portal_ceil_t),
                        ))
                    } else {
                        None
                    };

                    let y_portal_bottom = if view_portal_floor.0 > view_floor.0 {
                        let portal_floor_t =
                            (view_portal_floor.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                        Some((
                            lerpi(left_top.y, left_bottom.y, portal_floor_t),
                            lerpi(right_top.y, right_bottom.y, portal_floor_t),
                        ))
                    } else {
                        None
                    };

                    (y_portal_top, y_portal_bottom)
                } else {
                    (None, None)
                };

                let raw_hsv: Hsv = Srgb::<u8>::from(wall.color).into_format().into_color();
                let mut top_edge_prev = None;
                let mut portal_top_edge_prev = None;
                let mut portal_bottom_edge_prev = None;
                let mut bottom_edge_prev = None;

                for x in x_left..x_right {
                    let skip_floor_ceil = x >= self_portal.x_max as isize - GAP;
                    let skip_wall = x >= x_right - GAP;
                    let x_t = (x - left_top.x) as f32 / dx as f32;

                    let view_z = lerp(view_left.0.y, view_right.0.y, x_t);
                    let distance = view_z.abs();
                    let color = shade_color(raw_hsv, distance);
                    let ceiling_color = shade_color(ceiling_hsv, distance);
                    let floor_color = shade_color(floor_hsv, distance);

                    let y_top = lerpi(left_top.y, right_top.y, x_t);
                    let y_bottom = lerpi(left_bottom.y, right_bottom.y, x_t);
                    let y_min = y_min_vec[x as usize];
                    let y_max = y_max_vec[x as usize];
                    let y_top = y_top.clamp(y_min, y_max);
                    let y_bottom = y_bottom.clamp(y_min, y_max);

                    if !skip_floor_ceil {
                        draw_vertical_line(frame, x, y_min, y_top - GAP, ceiling_color);
                    }

                    let mut portal_top_edge = None;
                    let mut portal_bottom_edge = None;

                    if portal_sector.is_some() {
                        if let Some((y_portal_left_top, y_portal_right_top)) = y_portal_top {
                            let y_portal_top = lerpi(y_portal_left_top, y_portal_right_top, x_t)
                                .clamp(y_min, y_bottom);
                            if !skip_wall {
                                draw_vertical_line(frame, x, y_top, y_portal_top - GAP, color);
                            }
                            portal_top_edge = Some(Pixel::new(x, y_portal_top - GAP));
                            y_min_vec[x as usize] = y_portal_top;
                        } else {
                            y_min_vec[x as usize] =
                                portal_child_y_min(y_top, y_min, portal_ceiling_flush);
                        }

                        if let Some((portal_left_bottom_y, portal_right_bottom_y)) = y_portal_bottom
                        {
                            let y_portal_bottom =
                                lerpi(portal_left_bottom_y, portal_right_bottom_y, x_t)
                                    .clamp(y_top, y_max);
                            if !skip_wall {
                                draw_vertical_line(
                                    frame,
                                    x,
                                    y_portal_bottom,
                                    y_bottom - GAP,
                                    color,
                                );
                            }
                            portal_bottom_edge = Some(Pixel::new(x, y_portal_bottom - GAP));
                            y_max_vec[x as usize] = y_portal_bottom;
                        } else {
                            y_max_vec[x as usize] =
                                portal_child_y_max(y_bottom, y_max, portal_floor_flush);
                        }
                    } else if !skip_wall {
                        draw_vertical_line(frame, x, y_top, y_bottom - GAP, color);
                    }

                    if !skip_floor_ceil {
                        draw_vertical_line(frame, x, y_bottom, y_max - GAP, floor_color);
                    }

                    connect_edge_line(
                        frame,
                        &mut top_edge_prev,
                        (portal_sector.is_none() || !portal_ceiling_flush)
                            .then_some(Pixel::new(x, y_top - GAP)),
                    );
                    connect_edge_line(frame, &mut portal_top_edge_prev, portal_top_edge);
                    connect_edge_line(frame, &mut portal_bottom_edge_prev, portal_bottom_edge);
                    connect_edge_line(
                        frame,
                        &mut bottom_edge_prev,
                        (portal_sector.is_none() || !portal_floor_flush)
                            .then_some(Pixel::new(x, y_bottom - GAP)),
                    );
                }
            }
        }
    }
}

fn connect_edge_line(frame: &mut [u8], previous: &mut Option<Pixel>, current: Option<Pixel>) {
    if let Some(current) = current {
        if let Some(previous) = previous.replace(current) {
            draw_line(frame, previous, current, OUTLINE_COLOR);
        }
    } else {
        *previous = None;
    }
}

fn heights_match(a: f32, b: f32) -> bool {
    (a - b).abs() <= FLUSH_HEIGHT_EPSILON
}

fn portal_child_y_min(y_top: isize, y_min: isize, portal_ceiling_flush: bool) -> isize {
    if portal_ceiling_flush {
        (y_top - GAP).max(y_min)
    } else {
        y_top
    }
}

fn portal_child_y_max(y_bottom: isize, y_max: isize, portal_floor_flush: bool) -> isize {
    if portal_floor_flush {
        (y_bottom + GAP).min(y_max)
    } else {
        y_bottom
    }
}

fn shade_color(base_hsv: Hsv, distance: f32) -> RawColor {
    let brightness = if distance > FAR {
        BRIGHTNESS_FAR
    } else if distance < NEAR {
        BRIGHTNESS_NEAR
    } else {
        let distance_t = (distance - NEAR) / (FAR - NEAR);
        lerp(BRIGHTNESS_NEAR, BRIGHTNESS_FAR, distance_t)
    };
    let brightness_rounded = (brightness * 100.0).round() / 100.0;
    Hsv::new(base_hsv.hue, base_hsv.saturation, brightness_rounded).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_portal_ceiling_expands_child_span_into_threshold_gap() {
        assert_eq!(portal_child_y_min(40, 10, true), 39);
    }

    #[test]
    fn flush_portal_floor_expands_child_span_into_threshold_gap() {
        assert_eq!(portal_child_y_max(120, 200, true), 121);
    }

    #[test]
    fn non_flush_portal_bounds_preserve_threshold_lines() {
        assert_eq!(portal_child_y_min(40, 10, false), 40);
        assert_eq!(portal_child_y_max(120, 200, false), 120);
    }

    #[test]
    fn flush_portal_bound_expansion_respects_existing_clip_limits() {
        assert_eq!(portal_child_y_min(10, 10, true), 10);
        assert_eq!(portal_child_y_max(120, 120, true), 120);
    }
}
