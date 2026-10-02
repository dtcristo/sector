#![cfg(feature = "sector")]

use bevy::math::{vec2, vec3, Vec2};
use sector::{
    game::{player_render_view, simulate_player, Player, PlayerInput},
    map::{load_map_from_path, map_to_sectors},
    render::{render_frame, Automap, FrameBuffer},
    Position3, Sector, SectorId,
};

fn demo() -> Vec<Sector> {
    let map = load_map_from_path("assets/maps/default.map.ron").unwrap();
    map_to_sectors(&map).unwrap().1
}

fn walk(player: &mut Player, sectors: &[Sector], goal: Vec2, expected_sector: u32) {
    for _ in 0..600 {
        let delta = goal - player.position.0.truncate();
        if delta.length() < 0.08 {
            assert_eq!(player.current_sector, Some(SectorId(expected_sector)));
            assert!(!player.noclip && !player.fly_mode && !player.crouching);
            let mut frame = FrameBuffer::new();
            render_frame(
                frame.as_mut_slice(),
                &player_render_view(player),
                sectors,
                Automap::Off,
            );
            let mut seam_width = 0;
            for x in 24..296 {
                if (16..224).all(|y| frame.pixel(x, y)[..3] == [0, 0, 0]) {
                    seam_width += 1;
                    assert!(
                        seam_width <= 2,
                        "unfilled columns ending at {x} in sector {expected_sector}"
                    );
                } else {
                    seam_width = 0;
                }
            }
            return;
        }
        player.direction.0 = (-delta.x).atan2(delta.y);
        simulate_player(
            player,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
            1.0 / 60.0,
            sectors,
        );
    }
    panic!("blocked approaching {expected_sector} at {goal:?}, player {player:?}");
}

fn entrance() -> Player {
    let map = load_map_from_path("assets/maps/default.map.ron").unwrap();
    Player {
        position: Position3(vec3(map.initial_position.0, map.initial_position.1, 0.)),
        current_sector: Some(SectorId(0)),
        ..Player::default()
    }
}

fn skew(x: f32, y: f32) -> Vec2 {
    vec2(x + 0.22 * y, y + 0.08 * x)
}

fn route(player: &mut Player, sectors: &[Sector], points: &[(u32, f32, f32)]) {
    for &(id, x, y) in points {
        walk(player, sectors, skew(x, y), id);
    }
}

fn enter_atrium(player: &mut Player, sectors: &[Sector]) {
    route(
        player,
        sectors,
        &[(1, -0.4, 8.), (2, -1., 12.), (3, -1., 16.)],
    );
}

#[test]
fn two_turn_spiral_overlooks_and_return_loop_are_walkable() {
    let sectors = demo();
    let mut player = entrance();
    enter_atrium(&mut player, &sectors);
    route(
        &mut player,
        &sectors,
        &[
            (3, 6., 16.5),
            (45, 12., 16.5),
            (46, 17.5, 16.5),
            (46, 17.5, 25.5),
        ],
    );
    for step in 0..48 {
        let angle = (step as f32 + 0.5) * std::f32::consts::TAU / 24.;
        walk(
            &mut player,
            &sectors,
            skew(14. + 3.5 * angle.cos(), 26. + 3.5 * angle.sin()),
            48 + step,
        );
    }
    route(
        &mut player,
        &sectors,
        &[
            (100, 17.5, 27.),
            (100, 17.5, 32.5),
            (101, 12., 32.5),
            (101, 8.5, 32.),
            (102, 8.5, 29.),
            (102, 8.5, 20.),
        ],
    );
    assert!((player.position.0.z - 9.6).abs() < 0.001);
    route(
        &mut player,
        &sectors,
        &[
            (102, 8.5, 30.),
            (101, 8.5, 32.),
            (101, 12., 32.5),
            (100, 17.5, 32.5),
            (100, 17.5, 26.5),
        ],
    );
    for step in (24..48).rev() {
        let angle = (step as f32 + 0.5) * std::f32::consts::TAU / 24.;
        walk(
            &mut player,
            &sectors,
            skew(14. + 3.5 * angle.cos(), 26. + 3.5 * angle.sin()),
            48 + step,
        );
    }
    route(
        &mut player,
        &sectors,
        &[
            (96, 19.8, 26.6),
            (97, 21.8, 26.6),
            (97, 21.8, 30.),
            (98, 21.8, 32.5),
            (98, 12., 32.5),
            (99, 0., 32.5),
            (99, -7.5, 32.5),
        ],
    );
    assert!((player.position.0.z - 4.8).abs() < 0.001);
    for step in 0..24 {
        walk(
            &mut player,
            &sectors,
            skew(-8. - (step as f32 + 0.5) * 0.6, 32.5),
            103 + step,
        );
    }
    route(
        &mut player,
        &sectors,
        &[
            (127, -23.9, 32.5),
            (128, -23.9, 20.),
            (128, -23.9, 15.),
            (129, -23.9, 12.),
            (129, -10., 12.),
            (130, -6., 12.),
            (2, -1.2, 12.),
            (1, -0.4, 8.),
            (0, -1.5, -1.),
        ],
    );
    assert!(player.position.0.z.abs() < 0.001);
}

