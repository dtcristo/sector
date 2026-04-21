use crate::{Position3, Sector, SectorId, WallSegment};

use bevy::math::{Vec2, Vec3};
use std::collections::HashMap;

use super::{
    desired_fly_vertical_velocity, desired_horizontal_velocity, jump_speed_mps, Player,
    PlayerInput, EARTH_GRAVITY_MPS2, PLAYER_CROUCH_EYE_HEIGHT_METERS, PLAYER_EYE_HEIGHT_METERS,
    PLAYER_HEIGHT_METERS, PLAYER_MAX_STEP_HEIGHT_METERS, PLAYER_RADIUS_METERS,
};

const POSITION_EPSILON: f32 = 0.0001;
const AIR_CROUCH_FEET_LIFT: f32 = PLAYER_EYE_HEIGHT_METERS - PLAYER_CROUCH_EYE_HEIGHT_METERS;

pub fn simulate_player(
    player: &mut Player,
    input: PlayerInput,
    dt_seconds: f32,
    sectors: &[Sector],
) {
    if sectors.is_empty() {
        return;
    }

    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, sector)).collect();
    if player.noclip {
        simulate_player_noclip(player, input, dt_seconds, sectors, &sectors_by_id);
        return;
    }
    if player.fly_mode {
        simulate_player_fly(player, input, dt_seconds, sectors, &sectors_by_id);
        return;
    }

    player.current_sector = resolve_current_sector_with_height(
        player.position,
        player.current_sector,
        sectors,
        &sectors_by_id,
        player.height(),
    );
    update_crouch_state(&mut *player, input, &sectors_by_id);

    let horizontal_velocity = desired_horizontal_velocity(player, input);
    player.velocity.x = horizontal_velocity.x;
    player.velocity.y = horizontal_velocity.y;

    let movement_delta = horizontal_velocity.truncate() * dt_seconds;
    let (horizontal_position, sector_id, stepped_floor) =
        move_player_horizontally(player, movement_delta, sectors, &sectors_by_id);
    player.position.0.x = horizontal_position.x;
    player.position.0.y = horizontal_position.y;
    player.current_sector = sector_id.or(player.current_sector);

    if let Some(step_floor) = stepped_floor {
        player.position.0.z = step_floor;
        player.velocity.z = 0.0;
        player.grounded = true;
    }

    let current_sector = player
        .current_sector
        .and_then(|sector_id| sectors_by_id.get(&sector_id).copied());

    if let Some(current_sector) = current_sector {
        if player.position.0.z > current_sector.floor.0 + POSITION_EPSILON {
            player.grounded = false;
        } else {
            player.position.0.z = current_sector.floor.0;
            player.grounded = true;
            if player.velocity.z < 0.0 {
                player.velocity.z = 0.0;
            }
        }
    }

    if input.jump_pressed && player.grounded && !player.crouching {
        player.velocity.z = jump_speed_mps();
        player.grounded = false;
    }

    if !player.grounded || player.velocity.z > 0.0 {
        player.velocity.z -= EARTH_GRAVITY_MPS2 * dt_seconds;
        player.position.0.z += player.velocity.z * dt_seconds;
    }

    player.current_sector = resolve_current_sector_with_height(
        player.position,
        player.current_sector,
        sectors,
        &sectors_by_id,
        player.height(),
    );
    update_crouch_state(&mut *player, input, &sectors_by_id);

    if let Some(current_sector) = player
        .current_sector
        .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
    {
        let max_feet_z = current_sector.ceil.0 - player.height();
        if player.position.0.z > max_feet_z {
            player.position.0.z = max_feet_z;
            if player.velocity.z > 0.0 {
                player.velocity.z = 0.0;
            }
        }

        if player.position.0.z <= current_sector.floor.0 + POSITION_EPSILON {
            player.position.0.z = current_sector.floor.0;
            player.velocity.z = player.velocity.z.max(0.0);
            player.grounded = true;
        } else {
            player.grounded = false;
        }
    }
}

pub fn resolve_player_sector(player: &Player, sectors: &[Sector]) -> Option<SectorId> {
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, sector)).collect();
    if player.noclip {
        resolve_current_sector_noclip(
            player.position,
            player.current_sector,
            sectors,
            &sectors_by_id,
        )
    } else {
        resolve_current_sector_with_height(
            player.position,
            player.current_sector,
            sectors,
            &sectors_by_id,
            player.height(),
        )
    }
}

pub fn resolve_current_sector(
    position: Position3,
    current_sector: Option<SectorId>,
    sectors: &[Sector],
) -> Option<SectorId> {
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, sector)).collect();
    resolve_current_sector_with_height(
        position,
        current_sector,
        sectors,
        &sectors_by_id,
        PLAYER_HEIGHT_METERS,
    )
}

fn simulate_player_noclip(
    player: &mut Player,
    input: PlayerInput,
    dt_seconds: f32,
    sectors: &[Sector],
    sectors_by_id: &HashMap<SectorId, &Sector>,
) {
    player.current_sector = resolve_current_sector_noclip(
        player.position,
        player.current_sector,
        sectors,
        sectors_by_id,
    );
    update_noclip_grounded_state(player, sectors_by_id);
    update_crouch_state_noclip(player, noclip_crouch_input(input, player.fly_mode));

    let horizontal_velocity = desired_horizontal_velocity(player, input);
    let vertical_velocity = noclip_vertical_velocity(input, player.fly_mode);
    player.velocity = Vec3::new(
        horizontal_velocity.x,
        horizontal_velocity.y,
        vertical_velocity,
    );
    player.position.0 += player.velocity * dt_seconds;

    player.current_sector = resolve_current_sector_noclip(
        player.position,
        player.current_sector,
        sectors,
        sectors_by_id,
    );
    update_noclip_grounded_state(player, sectors_by_id);
}

