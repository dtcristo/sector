use super::{
    frame::{draw_pixel, Pixel},
    math::{clip_wall, lerp, project},
    RenderView, BRIGHTNESS_FAR, BRIGHTNESS_NEAR, HEIGHT, NEAR, SHADE_BANDS, SHADE_FAR,
    TAN_FAC_FOV_Y_2, WIDTH,
};
use crate::{Position3, RawColor, Sector, SectorId};

use bevy::{math::vec2, prelude::*};
use palette::{Hsv, IntoColor, Srgb};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Copy, Clone)]
struct PortalSpan<'a> {
    sector: &'a Sector,
    source_sector: Option<SectorId>,
    x_min: isize,
    x_max: isize,
}

#[derive(Debug, Copy, Clone)]
struct DeferredWallColumn {
    x: isize,
    y_top: isize,
    y_bottom: isize,
    color: RawColor,
    surface_tag: SurfaceTag,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SurfaceKind {
    Wall,
    Floor,
    Ceiling,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SurfaceTag {
    kind: SurfaceKind,
    color: RawColor,
    plane_key: i32,
}

impl SurfaceTag {
    fn wall(color: RawColor) -> Self {
        Self {
            kind: SurfaceKind::Wall,
            color,
            plane_key: 0,
        }
    }

    fn floor(color: RawColor, height: f32) -> Self {
        Self {
            kind: SurfaceKind::Floor,
            color,
            plane_key: plane_key(height),
        }
    }

    fn ceiling(color: RawColor, height: f32) -> Self {
        Self {
            kind: SurfaceKind::Ceiling,
            color,
            plane_key: plane_key(height),
        }
    }
}

const CONTAINMENT_EPSILON: f32 = 0.001;
const OUTLINE_COLOR: RawColor = RawColor([0, 0, 0]);
const ROOT_PORTAL_EPSILON: f32 = 0.05;

pub(crate) fn render_world(frame: &mut [u8], view: &RenderView, sectors: &[&Sector]) {
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, *sector)).collect();
    let root_sectors = root_sectors(view, sectors, &sectors_by_id);
    if root_sectors.is_empty() {
        return;
    }

    let view_matrix = Mat3::from_rotation_z(-view.direction)
        * Mat3::from_translation(-vec2(view.position.0.x, view.position.0.y));
    let mut surfaces = vec![None; WIDTH as usize * HEIGHT as usize];
    render_sector_tree(
        frame,
        &mut surfaces,
        view,
        &sectors_by_id,
        &root_sectors,
        view_matrix,
    );

    apply_outlines(frame, &surfaces);
}