#[test]
fn nested_rooms_and_their_upper_storey_are_reachable() {
    let sectors = demo();
    let mut player = entrance();
    enter_atrium(&mut player, &sectors);
    route(
        &mut player,
        &sectors,
        &[
            (3, 0., 17.),
            (7, 0., 18.1),
            (8, 0., 19.),
            (13, 0., 20.1),
            (14, 0., 22.),
            (15, 0., 23.9),
            (11, 0., 25.),
            (12, 0., 25.9),
            (6, 0., 28.),
            (5, 6., 25.),
            (5, 6., 19.),
            (3, 6., 16.),
            (3, -7., 16.),
        ],
    );
    for step in 0..18 {
        walk(
            &mut player,
            &sectors,
            skew(-7., 18. + (step as f32 + 0.5) * 0.4),
            17 + step,
        );
    }
    route(
        &mut player,
        &sectors,
        &[
            (35, -7., 25.5),
            (35, -4.5, 25.5),
            (36, -3.9, 25.5),
            (40, -3., 25.),
            (40, 0., 25.),
            (43, 0., 23.9),
            (42, 0., 22.),
            (41, 0., 20.1),
            (37, 0., 19.),
            (37, -3., 19.),
            (38, -3., 22.),
            (40, -3., 25.),
        ],
    );
    assert!((player.position.0.z - 3.6).abs() < 0.001);
}

#[test]
fn star_court_is_one_walkable_room_with_eight_petals() {
    let sectors = demo();
    let start = skew(-23.9, 23.);
    let mut player = Player {
        position: Position3(vec3(start.x, start.y, 0.)),
        current_sector: Some(SectorId(128)),
        ..Player::default()
    };
    route(
        &mut player,
        &sectors,
        &[(140, -27., 23.), (132, -30., 23.), (131, -34., 21.5)],
    );
    for petal in 0..8 {
        let angle = (petal as f32 + 0.5) * std::f32::consts::TAU / 8.;
        walk(
            &mut player,
            &sectors,
            skew(-34. + 4. * angle.cos(), 21.5 + 4. * angle.sin()),
            132 + petal,
        );
        walk(&mut player, &sectors, skew(-34., 21.5), 131);
    }
}

#[test]
fn gallery_window_shows_atrium_but_blocks_the_drop() {
    let sectors = demo();
    let start = skew(0., 30.5);
    let target = skew(0., 29.);
    let delta = target - start;
    let mut player = Player {
        position: Position3(vec3(start.x, start.y, 4.8)),
        current_sector: Some(SectorId(99)),
        ..Player::default()
    };
    player.direction.0 = (-delta.x).atan2(delta.y);
    for _ in 0..60 {
        simulate_player(
            &mut player,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
            1. / 60.,
            &sectors,
        );
    }
    assert_eq!(player.current_sector, Some(SectorId(99)));
    assert!((player.position.0.z - 4.8).abs() < 0.001);
}

#[test]
fn shortcut_requires_crouching_and_exits_into_the_atrium() {
    let sectors = demo();
    let start = skew(3., 22.5);
    let goal = skew(5., 22.5);
    let delta = goal - start;
    let mut original = Player {
        position: Position3(vec3(start.x, start.y, 0.)),
        current_sector: Some(SectorId(10)),
        ..Player::default()
    };
    original.direction.0 = (-delta.x).atan2(delta.y);
    let mut standing = original;
    for _ in 0..90 {
        simulate_player(
            &mut standing,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
            1. / 60.,
            &sectors,
        );
    }
    assert_eq!(standing.current_sector, Some(SectorId(10)));
    let mut crouching = original;
    for _ in 0..90 {
        simulate_player(
            &mut crouching,
            PlayerInput {
                forward: true,
                crouch_pressed: true,
                ..PlayerInput::default()
            },
            1. / 60.,
            &sectors,
        );
    }
    assert_eq!(crouching.current_sector, Some(SectorId(5)));
    simulate_player(&mut crouching, PlayerInput::default(), 1. / 60., &sectors);
    assert!(!crouching.crouching);
}