fn simulate_player_fly(
    player: &mut Player,
    input: PlayerInput,
    dt_seconds: f32,
    sectors: &[Sector],
    sectors_by_id: &HashMap<SectorId, &Sector>,
) {
    player.current_sector = resolve_current_sector_with_height(
        player.position,
        player.current_sector,
        sectors,
        sectors_by_id,
        player.height(),
    );
    update_crouch_state(
        player,
        PlayerInput {
            crouch_pressed: false,
            ..input
        },
        sectors_by_id,
    );

    let vertical_velocity = desired_fly_vertical_velocity(input);
    player.velocity.z = vertical_velocity;
    player.position.0.z += vertical_velocity * dt_seconds;
    player.grounded = false;
    player.current_sector = resolve_current_sector_with_height(
        player.position,
        player.current_sector,
        sectors,
        sectors_by_id,
        player.height(),
    );

    let horizontal_velocity = desired_horizontal_velocity(player, input);
    player.velocity.x = horizontal_velocity.x;
    player.velocity.y = horizontal_velocity.y;
    let movement_delta = horizontal_velocity.truncate() * dt_seconds;
    let (horizontal_position, sector_id, _) =
        move_player_horizontally(player, movement_delta, sectors, sectors_by_id);
    player.position.0.x = horizontal_position.x;
    player.position.0.y = horizontal_position.y;
    player.current_sector = sector_id.or(player.current_sector);
    player.current_sector = resolve_current_sector_with_height(
        player.position,
        player.current_sector,
        sectors,
        sectors_by_id,
        player.height(),
    );

    if let Some(current_sector) = player
        .current_sector
        .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
    {
        let min_feet_z = current_sector.floor.0;
        let max_feet_z = current_sector.ceil.0 - player.height();
        if player.position.0.z < min_feet_z {
            player.position.0.z = min_feet_z;
            if player.velocity.z < 0.0 {
                player.velocity.z = 0.0;
            }
        }
        if player.position.0.z > max_feet_z {
            player.position.0.z = max_feet_z;
            if player.velocity.z > 0.0 {
                player.velocity.z = 0.0;
            }
        }

        player.current_sector = resolve_current_sector_with_height(
            player.position,
            player.current_sector,
            sectors,
            sectors_by_id,
            player.height(),
        );
        player.grounded = (player.position.0.z - min_feet_z).abs() <= POSITION_EPSILON;
    } else {
        player.grounded = false;
    }
}

fn resolve_current_sector_with_height(
    position: Position3,
    current_sector: Option<SectorId>,
    sectors: &[Sector],
    sectors_by_id: &HashMap<SectorId, &Sector>,
    player_height: f32,
) -> Option<SectorId> {
    let mut first_matching_sector = None;
    let mut current_sector_matches = false;
    for sector in sectors {
        if sector_matches_position_for_resolution(sector, position, player_height) {
            first_matching_sector.get_or_insert(sector.id);
            if Some(sector.id) == current_sector {
                current_sector_matches = true;
            }
        }
    }

    if first_matching_sector.is_none() {
        if let Some(current_sector_id) = current_sector {
            if let Some(sector) = sectors_by_id.get(&current_sector_id).copied() {
                if sector_contains_horizontal_point(sector, position.truncate().0) {
                    return Some(current_sector_id);
                }
            }
        }
        return None;
    }

    if let Some(current_sector_id) = current_sector {
        if let Some(sector) = sectors_by_id.get(&current_sector_id).copied() {
            if let Some(adjacent_sector_id) = sector
                .wall_segments_iter()
                .filter_map(|wall| wall.portal_walkable.then_some(wall.portal_sector).flatten())
                .find(|portal_id| {
                    sectors_by_id
                        .get(portal_id)
                        .copied()
                        .is_some_and(|portal_sector| {
                            sector_matches_position_for_resolution(
                                portal_sector,
                                position,
                                player_height,
                            ) && (!sector_contains_player_with_height(
                                sector,
                                position,
                                player_height,
                            ) || position_on_portal_boundary(sector, position, *portal_id))
                        })
                })
            {
                return Some(adjacent_sector_id);
            }

            if current_sector_matches {
                return Some(current_sector_id);
            }
        }
    }

    first_matching_sector
}