fn render_sector_tree<'a>(
    frame: &mut [u8],
    surfaces: &mut [Option<SurfaceTag>],
    view: &RenderView,
    sectors_by_id: &HashMap<SectorId, &'a Sector>,
    root_sectors: &[&'a Sector],
    view_matrix: Mat3,
) {
    let mut portal_queue = VecDeque::<PortalSpan>::new();
    let mut y_min_vec = vec![0; WIDTH as usize];
    let mut y_max_vec = vec![HEIGHT as isize; WIDTH as usize];
    let mut deferred_walls = Vec::new();

    portal_queue.extend(root_sectors.iter().copied().map(|root_sector| PortalSpan {
        sector: root_sector,
        source_sector: None,
        x_min: 0,
        x_max: WIDTH as isize,
    }));

    while let Some(self_portal) = portal_queue.pop_front() {
        let sector = self_portal.sector;
        let view_floor = crate::Length(sector.floor.0 - view.position.0.z);
        let view_ceil = crate::Length(sector.ceil.0 - view.position.0.z);
        let ceiling_hsv: Hsv = Srgb::<u8>::from(sector.ceil_color)
            .into_format()
            .into_color();
        let floor_hsv: Hsv = Srgb::<u8>::from(sector.floor_color)
            .into_format()
            .into_color();
        let ceiling_tag = SurfaceTag::ceiling(sector.ceil_color, sector.ceil.0);
        let floor_tag = SurfaceTag::floor(sector.floor_color, sector.floor.0);

        'walls: for wall in sector.wall_segments() {
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            if let Some((view_left, view_right)) = clip_wall(view_left, view_right) {
                let norm_left_top = project(view_left, view_ceil);
                let norm_left_bottom = project(view_left, view_floor);
                let norm_right_top = project(view_right, view_ceil);
                let norm_right_bottom = project(view_right, view_floor);

                let left_top_x = screen_x(norm_left_top.0.x);
                let left_top_y = screen_y(norm_left_top.0.y);
                let left_bottom_y = screen_y(norm_left_bottom.0.y);
                let right_top_x = screen_x(norm_right_top.0.x);
                let right_top_y = screen_y(norm_right_top.0.y);
                let right_bottom_y = screen_y(norm_right_bottom.0.y);

                let (
                    view_left,
                    left_top_x,
                    left_top_y,
                    left_bottom_y,
                    view_right,
                    right_top_x,
                    right_top_y,
                    right_bottom_y,
                ) = if left_top_x <= right_top_x {
                    (
                        view_left,
                        left_top_x,
                        left_top_y,
                        left_bottom_y,
                        view_right,
                        right_top_x,
                        right_top_y,
                        right_bottom_y,
                    )
                } else {
                    (
                        view_right,
                        right_top_x,
                        right_top_y,
                        right_bottom_y,
                        view_left,
                        left_top_x,
                        left_top_y,
                        left_bottom_y,
                    )
                };

                let dx = right_top_x - left_top_x;
                if dx <= f32::EPSILON {
                    continue 'walls;
                }

                let x_left = ((left_top_x - 0.5).ceil() as isize)
                    .clamp(self_portal.x_min, self_portal.x_max);
                let x_right = ((right_top_x - 0.5).ceil() as isize)
                    .clamp(self_portal.x_min, self_portal.x_max);
                if x_left >= x_right {
                    continue 'walls;
                }

                let portal_sector = wall
                    .portal_sector
                    .and_then(|id| sectors_by_id.get(&id).copied());

                let (y_portal_top, y_portal_bottom, y_drop_face_bottom) =
                    if let Some(portal_sector) = portal_sector {
                        if Some(portal_sector.id) != self_portal.source_sector {
                            portal_queue.push_back(PortalSpan {
                                sector: portal_sector,
                                source_sector: Some(sector.id),
                                x_min: x_left,
                                x_max: x_right,
                            });
                        }

                        let view_portal_ceil =
                            crate::Length(portal_sector.ceil.0 - view.position.0.z);
                        let view_portal_floor =
                            crate::Length(portal_sector.floor.0 - view.position.0.z);

                        let y_portal_top = if sector.no_ceiling && portal_sector.no_ceiling {
                            None
                        } else if view_portal_ceil.0 < view_ceil.0 {
                            let portal_ceil_t =
                                (view_portal_ceil.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                            Some((
                                lerp(left_top_y, left_bottom_y, portal_ceil_t),
                                lerp(right_top_y, right_bottom_y, portal_ceil_t),
                            ))
                        } else {
                            None
                        };

                        let y_portal_bottom = if view_portal_floor.0 > view_floor.0 {
                            let portal_floor_t =
                                (view_portal_floor.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                            Some((
                                lerp(left_top_y, left_bottom_y, portal_floor_t),
                                lerp(right_top_y, right_bottom_y, portal_floor_t),
                            ))
                        } else {
                            None
                        };

                        let y_drop_face_bottom = None;

                        (y_portal_top, y_portal_bottom, y_drop_face_bottom)
                    } else {
                        (None, None, None)
                    };

                let wall_hsv: Hsv = Srgb::<u8>::from(wall.color).into_format().into_color();
                let wall_tag = SurfaceTag::wall(wall.color);
                let portal_upper_color = wall.portal_upper_color.unwrap_or(wall.color);
                let portal_upper_hsv: Hsv = Srgb::<u8>::from(portal_upper_color)
                    .into_format()
                    .into_color();
                let portal_upper_tag = SurfaceTag::wall(portal_upper_color);
                let portal_lower_color = wall.portal_lower_color.unwrap_or(wall.color);
                let portal_lower_hsv: Hsv = Srgb::<u8>::from(portal_lower_color)
                    .into_format()
                    .into_color();
                let portal_lower_tag = SurfaceTag::wall(portal_lower_color);

                for x in x_left..x_right {
                    let x_t = ((x as f32 + 0.5) - left_top_x) / dx;
                    let x_t = x_t.clamp(0.0, 1.0);

                    let distance = wall_distance(view_left.0.y, view_right.0.y, x_t);
                    let wall_color = shade_color(wall_hsv, distance);
                    let portal_upper_color = shade_color(portal_upper_hsv, distance);
                    let portal_lower_color = shade_color(portal_lower_hsv, distance);
                    let y_top = lerp(left_top_y, right_top_y, x_t).round() as isize;
                    let y_bottom = lerp(left_bottom_y, right_bottom_y, x_t).round() as isize;
                    let y_min = y_min_vec[x as usize];
                    let y_max = y_max_vec[x as usize];
                    let y_top = y_top.clamp(y_min, y_max);
                    let y_bottom = y_bottom.clamp(y_min, y_max);

                    if !sector.no_ceiling {
                        draw_surface_column(
                            frame,
                            surfaces,
                            x,
                            y_min,
                            y_top,
                            ceiling_hsv,
                            ceiling_tag,
                            view_ceil.0,
                        );
                    }

                    if portal_sector.is_some() {
                        let portal_y_min = if let Some((y_portal_left_top, y_portal_right_top)) =
                            y_portal_top
                        {
                            let y_portal_top =
                                lerp(y_portal_left_top, y_portal_right_top, x_t).round() as isize;
                            let y_portal_top = y_portal_top.clamp(y_min, y_bottom);
                            draw_wall_column(
                                frame,
                                surfaces,
                                x,
                                y_top,
                                y_portal_top,
                                portal_upper_color,
                                portal_upper_tag,
                            );
                            y_portal_top
                        } else {
                            y_top
                        };
                        y_min_vec[x as usize] = portal_y_min;

                        let portal_y_max =
                            if let Some((portal_left_bottom_y, portal_right_bottom_y)) =
                                y_portal_bottom
                            {
                                let y_portal_bottom =
                                    lerp(portal_left_bottom_y, portal_right_bottom_y, x_t).round()
                                        as isize;
                                let y_portal_bottom = y_portal_bottom.clamp(y_top, y_max);
                                if y_portal_bottom <= y_bottom {
                                    draw_wall_column(
                                        frame,
                                        surfaces,
                                        x,
                                        y_portal_bottom,
                                        y_bottom,
                                        portal_lower_color,
                                        portal_lower_tag,
                                    );
                                    y_portal_bottom
                                } else {
                                    deferred_walls.push(DeferredWallColumn {
                                        x,
                                        y_top: y_bottom,
                                        y_bottom: y_portal_bottom,
                                        color: portal_lower_color,
                                        surface_tag: portal_lower_tag,
                                    });
                                    y_max
                                }
                            } else {
                                y_bottom
                            };
                        y_max_vec[x as usize] = portal_y_max;

                        if let Some((drop_left_bottom_y, drop_right_bottom_y)) = y_drop_face_bottom
                        {
                            let y_drop_bottom =
                                lerp(drop_left_bottom_y, drop_right_bottom_y, x_t).round() as isize;
                            let y_drop_bottom = y_drop_bottom.clamp(y_bottom, y_max);
                            if y_drop_bottom > y_bottom {
                                deferred_walls.push(DeferredWallColumn {
                                    x,
                                    y_top: y_bottom,
                                    y_bottom: y_drop_bottom,
                                    color: portal_lower_color,
                                    surface_tag: portal_lower_tag,
                                });
                            }
                        }
                    } else {
                        draw_wall_column(frame, surfaces, x, y_top, y_bottom, wall_color, wall_tag);
                    }

                    draw_surface_column(
                        frame,
                        surfaces,
                        x,
                        y_bottom,
                        y_max,
                        floor_hsv,
                        floor_tag,
                        view_floor.0,
                    );
                }
            }
        }
    }

    for deferred_wall in deferred_walls {
        draw_wall_column(
            frame,
            surfaces,
            deferred_wall.x,
            deferred_wall.y_top,
            deferred_wall.y_bottom,
            deferred_wall.color,
            deferred_wall.surface_tag,
        );
    }
}

fn root_sectors<'a>(
    view: &RenderView,
    sectors: &[&'a Sector],
    sectors_by_id: &HashMap<SectorId, &'a Sector>,
) -> Vec<&'a Sector> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let current_sector = view
        .current_sector
        .and_then(|id| sectors_by_id.get(&id).copied());

    for sector in sectors {
        if sector_contains_view(sector, view.position) && seen.insert(sector.id) {
            roots.push(*sector);
        }
    }

    if roots.is_empty() {
        for sector in sectors {
            if sector_has_portal_near_view(sector, view.position) && seen.insert(sector.id) {
                roots.push(*sector);
            }
        }
    }

    if roots.is_empty() {
        if let Some(current_sector) = current_sector {
            roots.push(current_sector);
            seen.insert(current_sector.id);
        }
    }

    if !roots.is_empty() {
        let point = view.position.truncate().0;
        let forward = vec2(-view.direction.sin(), view.direction.cos());
        let current_sector_contains_view =
            current_sector.is_some_and(|sector| sector_contains_view(sector, view.position));

        let mut index = 0;
        while let Some(sector) = roots.get(index).copied() {
            for wall in sector.wall_segments() {
                let Some(portal_id) = wall.portal_sector else {
                    continue;
                };
                let Some(portal_sector) = sectors_by_id.get(&portal_id).copied() else {
                    continue;
                };
                if (portal_sector.floor.0 - sector.floor.0).abs() > CONTAINMENT_EPSILON
                    || (portal_sector.ceil.0 - sector.ceil.0).abs() > CONTAINMENT_EPSILON
                {
                    continue;
                }
                let wall_direction = (wall.right.0 - wall.left.0).normalize_or_zero();
                if position_near_wall(point, wall.left.0, wall.right.0)
                    && (!current_sector_contains_view || wall_direction.dot(forward).abs() >= 0.6)
                    && seen.insert(portal_sector.id)
                {
                    roots.push(portal_sector);
                }
            }
            index += 1;
        }
    }

    roots
}

