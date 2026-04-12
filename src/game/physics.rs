use crate::{Position3, Sector, SectorId, WallSegment};

use bevy::math::{Vec2, Vec3};
use std::collections::HashMap;

use super::{
    desired_horizontal_velocity, jump_speed_mps, Player, PlayerInput, EARTH_GRAVITY_MPS2,
    PLAYER_HEIGHT_METERS, PLAYER_MAX_STEP_HEIGHT_METERS, PLAYER_RADIUS_METERS,
};

const POSITION_EPSILON: f32 = 0.0001;

pub fn simulate_player<'a>(
    player: &mut Player,
    input: PlayerInput,
    dt_seconds: f32,
    sectors: impl IntoIterator<Item = &'a Sector>,
) {
    let sectors: Vec<_> = sectors.into_iter().collect();
    if sectors.is_empty() {
        return;
    }

    player.current_sector = resolve_current_sector(player.position, player.current_sector, sectors.iter().copied());

    let horizontal_velocity = desired_horizontal_velocity(player, input);
    player.velocity.x = horizontal_velocity.x;
    player.velocity.y = horizontal_velocity.y;

    let movement_delta = horizontal_velocity.truncate() * dt_seconds;
    let (horizontal_position, sector_id, stepped_floor) =
        move_player_horizontally(player, movement_delta, &sectors);
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
        .and_then(|sector_id| sectors.iter().find(|sector| sector.id == sector_id).copied());

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

    if input.jump_pressed && player.grounded {
        player.velocity.z = jump_speed_mps();
        player.grounded = false;
    }

    if !player.grounded || player.velocity.z > 0.0 {
        player.velocity.z -= EARTH_GRAVITY_MPS2 * dt_seconds;
        player.position.0.z += player.velocity.z * dt_seconds;
    }

    player.current_sector = resolve_current_sector(player.position, player.current_sector, sectors.iter().copied());

    if let Some(current_sector) = player
        .current_sector
        .and_then(|sector_id| sectors.iter().find(|sector| sector.id == sector_id).copied())
    {
        let max_feet_z = current_sector.ceil.0 - PLAYER_HEIGHT_METERS;
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

pub fn resolve_current_sector<'a>(
    position: Position3,
    current_sector: Option<SectorId>,
    sectors: impl IntoIterator<Item = &'a Sector>,
) -> Option<SectorId> {
    let sectors: Vec<_> = sectors.into_iter().collect();
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, *sector)).collect();

    if let Some(current_sector_id) = current_sector {
        if let Some(sector) = sectors_by_id.get(&current_sector_id).copied() {
            if let Some(adjacent_sector_id) = sector
                .wall_segments()
                .into_iter()
                .filter_map(|wall| wall.portal_sector)
                .find(|portal_id| {
                    sectors_by_id
                        .get(portal_id)
                        .copied()
                        .is_some_and(|portal_sector| {
                            sector_contains_player(portal_sector, position)
                                && (!sector_contains_player(sector, position)
                                    || position_on_portal_boundary(sector, position, *portal_id))
                        })
                })
            {
                return Some(adjacent_sector_id);
            }

            if sector_contains_player(sector, position) {
                return Some(current_sector_id);
            }
        }
    }

    sectors
        .into_iter()
        .find(|sector| sector_contains_player(sector, position))
        .map(|sector| sector.id)
        .or(current_sector)
}

pub fn sector_contains_player(sector: &Sector, position: Position3) -> bool {
    if position.0.z < sector.floor.0 - POSITION_EPSILON {
        return false;
    }
    if position.0.z + PLAYER_HEIGHT_METERS > sector.ceil.0 + POSITION_EPSILON {
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
    sectors: &[&Sector],
) -> (Vec2, Option<SectorId>, Option<f32>) {
    let Some(current_sector_id) = player.current_sector else {
        return (player.position.truncate().0, None, None);
    };

    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, *sector)).collect();
    let mut position = player.position.truncate().0;
    let mut sector_id = current_sector_id;
    let mut remaining = desired_delta;
    let mut stepped_to_floor = None;

    for _ in 0..4 {
        if remaining.length_squared() <= POSITION_EPSILON {
            break;
        }

        let Some(sector) = sectors_by_id.get(&sector_id).copied() else {
            break;
        };
        let target = position + remaining;

        if let Some(transition) = find_portal_transition(target, player, sector, &sectors_by_id) {
            position = target;
            sector_id = transition.target_sector_id;
            if transition.step_to_floor.is_some() {
                stepped_to_floor = transition.step_to_floor;
            }
            break;
        }

        if let Some(blocking_wall) = find_blocking_wall(position, target, player, sector, &sectors_by_id)
        {
            let tangent = (blocking_wall.right.0 - blocking_wall.left.0).normalize_or_zero();
            let projected = tangent * remaining.dot(tangent);
            if projected.length_squared() >= remaining.length_squared() - POSITION_EPSILON {
                break;
            }
            remaining = projected;
            continue;
        }

        position = target;
        break;
    }

    (position, Some(sector_id), stepped_to_floor)
}