fn resolve_current_sector_noclip(
    position: Position3,
    current_sector: Option<SectorId>,
    sectors: &[Sector],
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> Option<SectorId> {
    if let Some(current_sector_id) = current_sector {
        if let Some(sector) = sectors_by_id.get(&current_sector_id).copied() {
            if sector_contains_horizontal_point(sector, position.truncate().0) {
                return Some(current_sector_id);
            }
        }
    }

    sectors
        .iter()
        .filter(|sector| sector_contains_horizontal_point(sector, position.truncate().0))
        .min_by(|left, right| {
            vertical_distance_to_sector(position.0.z, left)
                .partial_cmp(&vertical_distance_to_sector(position.0.z, right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|sector| sector.id)
}

pub fn sector_contains_player(sector: &Sector, position: Position3) -> bool {
    sector_contains_player_with_height(sector, position, PLAYER_HEIGHT_METERS)
}

fn sector_contains_player_with_height(
    sector: &Sector,
    position: Position3,
    player_height: f32,
) -> bool {
    if position.0.z < sector.floor.0 - POSITION_EPSILON {
        return false;
    }
    if position.0.z + player_height > sector.ceil.0 + POSITION_EPSILON {
        return false;
    }

    sector_contains_horizontal_point(sector, position.truncate().0)
}

fn sector_matches_position_for_resolution(
    sector: &Sector,
    position: Position3,
    player_height: f32,
) -> bool {
    if position.0.z + player_height > sector.ceil.0 + POSITION_EPSILON {
        return false;
    }

    if position.0.z < sector.floor.0 - POSITION_EPSILON
        && sector.floor.0 - position.0.z > PLAYER_MAX_STEP_HEIGHT_METERS
    {
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
        if point.x <= intersect_x + POSITION_EPSILON {
            inside = !inside;
        }
    }

    inside
}

fn move_player_horizontally(
    player: &Player,
    desired_delta: Vec2,
    sectors: &[Sector],
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> (Vec2, Option<SectorId>, Option<f32>) {
    let mut position = player.position.truncate().0;
    let sector_id = player.current_sector.or_else(|| {
        resolve_current_sector_with_height(
            player.position,
            None,
            sectors,
            sectors_by_id,
            player.height(),
        )
    });
    let remaining = desired_delta;
    let mut stepped_to_floor = None;

    if sector_id.is_none() {
        let target = position + remaining;
        let target_position = Position3(Vec3::new(target.x, target.y, player.position.0.z));
        let target_sector = resolve_current_sector_with_height(
            target_position,
            None,
            sectors,
            sectors_by_id,
            player.height(),
        );
        return (target, target_sector, None);
    }

    let mut sector_id = sector_id.expect("sector_id checked above");

    for _ in 0..4 {
        if remaining.length_squared() <= POSITION_EPSILON {
            break;
        }

        let Some(sector) = sectors_by_id.get(&sector_id).copied() else {
            break;
        };
        let target = position + remaining;

        if let Some(transition) =
            find_portal_transition(position, target, player, sector, &sectors_by_id)
        {
            position = target;
            sector_id = transition.target_sector_id;
            if transition.step_to_floor.is_some() {
                stepped_to_floor = transition.step_to_floor;
            }
            break;
        }

        let clipped_target =
            clip_target_against_blocking_walls(position, target, player, sector, &sectors_by_id);
        if let Some(transition) =
            find_portal_transition(position, clipped_target, player, sector, &sectors_by_id)
        {
            position = clipped_target;
            sector_id = transition.target_sector_id;
            if transition.step_to_floor.is_some() {
                stepped_to_floor = transition.step_to_floor;
            }
            break;
        }

        position = clipped_target;
        break;
    }

    (position, Some(sector_id), stepped_to_floor)
}

fn find_portal_transition(
    start: Vec2,
    target: Vec2,
    player: &Player,
    sector: &Sector,
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> Option<PortalTransition> {
    sector
        .wall_segments_iter()
        .filter_map(|wall| {
            if !wall.portal_walkable {
                return None;
            }
            let target_sector = wall
                .portal_sector
                .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())?;
            portal_transition_for_wall(start, target, player, sector, wall, target_sector)
        })
        .next()
}

fn clip_target_against_blocking_walls(
    start: Vec2,
    target: Vec2,
    player: &Player,
    sector: &Sector,
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> Vec2 {
    let limit = -PLAYER_RADIUS_METERS;
    let mut clipped = target;

    for _ in 0..sector.vertices.len().max(1) {
        let mut changed = false;

        for wall in sector.wall_segments_iter() {
            if wall.portal_walkable
                && wall
                    .portal_sector
                    .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
                    .is_some_and(|target_sector| portal_clearance(player, target_sector).is_some())
            {
                continue;
            }

            let projection = segment_projection(clipped, wall.left.0, wall.right.0);
            if projection <= POSITION_EPSILON || projection >= 1.0 - POSITION_EPSILON {
                let endpoint = if projection <= 0.5 {
                    wall.left.0
                } else {
                    wall.right.0
                };
                let delta = clipped - endpoint;
                let distance = delta.length();
                if distance >= PLAYER_RADIUS_METERS - POSITION_EPSILON {
                    continue;
                }

                let correction_direction = if distance > POSITION_EPSILON {
                    delta / distance
                } else {
                    -wall_outward_normal(wall)
                };
                clipped = endpoint + correction_direction * PLAYER_RADIUS_METERS;
                changed = true;
                continue;
            }

            let start_distance = signed_distance_to_wall(start, wall);
            let end_distance = signed_distance_to_wall(clipped, wall);
            if start_distance > POSITION_EPSILON && end_distance < start_distance {
                continue;
            }
            if end_distance <= limit + POSITION_EPSILON {
                continue;
            }

            clipped -= wall_outward_normal(wall) * (end_distance - limit);
            changed = true;
        }

        if !changed {
            break;
        }
    }

    clipped
}

#[derive(Debug, Copy, Clone)]
struct PortalTransition {
    target_sector_id: SectorId,
    step_to_floor: Option<f32>,
}

fn portal_transition_for_wall(
    start: Vec2,
    target: Vec2,
    player: &Player,
    sector: &Sector,
    wall: WallSegment,
    target_sector: &Sector,
) -> Option<PortalTransition> {
    if !wall.portal_walkable {
        return None;
    }

    if !sector_contains_horizontal_point(target_sector, target)
        && !segments_intersect(start, target, wall.left.0, wall.right.0)
        && !position_on_portal_boundary(
            sector,
            Position3(Vec3::new(target.x, target.y, player.position.0.z)),
            target_sector.id,
        )
    {
        return None;
    }

    let feet_z = portal_clearance(player, target_sector)?;
    let floor_delta = target_sector.floor.0 - player.position.0.z;
    let step_to_floor = if player.grounded
        && !player.fly_mode
        && floor_delta.abs() > POSITION_EPSILON
        && floor_delta.abs() <= PLAYER_MAX_STEP_HEIGHT_METERS
    {
        Some(target_sector.floor.0)
    } else {
        None
    };

    Some(PortalTransition {
        target_sector_id: target_sector.id,
        step_to_floor: step_to_floor
            .or_else(|| (feet_z > player.position.0.z + POSITION_EPSILON).then_some(feet_z)),
    })
}

fn portal_clearance(player: &Player, target_sector: &Sector) -> Option<f32> {
    let mut feet_z = player.position.0.z;
    if target_sector.floor.0 > feet_z + POSITION_EPSILON {
        if !player.grounded || target_sector.floor.0 - feet_z > PLAYER_MAX_STEP_HEIGHT_METERS {
            return None;
        }
        feet_z = target_sector.floor.0;
    }

    if feet_z + player.height() > target_sector.ceil.0 - POSITION_EPSILON {
        return None;
    }

    Some(feet_z)
}

fn vertical_distance_to_sector(feet_z: f32, sector: &Sector) -> f32 {
    if feet_z < sector.floor.0 {
        sector.floor.0 - feet_z
    } else if feet_z > sector.ceil.0 {
        feet_z - sector.ceil.0
    } else {
        0.0
    }
}

fn update_crouch_state(
    player: &mut Player,
    input: PlayerInput,
    sectors_by_id: &HashMap<SectorId, &Sector>,
) {
    if input.crouch_pressed {
        if !player.crouching {
            if !player.grounded {
                player.position.0.z += AIR_CROUCH_FEET_LIFT;
            }
            player.crouching = true;
        }
        return;
    }

    if !player.crouching {
        return;
    }

    let Some(current_sector) = player
        .current_sector
        .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
    else {
        if !player.grounded {
            player.position.0.z -= AIR_CROUCH_FEET_LIFT;
        }
        player.crouching = false;
        return;
    };

    let standing_feet_z = if player.grounded {
        player.position.0.z
    } else {
        player.position.0.z - AIR_CROUCH_FEET_LIFT
    };
    if can_use_height_in_sector(current_sector, standing_feet_z, PLAYER_HEIGHT_METERS) {
        if !player.grounded {
            player.position.0.z = standing_feet_z;
        }
        player.crouching = false;
    }
}

fn update_crouch_state_noclip(player: &mut Player, input: PlayerInput) {
    if input.crouch_pressed {
        if !player.crouching {
            if !player.grounded {
                player.position.0.z += AIR_CROUCH_FEET_LIFT;
            }
            player.crouching = true;
        }
        return;
    }

    if !player.crouching {
        return;
    }

    if !player.grounded {
        player.position.0.z -= AIR_CROUCH_FEET_LIFT;
    }
    player.crouching = false;
}

fn noclip_crouch_input(input: PlayerInput, fly_mode: bool) -> PlayerInput {
    if fly_mode {
        PlayerInput {
            crouch_pressed: false,
            ..input
        }
    } else {
        input
    }
}

fn noclip_vertical_velocity(input: PlayerInput, fly_mode: bool) -> f32 {
    if fly_mode {
        desired_fly_vertical_velocity(input)
    } else {
        0.0
    }
}

fn update_noclip_grounded_state(player: &mut Player, sectors_by_id: &HashMap<SectorId, &Sector>) {
    player.grounded = player
        .current_sector
        .and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
        .is_some_and(|sector| (player.position.0.z - sector.floor.0).abs() <= POSITION_EPSILON);
}

fn can_use_height_in_sector(sector: &Sector, feet_z: f32, height: f32) -> bool {
    feet_z >= sector.floor.0 - POSITION_EPSILON
        && feet_z + height <= sector.ceil.0 + POSITION_EPSILON
}

fn wall_outward_normal(wall: WallSegment) -> Vec2 {
    let edge = wall.right.0 - wall.left.0;
    Vec2::new(-edge.y, edge.x).normalize_or_zero()
}

fn signed_distance_to_wall(point: Vec2, wall: WallSegment) -> f32 {
    wall_outward_normal(wall).dot(point - wall.left.0)
}

fn segment_projection(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= POSITION_EPSILON {
        return 0.0;
    }

    (point - start).dot(segment) / length_squared
}

fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let orientation =
        |p: Vec2, q: Vec2, r: Vec2| (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);

    let o1 = orientation(a1, a2, b1);
    let o2 = orientation(a1, a2, b2);
    let o3 = orientation(b1, b2, a1);
    let o4 = orientation(b1, b2, a2);

    if o1.abs() <= POSITION_EPSILON && point_on_segment(b1, a1, a2) {
        return true;
    }
    if o2.abs() <= POSITION_EPSILON && point_on_segment(b2, a1, a2) {
        return true;
    }
    if o3.abs() <= POSITION_EPSILON && point_on_segment(a1, b1, b2) {
        return true;
    }
    if o4.abs() <= POSITION_EPSILON && point_on_segment(a2, b1, b2) {
        return true;
    }

    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

fn point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> bool {
    let segment = end - start;
    let point_delta = point - start;
    let cross = segment.perp_dot(point_delta).abs();
    if cross > POSITION_EPSILON {
        return false;
    }

    let dot = point_delta.dot(segment);
    if dot < -POSITION_EPSILON {
        return false;
    }

    let squared_length = segment.length_squared();
    if dot > squared_length + POSITION_EPSILON {
        return false;
    }

    true
}

fn position_on_portal_boundary(
    sector: &Sector,
    position: Position3,
    portal_sector_id: SectorId,
) -> bool {
    let point = position.truncate().0;
    sector
        .wall_segments_iter()
        .filter(|wall| wall.portal_walkable && wall.portal_sector == Some(portal_sector_id))
        .any(|wall| point_on_segment(point, wall.left.0, wall.right.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{apply_player_look, Direction, PlayerInput},
        map::{map_to_sectors, SectorMap},
        Length, Position2, RawColor, CEILING_COLOR, FLOOR_COLOR,
    };
    use bevy::{
        app::App,
        ecs::system::{Query, Res},
        math::{vec2, vec3},
        prelude::{KeyCode, Update},
    };

    fn sector(
        id: u32,
        vertices: &[(f32, f32)],
        portal_sectors: &[Option<u32>],
        colors: &[[u8; 3]],
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
            colors: colors.iter().copied().map(RawColor).collect(),
            portal_upper_colors: vec![None; vertices.len()],
            portal_lower_colors: vec![None; vertices.len()],
            floor: Length(floor),
            ceil: Length(ceil),
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            no_ceiling: false,
            sky_color: None,
        }
    }

    fn simple_room() -> Sector {
        sector(
            0,
            &[(-4.0, 4.0), (4.0, 4.0), (4.0, -4.0), (-4.0, -4.0)],
            &[None, None, None, None],
            &[[250, 0, 0], [0, 250, 0], [0, 0, 250], [250, 250, 0]],
            0.0,
            3.2,
        )
    }

    fn portal_pair(low_floor: f32, high_floor: f32) -> Vec<Sector> {
        vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                &[[255, 0, 255], [0, 255, 0], [0, 255, 0], [0, 255, 0]],
                low_floor,
                3.2 + low_floor,
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
                &[[255, 0, 255], [0, 255, 0], [0, 0, 255], [0, 255, 0]],
                high_floor,
                3.2 + high_floor,
            ),
        ]
    }

    fn crouch_tunnel_chain() -> Vec<Sector> {
        vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                &[[255, 0, 255], [0, 255, 0], [0, 255, 0], [0, 255, 0]],
                0.0,
                3.2,
            ),
            sector(
                1,
                &[(-1.0, 3.0), (1.0, 3.0), (1.0, 1.0), (-1.0, 1.0)],
                &[Some(2), None, Some(0), None],
                &[
                    [255, 0, 255],
                    [120, 120, 120],
                    [255, 0, 255],
                    [120, 120, 120],
                ],
                0.0,
                1.35,
            ),
            sector(
                2,
                &[(-1.0, 5.0), (1.0, 5.0), (1.0, 3.0), (-1.0, 3.0)],
                &[None, None, Some(1), None],
                &[[0, 255, 255], [0, 255, 255], [255, 0, 255], [0, 255, 255]],
                0.0,
                3.2,
            ),
        ]
    }

    fn default_map_sectors() -> (SectorMap, Vec<Sector>) {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        (map, sectors)
    }

    fn sector_centroid(sector: &Sector) -> Vec2 {
        sector
            .vertices
            .iter()
            .fold(Vec2::ZERO, |sum, vertex| sum + vertex.0)
            / sector.vertices.len() as f32
    }

    fn direction_toward(from: Vec2, to: Vec2) -> Direction {
        let delta = (to - from).normalize();
        Direction((-delta.x).atan2(delta.y))
    }

    fn simulate_forward_steps(
        sectors: &[Sector],
        start_sector: SectorId,
        start: Vec2,
        feet_z: f32,
        direction: Direction,
        steps: usize,
    ) -> Player {
        let mut player = Player {
            current_sector: Some(start_sector),
            position: Position3(vec3(start.x, start.y, feet_z)),
            direction,
            grounded: true,
            ..Player::default()
        };

        for _ in 0..steps {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        player
    }

    #[test]
    fn sector_contains_player_checks_polygon_and_height() {
        let sector = simple_room();
        assert!(sector_contains_player(
            &sector,
            Position3(vec3(0.0, 0.0, 0.0))
        ));
        assert!(!sector_contains_player(
            &sector,
            Position3(vec3(20.0, 0.0, 0.0))
        ));
        assert!(!sector_contains_player(
            &sector,
            Position3(vec3(0.0, 0.0, 2.0))
        ));
    }

    #[test]
    fn boundary_points_count_as_inside_sector() {
        let sectors = portal_pair(0.0, 0.2);
        assert!(sector_contains_player(
            &sectors[0],
            Position3(vec3(0.0, 1.0, 0.0))
        ));
        assert!(sector_contains_player(
            &sectors[1],
            Position3(vec3(0.0, 1.0, 0.2))
        ));
    }

    #[test]
    fn resolve_current_sector_prefers_adjacent_portal_sector() {
        let sectors = portal_pair(0.0, 0.2);
        let resolved =
            resolve_current_sector(Position3(vec3(0.0, 2.0, 0.2)), Some(SectorId(0)), &sectors);
        assert_eq!(resolved, Some(SectorId(1)));
    }

    #[test]
    fn resolve_current_sector_switches_on_shared_portal_boundary() {
        let sectors = portal_pair(0.0, 0.2);
        let resolved =
            resolve_current_sector(Position3(vec3(0.0, 1.0, 0.2)), Some(SectorId(0)), &sectors);
        assert_eq!(resolved, Some(SectorId(1)));
    }

    #[test]
    fn resolve_current_sector_returns_none_when_outside_all_geometry() {
        let sectors = [simple_room()];
        let resolved =
            resolve_current_sector(Position3(vec3(5.0, 0.0, 0.0)), Some(SectorId(0)), &sectors);
        assert_eq!(resolved, None);
    }

    #[test]
    fn player_jump_uses_earth_gravity_and_lands_back_on_floor() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            ..Player::default()
        };

        simulate_player(
            &mut player,
            PlayerInput {
                jump_pressed: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            &sectors,
        );

        let mut peak = player.position.0.z;
        for _ in 0..240 {
            simulate_player(&mut player, PlayerInput::default(), 1.0 / 60.0, &sectors);
            peak = peak.max(player.position.0.z);
        }

        assert!(peak > 0.3);
        assert!(player.grounded);
        assert!((player.position.0.z - 0.0).abs() < 0.0001);
    }

    #[test]
    fn crouching_blocks_jump_until_player_stands() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            crouching: true,
            ..Player::default()
        };

        simulate_player(
            &mut player,
            PlayerInput {
                jump_pressed: true,
                crouch_pressed: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            &sectors,
        );

        assert!(player.grounded);
        assert_eq!(player.velocity.z, 0.0);
        assert_eq!(player.position.0.z, 0.0);
    }

    #[test]
    fn midair_crouch_keeps_eye_height_fixed_by_lifting_feet() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.0, 0.5)),
            grounded: false,
            ..Player::default()
        };
        let eye_before = player.eye_position().0.z;

        simulate_player(
            &mut player,
            PlayerInput {
                crouch_pressed: true,
                ..PlayerInput::default()
            },
            0.0,
            &sectors,
        );

        assert!(player.crouching);
        assert!((player.eye_position().0.z - eye_before).abs() < 0.0001);
        assert!((player.position.0.z - (0.5 + AIR_CROUCH_FEET_LIFT)).abs() < 0.0001);
    }

    #[test]
    fn player_collides_with_solid_wall() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(3.5, 0.0, 0.0)),
            direction: Direction(-std::f32::consts::FRAC_PI_2),
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
            eprintln!(
                "player pos=({:.3},{:.3},{:.3}) sector={:?}",
                player.position.0.x,
                player.position.0.y,
                player.position.0.z,
                player.current_sector
            );
        }

        assert!(player.position.0.x < 3.71);
    }

    #[test]
    fn noclip_passes_through_solid_wall_without_stopping() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(3.5, 0.0, 0.0)),
            direction: Direction(-std::f32::consts::FRAC_PI_2),
            noclip: true,
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert!(player.position.0.x > 4.3);
    }

    #[test]
    fn noclip_keeps_player_above_ceiling_without_vertical_clamp() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.0, 4.5)),
            noclip: true,
            ..Player::default()
        };

        simulate_player(&mut player, PlayerInput::default(), 1.0 / 60.0, &sectors);

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!((player.position.0.z - 4.5).abs() < 0.0001);
        assert!(!player.grounded);
        assert_eq!(resolve_player_sector(&player, &sectors), Some(SectorId(0)));
    }

    #[test]
    fn fly_mode_hovers_without_gravity_and_still_hits_walls() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(3.5, 0.0, 1.0)),
            direction: Direction(-std::f32::consts::FRAC_PI_2),
            fly_mode: true,
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert!(player.position.0.x < 3.71);
        assert!((player.position.0.z - 1.0).abs() < 0.0001);
    }

    #[test]
    fn fly_mode_vertical_controls_clip_to_ceiling_and_floor() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.0, 0.5)),
            fly_mode: true,
            ..Player::default()
        };

        for _ in 0..80 {
            simulate_player(
                &mut player,
                PlayerInput {
                    ascend: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        let max_feet_z = sectors[0].ceil.0 - player.height();
        assert!((player.position.0.z - max_feet_z).abs() < 0.0001);

        for _ in 0..80 {
            simulate_player(
                &mut player,
                PlayerInput {
                    descend: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert!((player.position.0.z - sectors[0].floor.0).abs() < 0.0001);
    }

    #[test]
    fn fly_mode_with_noclip_moves_past_ceiling() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.0, 0.5)),
            noclip: true,
            fly_mode: true,
            ..Player::default()
        };

        for _ in 0..40 {
            simulate_player(
                &mut player,
                PlayerInput {
                    ascend: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert!(player.position.0.z > sectors[0].ceil.0 - player.height() + 0.5);
    }

    #[test]
    fn player_slides_along_wall_when_moving_diagonally() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(3.5, 0.0, 0.0)),
            direction: Direction(-std::f32::consts::FRAC_PI_2),
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    strafe_left: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert!(player.position.0.x < 3.71);
        assert!(player.position.0.y > 0.2);
    }

    #[test]
    fn player_cannot_escape_through_convex_corner() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(3.5, 3.5, 0.0)),
            direction: Direction(-std::f32::consts::FRAC_PI_4),
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(sector_contains_player(&sectors[0], player.position));
        assert!(player.position.0.x <= 4.0 - PLAYER_RADIUS_METERS + 0.01);
        assert!(player.position.0.y <= 4.0 - PLAYER_RADIUS_METERS + 0.01);
    }

    #[test]
    fn player_can_reenter_room_through_backside_when_out_of_bounds() {
        let sectors = [simple_room()];
        let mut player = Player {
            current_sector: None,
            position: Position3(vec3(4.2, 0.0, 0.0)),
            direction: Direction(std::f32::consts::FRAC_PI_2),
            ..Player::default()
        };

        for _ in 0..10 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(sector_contains_player(&sectors[0], player.position));
        assert!(player.position.0.x < 4.0 - PLAYER_RADIUS_METERS + 0.01);
    }

    #[test]
    fn low_portal_step_is_walkable_and_raises_player() {
        let sectors = portal_pair(0.0, 0.3);
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.8, 0.0)),
            ..Player::default()
        };

        for _ in 0..10 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(1)));
        assert!((player.position.0.z - 0.3).abs() < 0.0001);
    }

    #[test]
    fn high_portal_step_behaves_like_solid_wall() {
        let sectors = portal_pair(0.0, 0.8);
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.8, 0.0)),
            ..Player::default()
        };

        for _ in 0..10 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(player.position.0.y < 1.0);
    }

    #[test]
    fn non_walkable_portal_behaves_like_window() {
        let mut sectors = portal_pair(0.0, 0.0);
        sectors[0].portal_walkable[0] = false;
        sectors[1].portal_walkable[0] = false;
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.8, 0.0)),
            ..Player::default()
        };

        for _ in 0..10 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(player.position.0.y < 1.0);
    }

    #[test]
    fn resolve_current_sector_does_not_switch_across_non_walkable_portal() {
        let mut sectors = portal_pair(0.0, 0.2);
        sectors[0].portal_walkable[0] = false;
        sectors[1].portal_walkable[0] = false;

        let resolved =
            resolve_current_sector(Position3(vec3(0.0, 1.0, 0.2)), Some(SectorId(0)), &sectors);

        assert_eq!(resolved, Some(SectorId(0)));
    }

    #[test]
    fn descending_step_snaps_to_lower_floor_without_airborne_frame() {
        let sectors = portal_pair(0.0, 0.3);
        let mut player = Player {
            current_sector: Some(SectorId(1)),
            position: Position3(vec3(0.0, 1.02, 0.3)),
            direction: Direction(std::f32::consts::PI),
            ..Player::default()
        };

        simulate_player(
            &mut player,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            &sectors,
        );

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!((player.position.0.z - 0.0).abs() < 0.0001);
        assert!(player.grounded);
        assert_eq!(player.velocity.z, 0.0);
    }

    #[test]
    fn player_slides_through_portal_boundary_without_sticking() {
        let sectors = portal_pair(0.0, 0.0);
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.72, 0.8, 0.0)),
            direction: Direction(0.0),
            ..Player::default()
        };

        for _ in 0..20 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    strafe_right: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(1)));
        assert!(
            player.position.0.y > 1.0,
            "expected player to slide through portal"
        );
    }

    #[test]
    fn standing_player_cannot_enter_low_crouch_tunnel() {
        let sectors = crouch_tunnel_chain();
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.5, 0.0)),
            ..Player::default()
        };

        for _ in 0..30 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(player.position.0.y < 1.0);
    }

    #[test]
    fn player_stays_crouched_until_exiting_low_tunnel() {
        let sectors = crouch_tunnel_chain();
        let mut player = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.5, 0.0)),
            ..Player::default()
        };

        for _ in 0..30 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    crouch_pressed: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(1)));
        assert!(player.crouching);

        for _ in 0..45 {
            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(2)));
        assert!(!player.crouching);
    }

    #[test]
    fn midair_crouch_jump_reaches_higher_portal_than_standing_jump() {
        let sectors = portal_pair(0.0, 0.75);
        let mut standing_jump = Player {
            current_sector: Some(SectorId(0)),
            position: Position3(vec3(0.0, 0.55, 0.0)),
            grounded: true,
            ..Player::default()
        };
        let mut crouch_jump = standing_jump;

        simulate_player(
            &mut standing_jump,
            PlayerInput {
                forward: true,
                jump_pressed: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            &sectors,
        );
        simulate_player(
            &mut crouch_jump,
            PlayerInput {
                forward: true,
                jump_pressed: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            &sectors,
        );

        for frame in 0..45 {
            simulate_player(
                &mut standing_jump,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
            simulate_player(
                &mut crouch_jump,
                PlayerInput {
                    forward: true,
                    crouch_pressed: frame >= 2,
                    ..PlayerInput::default()
                },
                1.0 / 60.0,
                &sectors,
            );
        }

        assert_eq!(standing_jump.current_sector, Some(SectorId(0)));
        assert_eq!(crouch_jump.current_sector, Some(SectorId(1)));
        assert!(crouch_jump.position.0.z >= 0.75 - 0.0001);
    }

    #[test]
    fn headless_app_can_move_player_between_sectors() {
        fn headless_step_system(
            keys: Res<bevy::input::ButtonInput<KeyCode>>,
            mut player_query: Query<&mut Player>,
            sector_query: Query<&Sector>,
        ) {
            let mut player = player_query.single_mut().unwrap();
            let input = PlayerInput::from_keys(&keys, keys.just_pressed(KeyCode::Space));
            apply_player_look(&mut player, input);
            let sectors = sector_query.iter().cloned().collect::<Vec<_>>();
            simulate_player(&mut player, input, 1.0 / 60.0, &sectors);
        }

        let mut app = App::new();
        app.insert_resource(bevy::input::ButtonInput::<KeyCode>::default());
        app.add_systems(Update, headless_step_system);

        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        app.world_mut().spawn(player);
        for sector in portal_pair(0.0, 0.2) {
            app.world_mut().spawn(sector);
        }

        app.world_mut()
            .resource_mut::<bevy::input::ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        for _ in 0..30 {
            app.update();
        }

        let world = app.world_mut();
        let mut query = world.query::<&Player>();
        let moved_player = *query.single(world).unwrap();
        assert!(moved_player.position.0.y > 1.0);
        assert_eq!(moved_player.current_sector, Some(SectorId(1)));
    }

    #[test]
    fn default_map_spawn_stays_stable_without_input() {
        let (map, sectors) = default_map_sectors();
        let initial_sector = crate::InitialSector(SectorId(map.initial_sector as u32));
        let initial_floor = sectors
            .iter()
            .find(|sector| sector.id == initial_sector.0)
            .unwrap()
            .floor
            .0;
        let mut player = Player::default();
        player.position = Position3(vec3(
            map.initial_position.0,
            map.initial_position.1,
            initial_floor,
        ));
        player.direction.0 = map.initial_direction_radians();
        player.current_sector = Some(initial_sector.0);

        for _ in 0..120 {
            simulate_player(&mut player, PlayerInput::default(), 1.0 / 60.0, &sectors);
        }

        assert_eq!(player.current_sector, Some(initial_sector.0));
        assert!((player.position.0.x - map.initial_position.0).abs() < 0.001);
        assert!((player.position.0.y - map.initial_position.1).abs() < 0.001);
        assert!((player.position.0.z - initial_floor).abs() < 0.001);
    }

    #[test]
    fn default_map_walkable_portals_allow_bidirectional_crossing() {
        let (_, sectors) = default_map_sectors();
        let offset = PLAYER_RADIUS_METERS + 0.05;
        let mut checked_pairs = 0;

        for source_sector in &sectors {
            let source_centroid = sector_centroid(source_sector);
            for wall in source_sector.wall_segments() {
                if !wall.portal_walkable {
                    continue;
                }
                let Some(target_sector_id) = wall.portal_sector else {
                    continue;
                };
                if source_sector.id.0 >= target_sector_id.0 {
                    continue;
                }

                let target_sector = sectors
                    .iter()
                    .find(|sector| sector.id == target_sector_id)
                    .unwrap();
                let midpoint = (wall.left.0 + wall.right.0) * 0.5;
                let source_start = midpoint + (source_centroid - midpoint).normalize() * offset;
                let target_centroid = sector_centroid(target_sector);
                let target_start = midpoint + (target_centroid - midpoint).normalize() * offset;

                let from_source = Player {
                    position: Position3(vec3(
                        source_start.x,
                        source_start.y,
                        source_sector.floor.0,
                    )),
                    grounded: true,
                    ..Player::default()
                };
                let from_target = Player {
                    position: Position3(vec3(
                        target_start.x,
                        target_start.y,
                        target_sector.floor.0,
                    )),
                    grounded: true,
                    ..Player::default()
                };

                if portal_clearance(&from_source, target_sector).is_none()
                    || portal_clearance(&from_target, source_sector).is_none()
                {
                    continue;
                }

                checked_pairs += 1;

                let toward_target = simulate_forward_steps(
                    &sectors,
                    source_sector.id,
                    source_start,
                    source_sector.floor.0,
                    direction_toward(source_start, target_centroid),
                    30,
                );
                assert_eq!(
                    toward_target.current_sector,
                    Some(target_sector.id),
                    "expected portal {:?}->{:?} to be walkable from source",
                    source_sector.id,
                    target_sector.id
                );
                assert!(
                    sector_contains_player(target_sector, toward_target.position),
                    "player should finish inside target sector {:?}",
                    target_sector.id
                );

                let toward_source = simulate_forward_steps(
                    &sectors,
                    target_sector.id,
                    target_start,
                    target_sector.floor.0,
                    direction_toward(target_start, source_centroid),
                    30,
                );
                assert_eq!(
                    toward_source.current_sector,
                    Some(source_sector.id),
                    "expected portal {:?}->{:?} to be walkable from target",
                    target_sector.id,
                    source_sector.id
                );
                assert!(
                    sector_contains_player(source_sector, toward_source.position),
                    "player should finish inside source sector {:?}",
                    source_sector.id
                );
            }
        }

        assert!(checked_pairs > 0);
    }
}