fn sector_has_portal_near_view(sector: &Sector, position: Position3) -> bool {
    if position.0.z < sector.floor.0 - CONTAINMENT_EPSILON {
        return false;
    }
    if position.0.z > sector.ceil.0 + CONTAINMENT_EPSILON {
        return false;
    }

    let point = position.truncate().0;
    sector
        .wall_segments()
        .into_iter()
        .filter(|wall| wall.portal_sector.is_some())
        .any(|wall| position_near_wall(point, wall.left.0, wall.right.0))
}

fn sector_contains_view(sector: &Sector, position: Position3) -> bool {
    if position.0.z < sector.floor.0 - CONTAINMENT_EPSILON {
        return false;
    }
    if position.0.z > sector.ceil.0 + CONTAINMENT_EPSILON {
        return false;
    }

    sector_contains_horizontal_point(sector, position.truncate().0)
}

fn sector_contains_horizontal_point(sector: &Sector, point: Vec2) -> bool {
    for index in 0..sector.vertices.len() {
        let current = sector.vertices[index].0;
        let next = sector.vertices[(index + 1) % sector.vertices.len()].0;
        if point_on_segment(point, current, next) {
            return true;
        }
    }

    let mut inside = false;
    for index in 0..sector.vertices.len() {
        let current = sector.vertices[index].0;
        let next = sector.vertices[(index + 1) % sector.vertices.len()].0;
        let crosses_scanline = (current.y > point.y) != (next.y > point.y);
        if !crosses_scanline {
            continue;
        }

        let intersect_x =
            ((next.x - current.x) * (point.y - current.y) / (next.y - current.y)) + current.x;
        if point.x <= intersect_x + CONTAINMENT_EPSILON {
            inside = !inside;
        }
    }

    inside
}