fn find_portal_transition(
    target: Vec2,
    player: &Player,
    sector: &Sector,
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> Option<PortalTransition> {
    sector
        .wall_segments()
        .into_iter()
        .filter_map(|wall| {
            let target_sector = wall.portal_sector.and_then(|sector_id| sectors_by_id.get(&sector_id).copied())?;
            portal_transition_for_wall(target, player, sector, target_sector)
        })
        .next()
}

fn find_blocking_wall(
    start: Vec2,
    target: Vec2,
    player: &Player,
    sector: &Sector,
    sectors_by_id: &HashMap<SectorId, &Sector>,
) -> Option<WallSegment> {
    sector
        .wall_segments()
        .into_iter()
        .filter(|wall| {
            if let Some(target_sector) =
                wall.portal_sector.and_then(|sector_id| sectors_by_id.get(&sector_id).copied())
            {
                if portal_clearance(player, target_sector).is_some() {
                    return false;
                }
            }

            movement_interacts_with_wall(start, target, wall)
                || distance_to_segment(target, wall.left.0, wall.right.0) < PLAYER_RADIUS_METERS - POSITION_EPSILON
        })
        .min_by(|left, right| {
            distance_to_segment(target, left.left.0, left.right.0)
                .total_cmp(&distance_to_segment(target, right.left.0, right.right.0))
        })
}

#[derive(Debug, Copy, Clone)]
struct PortalTransition {
    target_sector_id: SectorId,
    step_to_floor: Option<f32>,
}

fn portal_transition_for_wall(
    target: Vec2,
    player: &Player,
    sector: &Sector,
    target_sector: &Sector,
) -> Option<PortalTransition> {
    if !sector_contains_horizontal_point(target_sector, target)
        && !position_on_portal_boundary(
            sector,
            Position3(Vec3::new(target.x, target.y, player.position.0.z)),
            target_sector.id,
        )
    {
        return None;
    }

    let feet_z = portal_clearance(player, target_sector)?;

    Some(PortalTransition {
        target_sector_id: target_sector.id,
        step_to_floor: (feet_z > player.position.0.z + POSITION_EPSILON).then_some(feet_z),
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

    if feet_z + PLAYER_HEIGHT_METERS > target_sector.ceil.0 - POSITION_EPSILON {
        return None;
    }

    Some(feet_z)
}

fn movement_interacts_with_wall(start: Vec2, target: Vec2, wall: &WallSegment) -> bool {
    segments_intersect(start, target, wall.left.0, wall.right.0)
        || distance_to_segment(target, wall.left.0, wall.right.0) < PLAYER_RADIUS_METERS - POSITION_EPSILON
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= POSITION_EPSILON {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    let projection = start + segment * t;
    point.distance(projection)
}

fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let orientation = |p: Vec2, q: Vec2, r: Vec2| (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);

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

fn position_on_portal_boundary(sector: &Sector, position: Position3, portal_sector_id: SectorId) -> bool {
    let point = position.truncate().0;
    sector
        .wall_segments()
        .into_iter()
        .filter(|wall| wall.portal_sector == Some(portal_sector_id))
        .any(|wall| point_on_segment(point, wall.left.0, wall.right.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{apply_player_look, Direction, PlayerInput},
        map::{map_to_sectors, SectorMap},
        Length, Position2, RawColor,
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
            portal_sectors: portal_sectors.iter().map(|portal| portal.map(SectorId)).collect(),
            colors: colors.iter().copied().map(RawColor).collect(),
            floor: Length(floor),
            ceil: Length(ceil),
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

    #[test]
    fn sector_contains_player_checks_polygon_and_height() {
        let sector = simple_room();
        assert!(sector_contains_player(&sector, Position3(vec3(0.0, 0.0, 0.0))));
        assert!(!sector_contains_player(&sector, Position3(vec3(20.0, 0.0, 0.0))));
        assert!(!sector_contains_player(&sector, Position3(vec3(0.0, 0.0, 2.0))));
    }

    #[test]
    fn boundary_points_count_as_inside_sector() {
        let sectors = portal_pair(0.0, 0.2);
        assert!(sector_contains_player(&sectors[0], Position3(vec3(0.0, 1.0, 0.0))));
        assert!(sector_contains_player(&sectors[1], Position3(vec3(0.0, 1.0, 0.2))));
    }

    #[test]
    fn resolve_current_sector_prefers_adjacent_portal_sector() {
        let sectors = portal_pair(0.0, 0.2);
        let resolved =
            resolve_current_sector(Position3(vec3(0.0, 2.0, 0.2)), Some(SectorId(0)), sectors.iter());
        assert_eq!(resolved, Some(SectorId(1)));
    }

    #[test]
    fn resolve_current_sector_switches_on_shared_portal_boundary() {
        let sectors = portal_pair(0.0, 0.2);
        let resolved =
            resolve_current_sector(Position3(vec3(0.0, 1.0, 0.2)), Some(SectorId(0)), sectors.iter());
        assert_eq!(resolved, Some(SectorId(1)));
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
            sectors.iter(),
        );

        let mut peak = player.position.0.z;
        for _ in 0..240 {
            simulate_player(&mut player, PlayerInput::default(), 1.0 / 60.0, sectors.iter());
            peak = peak.max(player.position.0.z);
        }

        assert!(peak > 0.3);
        assert!(player.grounded);
        assert!((player.position.0.z - 0.0).abs() < 0.0001);
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
                sectors.iter(),
            );
        }

        assert!(player.position.0.x < 3.71);
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
                sectors.iter(),
            );
        }

        assert!(player.position.0.x < 3.71);
        assert!(player.position.0.y > 0.2);
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
                sectors.iter(),
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
                sectors.iter(),
            );
        }

        assert_eq!(player.current_sector, Some(SectorId(0)));
        assert!(player.position.0.y < 1.0);
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
            simulate_player(&mut player, input, 1.0 / 60.0, sector_query.iter());
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
        let map =
            ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
                .unwrap();
        let (initial_sector, sectors) = map_to_sectors(&map).unwrap();
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
            simulate_player(&mut player, PlayerInput::default(), 1.0 / 60.0, sectors.iter());
        }

        assert_eq!(player.current_sector, Some(initial_sector.0));
        assert!((player.position.0.x - map.initial_position.0).abs() < 0.001);
        assert!((player.position.0.y - map.initial_position.1).abs() < 0.001);
        assert!((player.position.0.z - initial_floor).abs() < 0.001);
    }
}