fn point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> bool {
    let segment = end - start;
    let point_delta = point - start;
    if segment.perp_dot(point_delta).abs() > CONTAINMENT_EPSILON {
        return false;
    }

    let dot = point_delta.dot(segment);
    if dot < -CONTAINMENT_EPSILON {
        return false;
    }

    dot <= segment.length_squared() + CONTAINMENT_EPSILON
}

fn position_near_wall(point: Vec2, start: Vec2, end: Vec2) -> bool {
    distance_to_segment(point, start, end) <= ROOT_PORTAL_EPSILON
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= CONTAINMENT_EPSILON {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    let projection = start + segment * t;
    point.distance(projection)
}

fn draw_wall_column(
    frame: &mut [u8],
    surfaces: &mut [Option<SurfaceTag>],
    x: isize,
    y_top: isize,
    y_bottom: isize,
    color: RawColor,
    surface_tag: SurfaceTag,
) {
    for y in y_top..y_bottom {
        draw_surface_pixel(frame, surfaces, x, y, color, surface_tag);
    }
}

fn draw_surface_column(
    frame: &mut [u8],
    surfaces: &mut [Option<SurfaceTag>],
    x: isize,
    y_top: isize,
    y_bottom: isize,
    base_hsv: Hsv,
    surface_tag: SurfaceTag,
    plane_height: f32,
) {
    for y in y_top..y_bottom {
        let color = shade_color(
            base_hsv,
            surface_distance(plane_height, surface_sample_y(plane_height, y)),
        );
        draw_surface_pixel(frame, surfaces, x, y, color, surface_tag);
    }
}

fn draw_surface_pixel(
    frame: &mut [u8],
    surfaces: &mut [Option<SurfaceTag>],
    x: isize,
    y: isize,
    color: RawColor,
    surface_tag: SurfaceTag,
) {
    draw_pixel(frame, Pixel::new(x, y), color);
    if let Some(index) = surface_index(x, y) {
        surfaces[index] = Some(surface_tag);
    }
}

fn apply_outlines(frame: &mut [u8], surfaces: &[Option<SurfaceTag>]) {
    for y in 0..HEIGHT as isize {
        for x in 0..WIDTH as isize {
            let Some(current) = surface_at(surfaces, x, y) else {
                continue;
            };
            let left = surface_at(surfaces, x - 1, y);
            let up = surface_at(surfaces, x, y - 1);
            let upper_left = surface_at(surfaces, x - 1, y - 1);
            let upper_right = surface_at(surfaces, x + 1, y - 1);

            let needs_outline = (should_outline_edge(current, left)
                && up == Some(current)
                && upper_left != Some(current))
                || (should_outline_edge(current, up)
                    && (upper_left != Some(current) || upper_right != Some(current)));

            if needs_outline {
                draw_pixel(frame, Pixel::new(x, y), OUTLINE_COLOR);
            }
        }
    }
}

fn should_outline_edge(current: SurfaceTag, neighbor: Option<SurfaceTag>) -> bool {
    neighbor != Some(current)
}

fn surface_index(x: isize, y: isize) -> Option<usize> {
    if x >= 0 && x < WIDTH as isize && y >= 0 && y < HEIGHT as isize {
        Some(y as usize * WIDTH as usize + x as usize)
    } else {
        None
    }
}

fn surface_at(surfaces: &[Option<SurfaceTag>], x: isize, y: isize) -> Option<SurfaceTag> {
    surface_index(x, y).and_then(|index| surfaces[index])
}

fn plane_key(height: f32) -> i32 {
    (height * 1000.0).round() as i32
}

fn screen_x(norm_x: f32) -> f32 {
    WIDTH as f32 * 0.5 + WIDTH as f32 * 0.5 * norm_x
}

fn screen_y(norm_y: f32) -> f32 {
    HEIGHT as f32 * 0.5 - HEIGHT as f32 * 0.5 * norm_y
}

fn wall_distance(view_left_depth: f32, view_right_depth: f32, screen_t: f32) -> f32 {
    let inv_left = 1.0 / view_left_depth.max(NEAR);
    let inv_right = 1.0 / view_right_depth.max(NEAR);
    1.0 / lerp(inv_left, inv_right, screen_t).max(f32::EPSILON)
}

fn surface_sample_y(plane_height: f32, y: isize) -> f32 {
    if plane_height < 0.0 {
        y as f32 + 1.0
    } else {
        y as f32
    }
}

fn surface_distance(plane_height: f32, y: f32) -> f32 {
    let screen_offset = (HEIGHT as f32 * 0.5 - y).abs();
    if plane_height.abs() <= f32::EPSILON || screen_offset <= f32::EPSILON {
        return f32::INFINITY;
    }

    (plane_height.abs() * HEIGHT as f32 * 0.5 / (screen_offset * *TAN_FAC_FOV_Y_2)).max(NEAR)
}

fn shade_color(base_hsv: Hsv, distance: f32) -> RawColor {
    let brightness = lerp(BRIGHTNESS_NEAR, BRIGHTNESS_FAR, shade_band_t(distance));
    Hsv::new(
        base_hsv.hue,
        base_hsv.saturation,
        (base_hsv.value * brightness).clamp(0.0, 1.0),
    )
    .into()
}

fn shade_band(distance: f32) -> usize {
    let bands = SHADE_BANDS.saturating_sub(1);
    if bands == 0 {
        return 0;
    }

    if distance <= NEAR {
        0
    } else if distance >= SHADE_FAR {
        bands
    } else {
        (((distance - NEAR) / (SHADE_FAR - NEAR)) * bands as f32).floor() as usize
    }
}

fn shade_band_t(distance: f32) -> f32 {
    let bands = SHADE_BANDS.saturating_sub(1);
    if bands == 0 {
        0.0
    } else {
        shade_band(distance) as f32 / bands as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Length, Position2, Position3, CEILING_COLOR, FLOOR_COLOR};
    use bevy::math::{vec2, vec3};
    use std::collections::BTreeSet;

    fn sector(
        id: u32,
        vertices: &[(f32, f32)],
        portal_sectors: &[Option<u32>],
        floor: f32,
        ceil: f32,
    ) -> Sector {
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
            portal_walkable: portal_sectors.iter().map(|_| true).collect(),
            colors: vec![RawColor([255, 255, 255]); vertices.len()],
            portal_upper_colors: vec![None; vertices.len()],
            portal_lower_colors: vec![None; vertices.len()],
            floor: Length(floor),
            ceil: Length(ceil),
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            no_ceiling: false,
        }
    }

    #[test]
    fn root_sectors_include_portal_neighbor_on_shared_boundary() {
        let sectors = vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                0.0,
                4.0,
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
                0.0,
                4.0,
            ),
        ];
        let sector_refs = sectors.iter().collect::<Vec<_>>();
        let sectors_by_id = sector_refs
            .iter()
            .map(|sector| (sector.id, *sector))
            .collect::<HashMap<_, _>>();
        let view = RenderView::new(
            Position3(vec3(0.0, 1.0, 1.62)),
            -std::f32::consts::FRAC_PI_2,
            Some(SectorId(0)),
        );

        let roots = root_sectors(&view, &sector_refs, &sectors_by_id);

        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|sector| sector.id == SectorId(0)));
        assert!(roots.iter().any(|sector| sector.id == SectorId(1)));
    }

    #[test]
    fn root_sectors_include_same_height_neighbor_when_view_is_close_and_parallel() {
        let sectors = vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                0.0,
                4.0,
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
                0.0,
                4.0,
            ),
        ];
        let sector_refs = sectors.iter().collect::<Vec<_>>();
        let sectors_by_id = sector_refs
            .iter()
            .map(|sector| (sector.id, *sector))
            .collect::<HashMap<_, _>>();
        let view = RenderView::new(
            Position3(vec3(0.0, 0.975, 1.62)),
            -std::f32::consts::FRAC_PI_2,
            Some(SectorId(0)),
        );

        let roots = root_sectors(&view, &sector_refs, &sectors_by_id);

        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|sector| sector.id == SectorId(0)));
        assert!(roots.iter().any(|sector| sector.id == SectorId(1)));
    }

    #[test]
    fn root_sectors_prefer_geometric_sector_over_stale_current_sector() {
        let sectors = vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                0.0,
                4.0,
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
                0.0,
                4.0,
            ),
        ];
        let sector_refs = sectors.iter().collect::<Vec<_>>();
        let sectors_by_id = sector_refs
            .iter()
            .map(|sector| (sector.id, *sector))
            .collect::<HashMap<_, _>>();
        let view = RenderView::new(Position3(vec3(0.0, 2.0, 1.62)), 0.0, Some(SectorId(0)));

        let roots = root_sectors(&view, &sector_refs, &sectors_by_id);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].id, SectorId(1));
    }

    #[test]
    fn wall_surfaces_skip_outline_when_colors_match() {
        let wall = SurfaceTag::wall(RawColor([1, 2, 3]));
        assert!(!should_outline_edge(wall, Some(wall)));
    }

    #[test]
    fn wall_surfaces_outline_when_colors_differ() {
        let wall = SurfaceTag::wall(RawColor([1, 2, 3]));
        let other = SurfaceTag::wall(RawColor([4, 5, 6]));
        assert!(should_outline_edge(wall, Some(other)));
    }

    #[test]
    fn matching_ceilings_skip_outline_when_height_and_color_match() {
        let ceiling = SurfaceTag::ceiling(*CEILING_COLOR, 3.2);
        assert!(!should_outline_edge(ceiling, Some(ceiling)));
    }

    #[test]
    fn differing_ceiling_heights_keep_outline() {
        let ceiling = SurfaceTag::ceiling(*CEILING_COLOR, 3.2);
        let higher = SurfaceTag::ceiling(*CEILING_COLOR, 3.4);
        assert!(should_outline_edge(ceiling, Some(higher)));
    }

    #[test]
    fn portal_trim_colors_override_wall_color() {
        let mut near = sector(
            0,
            &[(-2.0, 4.0), (2.0, 4.0), (2.0, -2.0), (-2.0, -2.0)],
            &[Some(1), None, None, None],
            0.0,
            4.0,
        );
        near.colors[0] = RawColor([180, 60, 60]);
        near.portal_upper_colors[0] = Some(RawColor([80, 160, 220]));
        near.portal_lower_colors[0] = Some(RawColor([220, 140, 80]));

        let mut far = sector(
            1,
            &[(2.0, 4.0), (-2.0, 4.0), (-2.0, 8.0), (2.0, 8.0)],
            &[Some(0), None, None, None],
            1.5,
            2.5,
        );
        far.colors[0] = RawColor([60, 180, 60]);

        let sectors = [&near, &far];
        let view = RenderView::new(Position3(vec3(0.0, 0.0, 1.62)), 0.0, Some(SectorId(0)));
        let mut frame = vec![255; WIDTH as usize * HEIGHT as usize * 4];
        let mut baseline_frame = vec![255; WIDTH as usize * HEIGHT as usize * 4];

        render_world(&mut frame, &view, &sectors);
        near.portal_upper_colors[0] = None;
        near.portal_lower_colors[0] = None;
        render_world(&mut baseline_frame, &view, &[&near, &far]);

        assert_ne!(frame, baseline_frame);
    }

    #[test]
    fn descending_portal_does_not_draw_lower_backface() {
        let mut high = sector(
            0,
            &[(-2.0, 4.0), (2.0, 4.0), (2.0, -2.0), (-2.0, -2.0)],
            &[Some(1), None, None, None],
            1.2,
            4.4,
        );
        high.portal_lower_colors[0] = Some(RawColor([220, 40, 40]));
        let low = sector(
            1,
            &[(2.0, 4.0), (-2.0, 4.0), (-2.0, 8.0), (2.0, 8.0)],
            &[Some(0), None, None, None],
            0.0,
            3.2,
        );
        let view = RenderView::new(Position3(vec3(0.0, 0.0, 2.82)), 0.0, Some(SectorId(0)));
        let mut frame = vec![255; WIDTH as usize * HEIGHT as usize * 4];
        let mut baseline_frame = vec![255; WIDTH as usize * HEIGHT as usize * 4];

        render_world(&mut frame, &view, &[&high, &low]);
        high.portal_lower_colors[0] = None;
        render_world(&mut baseline_frame, &view, &[&high, &low]);

        assert_eq!(frame, baseline_frame);
    }

    #[test]
    fn surface_distance_matches_projected_floor_and_ceiling_rows() {
        for (plane_height, expected_distances) in [
            (-1.62_f32, [4.0_f32, 8.0, 16.0]),
            (2.38_f32, [4.0, 8.0, 16.0]),
        ] {
            for expected_distance in expected_distances {
                let row = Pixel::from(project(
                    Position2(vec2(0.0, expected_distance)),
                    Length(plane_height),
                ))
                .y as f32;
                let actual_distance = surface_distance(plane_height, row);

                assert!(
                    (actual_distance - expected_distance).abs() < 0.35,
                    "expected projected row for plane height {plane_height} to recover {expected_distance}, got {actual_distance}",
                );
            }
        }
    }

    #[test]
    fn projected_surface_rows_share_wall_shade_bands() {
        for (plane_height, expected_distances) in [
            (-1.62_f32, [4.0_f32, 8.0, 16.0]),
            (2.38_f32, [4.0, 8.0, 16.0]),
        ] {
            for expected_distance in expected_distances {
                let row = Pixel::from(project(
                    Position2(vec2(0.0, expected_distance)),
                    Length(plane_height),
                ))
                .y as f32;

                assert_eq!(
                    shade_band(surface_distance(
                        plane_height,
                        surface_sample_y(plane_height, row as isize),
                    )),
                    shade_band(expected_distance),
                );
            }
        }
    }

    #[test]
    fn wall_distance_recovers_depth_from_projected_column() {
        let view_left = Position2(vec2(-2.0, 4.0));
        let view_right = Position2(vec2(2.0, 8.0));
        let sample = Position2(vec2(-1.0, 5.0));
        let left_x = screen_x(project(view_left, Length(0.0)).0.x);
        let right_x = screen_x(project(view_right, Length(0.0)).0.x);
        let sample_x = screen_x(project(sample, Length(0.0)).0.x);
        let screen_t = (sample_x - left_x) / (right_x - left_x);

        let recovered = wall_distance(view_left.0.y, view_right.0.y, screen_t);

        assert!((recovered - sample.0.y).abs() < 0.01);
    }

    #[test]
    fn shade_color_uses_limited_bands_for_retro_falloff() {
        let base_hsv: Hsv = Srgb::<u8>::new(255, 0, 0).into_format().into_color();
        let shades = (0..96)
            .map(|step| shade_color(base_hsv, NEAR + step as f32 * 0.35).0[0])
            .collect::<BTreeSet<_>>();

        assert!(shades.len() <= SHADE_BANDS);
        assert!(shades.len() > SHADE_BANDS / 2);
    }

    fn paint_surface_span(
        surfaces: &mut [Option<SurfaceTag>],
        x: isize,
        y_min: isize,
        y_max: isize,
        tag: SurfaceTag,
    ) {
        for y in y_min..y_max {
            if let Some(index) = surface_index(x, y) {
                surfaces[index] = Some(tag);
            }
        }
    }

    fn assert_boundary_is_single_pixel_thick(boundary_rows: &[isize]) {
        let mut frame = vec![255; WIDTH as usize * HEIGHT as usize * 4];
        let mut surfaces = vec![None; WIDTH as usize * HEIGHT as usize];
        let top = SurfaceTag::ceiling(RawColor([120, 120, 120]), 3.2);
        let bottom = SurfaceTag::floor(RawColor([180, 180, 180]), 0.0);
        let start_x = 48_isize;

        for (offset, boundary_row) in boundary_rows.iter().copied().enumerate() {
            let x = start_x + offset as isize;
            paint_surface_span(&mut surfaces, x, 72, boundary_row, top);
            paint_surface_span(&mut surfaces, x, boundary_row, 104, bottom);
        }

        apply_outlines(&mut frame, &surfaces);

        for (offset, boundary_row) in boundary_rows
            .iter()
            .copied()
            .enumerate()
            .skip(1)
            .take(boundary_rows.len().saturating_sub(2))
        {
            let x = (start_x + offset as isize) as usize;
            let outline_pixels = ((boundary_row - 1) as usize..=(boundary_row + 1) as usize)
                .filter(|&y| frame[(y * WIDTH as usize + x) * 4..][..4] == [0, 0, 0, 255])
                .count();
            assert_eq!(
                outline_pixels, 1,
                "expected a single outline pixel near column {x}, found {outline_pixels}"
            );
        }
    }

    #[test]
    fn apply_outlines_keeps_sloped_boundaries_single_pixel_thick() {
        assert_boundary_is_single_pixel_thick(&[80, 81, 82, 83, 84, 85]);
        assert_boundary_is_single_pixel_thick(&[85, 84, 83, 82, 81, 80]);
    }

    #[test]
    fn apply_outlines_keeps_flat_boundaries_single_pixel_thick() {
        assert_boundary_is_single_pixel_thick(&[82, 82, 82, 82, 82, 82]);
    }
}
