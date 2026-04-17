mod automap;
mod frame;
mod math;
mod world;

use crate::{Position2, Position3, Sector, SectorId};

use bevy::{math::vec2, prelude::*};
use lazy_static::lazy_static;

pub use frame::{clear_frame, FrameBuffer, Pixel, FRAME_BYTES};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub const WINDOW_SCALE: u32 = 4;

pub(crate) const NEAR: f32 = 0.1;
pub(crate) const FAR: f32 = 50.0;
pub(crate) const SHADE_FAR: f32 = 20.0;
pub(crate) const SHADE_BANDS: usize = 16;
pub(crate) const BRIGHTNESS_NEAR: f32 = 1.0;
pub(crate) const BRIGHTNESS_FAR: f32 = 0.35;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const AUTOMAP_SCALE: f32 = 8.0;

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    pub(crate) static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_reverse_rh(*FOV_Y_RADIANS, ASPECT_RATIO, NEAR);
    pub(crate) static ref TAN_FAC_FOV_Y_2: f32 = (*FOV_Y_RADIANS / 2.0).tan();
    static ref TAN_FAC_FOV_X_2: f32 = (FOV_X_RADIANS / 2.0).tan();
    pub(crate) static ref X_NEAR: f32 = NEAR * *TAN_FAC_FOV_X_2;
    static ref X_FAR: f32 = FAR * *TAN_FAC_FOV_X_2;
    pub(crate) static ref BACK_CLIP_1: Vec2 = vec2(*X_NEAR, NEAR);
    pub(crate) static ref BACK_CLIP_2: Vec2 = vec2(-*X_NEAR, NEAR);
    pub(crate) static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_2;
    pub(crate) static ref LEFT_CLIP_2: Vec2 = vec2(-*X_FAR, FAR);
    pub(crate) static ref RIGHT_CLIP_1: Vec2 = vec2(*X_FAR, FAR);
    pub(crate) static ref RIGHT_CLIP_2: Vec2 = *BACK_CLIP_1;
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Normalized(pub Vec3);

impl From<Normalized> for Pixel {
    fn from(norm: Normalized) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * norm.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * norm.0.y).round() as isize,
        }
    }
}

impl From<Position2> for Pixel {
    fn from(position: Position2) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (AUTOMAP_SCALE * position.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (AUTOMAP_SCALE * position.0.y).round() as isize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Automap {
    Off,
    RotateFull,
    RotateVisible,
    NorthUpFull,
    NorthUpVisible,
}

impl Automap {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::RotateVisible,
            Self::RotateVisible => Self::RotateFull,
            Self::RotateFull => Self::NorthUpVisible,
            Self::NorthUpVisible => Self::NorthUpFull,
            Self::NorthUpFull => Self::Off,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RenderView {
    pub position: Position3,
    pub direction: f32,
    pub current_sector: Option<SectorId>,
}

impl RenderView {
    pub fn new(position: Position3, direction: f32, current_sector: Option<SectorId>) -> Self {
        Self {
            position,
            direction,
            current_sector,
        }
    }
}

pub fn render_frame<'a>(
    frame: &mut [u8],
    view: &RenderView,
    sectors: impl IntoIterator<Item = &'a Sector>,
    automap: Automap,
) {
    clear_frame(frame);
    let sectors: Vec<_> = sectors.into_iter().collect();
    world::render_world(frame, view, &sectors);
    automap::render_automap(frame, view, &sectors, automap);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{
            player_render_view, resolve_current_sector, simulate_player, Direction, Player,
            PlayerInput,
        },
        map::{map_to_sectors, SectorMap},
        Length, RawColor,
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
            colors: colors.iter().copied().map(RawColor).collect(),
            floor: Length(floor),
            ceil: Length(ceil),
        }
    }

    fn room_with_front_wall(front_y: f32) -> Sector {
        sector(
            0,
            &[(-6.0, front_y), (6.0, front_y), (6.0, -10.0), (-6.0, -10.0)],
            &[None, None, None, None],
            &[[250, 0, 0], [0, 250, 0], [0, 0, 250], [250, 250, 0]],
            0.0,
            4.0,
        )
    }

    fn room_with_split_front_wall(colors: [[u8; 3]; 5]) -> Sector {
        sector(
            0,
            &[
                (-6.0, 10.0),
                (0.0, 10.0),
                (6.0, 10.0),
                (6.0, -10.0),
                (-6.0, -10.0),
            ],
            &[None, None, None, None, None],
            &colors,
            0.0,
            4.0,
        )
    }

    fn connected_portal_sectors() -> Vec<Sector> {
        vec![
            sector(
                0,
                &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)],
                &[Some(1), None, None, None],
                &[[255, 0, 255], [0, 255, 0], [0, 255, 0], [0, 255, 0]],
                0.0,
                4.0,
            ),
            sector(
                1,
                &[(1.0, 1.0), (-1.0, 1.0), (-1.0, 4.0), (1.0, 4.0)],
                &[Some(0), None, None, None],
                &[[255, 0, 255], [0, 255, 0], [0, 0, 255], [0, 255, 0]],
                0.0,
                4.0,
            ),
        ]
    }

    fn render_connected_boundary_frame(
        y: f32,
        direction: f32,
        initial_sector: SectorId,
        resolve_sector: bool,
    ) -> FrameBuffer {
        let sectors = connected_portal_sectors();
        let mut player = Player {
            position: Position3(Vec3::new(0.0, y, 0.0)),
            direction: Direction(direction),
            current_sector: Some(initial_sector),
            ..Player::default()
        };
        if resolve_sector {
            player.current_sector =
                resolve_current_sector(player.position, player.current_sector, sectors.iter());
        }
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        frame
    }

    fn assert_no_fully_black_columns(frame: &FrameBuffer) {
        let black_columns = (24..WIDTH as usize - 24)
            .filter(|&x| (16..HEIGHT as usize - 16).all(|y| frame.pixel(x, y) == [0, 0, 0, 255]))
            .collect::<Vec<_>>();
        assert!(
            black_columns.is_empty(),
            "expected no fully black interior columns, found {:?}",
            black_columns
        );
    }

    fn longest_black_run_on_center_row(frame: &FrameBuffer) -> usize {
        let mut longest_run = 0;
        let mut current_run = 0;
        let center_y = HEIGHT as usize / 2;
        for x in 24..WIDTH as usize - 24 {
            if frame.pixel(x, center_y) == [0, 0, 0, 255] {
                current_run += 1;
                longest_run = longest_run.max(current_run);
            } else {
                current_run = 0;
            }
        }

        longest_run
    }

    fn assert_no_long_black_run_on_center_row(frame: &FrameBuffer) {
        let longest_run = longest_black_run_on_center_row(frame);
        assert!(
            longest_run <= 3,
            "expected center row to stay filled across portal boundary, longest black run was {longest_run}"
        );
    }

    fn assert_no_adjacent_tall_black_columns(frame: &FrameBuffer) {
        let tall_black_columns = (1..WIDTH as usize - 1)
            .filter_map(|x| {
                let count = (16..HEIGHT as usize - 16)
                    .filter(|&y| frame.pixel(x, y) == [0, 0, 0, 255])
                    .count();
                (count > 12).then_some(x)
            })
            .collect::<Vec<_>>();

        assert!(
            tall_black_columns
                .windows(2)
                .all(|pair| pair[1] > pair[0] + 1),
            "expected tall black columns to stay isolated, found {tall_black_columns:?}"
        );
    }

    fn staircase_portal_walk_frames(direction_sign: f32) -> Vec<FrameBuffer> {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        let portal_midpoint = Vec2::new(4.0, -3.5);
        let portal_normal = Vec2::new(3.0, -4.0).normalize();
        let feet_z = if direction_sign > 0.0 { 0.0 } else { 0.2 };
        let mut player = Player {
            position: Position3(Vec3::new(
                portal_midpoint.x - direction_sign * portal_normal.x * 0.18,
                portal_midpoint.y - direction_sign * portal_normal.y * 0.18,
                feet_z,
            )),
            direction: Direction(
                (-direction_sign * portal_normal.x).atan2(direction_sign * portal_normal.y),
            ),
            current_sector: Some(if direction_sign > 0.0 {
                SectorId(0)
            } else {
                SectorId(3)
            }),
            grounded: true,
            ..Player::default()
        };

        let mut frames = Vec::new();
        for _ in 0..128 {
            let mut frame = FrameBuffer::new();
            render_frame(
                frame.as_mut_slice(),
                &player_render_view(&player),
                sectors.iter(),
                Automap::Off,
            );
            frames.push(frame);

            simulate_player(
                &mut player,
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                0.001,
                sectors.iter(),
            );
        }

        frames
    }

    #[test]
    fn render_frame_without_current_sector_still_uses_geometric_roots() {
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
    }

    #[test]
    fn render_frame_without_current_sector_stays_background_when_outside_geometry() {
        let view = RenderView::new(
            Position3(Vec3::new(40.0, 40.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255]));
    }

    #[test]
    fn render_frame_draws_ceiling_wall_and_floor_on_center_column() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        let center_x = WIDTH as usize / 2;
        let ceiling_pixel = frame.pixel(center_x, 10);
        let wall_pixel = frame.pixel(center_x, HEIGHT as usize / 2);
        let floor_pixel = frame.pixel(center_x, HEIGHT as usize - 10);

        assert_ne!(ceiling_pixel, [0, 0, 0, 255]);
        assert_ne!(wall_pixel, [0, 0, 0, 255]);
        assert_ne!(floor_pixel, [0, 0, 0, 255]);
        assert_ne!(ceiling_pixel, wall_pixel);
        assert_ne!(floor_pixel, wall_pixel);
    }

    #[test]
    fn render_frame_shades_nearer_wall_brighter_than_farther_wall() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let near_room = [room_with_front_wall(2.0)];
        let far_room = [room_with_front_wall(10.0)];
        let mut near_frame = FrameBuffer::new();
        let mut far_frame = FrameBuffer::new();

        render_frame(
            near_frame.as_mut_slice(),
            &view,
            near_room.iter(),
            Automap::Off,
        );
        render_frame(
            far_frame.as_mut_slice(),
            &view,
            far_room.iter(),
            Automap::Off,
        );

        let center_x = WIDTH as usize / 2;
        let center_y = HEIGHT as usize / 2;
        assert!(near_frame.pixel(center_x, center_y)[0] > far_frame.pixel(center_x, center_y)[0]);
    }

    #[test]
    fn render_frame_shades_floor_and_ceiling_by_distance() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let near_room = [room_with_front_wall(2.0)];
        let far_room = [room_with_front_wall(10.0)];
        let mut near_frame = FrameBuffer::new();
        let mut far_frame = FrameBuffer::new();

        render_frame(
            near_frame.as_mut_slice(),
            &view,
            near_room.iter(),
            Automap::Off,
        );
        render_frame(
            far_frame.as_mut_slice(),
            &view,
            far_room.iter(),
            Automap::Off,
        );

        let center_x = WIDTH as usize / 2;
        assert!(near_frame.pixel(center_x, 10)[0] > far_frame.pixel(center_x, 10)[0]);
        assert!(
            near_frame.pixel(center_x, HEIGHT as usize - 10)[0]
                > far_frame.pixel(center_x, HEIGHT as usize - 10)[0]
        );
    }

    #[test]
    fn render_frame_draws_floor_and_ceiling_in_horizontal_bands() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        let sample_xs = [48_usize, WIDTH as usize / 2, WIDTH as usize - 48];
        let ceiling_samples = sample_xs.map(|x| frame.pixel(x, 20));
        let floor_samples = sample_xs.map(|x| frame.pixel(x, HEIGHT as usize - 20));

        assert!(ceiling_samples
            .into_iter()
            .all(|pixel| pixel != [0, 0, 0, 255]));
        assert!(floor_samples
            .into_iter()
            .all(|pixel| pixel != [0, 0, 0, 255]));

        assert_eq!(ceiling_samples[0], ceiling_samples[1]);
        assert_eq!(ceiling_samples[1], ceiling_samples[2]);
        assert_eq!(floor_samples[0], floor_samples[1]);
        assert_eq!(floor_samples[1], floor_samples[2]);
    }

    #[test]
    fn render_frame_skips_vertical_outline_between_same_color_walls() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let sectors = [room_with_split_front_wall([
            [220, 80, 80],
            [220, 80, 80],
            [120, 120, 180],
            [120, 120, 180],
            [120, 120, 180],
        ])];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        let center_x = WIDTH as usize / 2;
        let center_y = HEIGHT as usize / 2;
        assert_ne!(frame.pixel(center_x, center_y), [0, 0, 0, 255]);
    }

    #[test]
    fn render_frame_keeps_vertical_outline_between_different_color_walls() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let sectors = [room_with_split_front_wall([
            [220, 80, 80],
            [80, 220, 80],
            [120, 120, 180],
            [120, 120, 180],
            [120, 120, 180],
        ])];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        let center_x = WIDTH as usize / 2;
        let center_y = HEIGHT as usize / 2;
        assert!(
            ((center_x - 1)..=(center_x + 1)).any(|x| frame.pixel(x, center_y) == [0, 0, 0, 255])
        );
    }

    #[test]
    fn render_frame_north_up_automap_draws_player_and_frustum() {
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(
            frame.as_mut_slice(),
            &view,
            sectors.iter(),
            Automap::NorthUpFull,
        );

        assert_eq!(
            frame.pixel(WIDTH as usize / 2, HEIGHT as usize / 2),
            [
                crate::PLAYER_COLOR.0[0],
                crate::PLAYER_COLOR.0[1],
                crate::PLAYER_COLOR.0[2],
                255
            ]
        );
        assert!(frame.count_color(*crate::FRUSTUM_COLOR) > 0);
    }

    #[test]
    fn automap_mode_cycle_swaps_visible_and_full_order() {
        let mut mode = Automap::Off;
        let mut seen = Vec::new();

        for _ in 0..5 {
            mode = mode.next();
            seen.push(mode);
        }

        assert_eq!(
            seen,
            vec![
                Automap::RotateVisible,
                Automap::RotateFull,
                Automap::NorthUpVisible,
                Automap::NorthUpFull,
                Automap::Off,
            ]
        );
    }

    #[test]
    fn boundary_transition_keeps_non_black_frame() {
        let sectors = connected_portal_sectors();
        let mut player = Player::default();
        player.position = Position3(Vec3::new(0.0, 1.0, 0.2));
        player.current_sector = Some(SectorId(0));
        player.current_sector =
            resolve_current_sector(player.position, player.current_sector, sectors.iter());
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
    }

    #[test]
    fn portal_boundary_views_parallel_to_shared_wall_keep_columns_filled() {
        for (y, initial_sector) in [
            (0.99_f32, SectorId(0)),
            (1.0_f32, SectorId(0)),
            (1.01_f32, SectorId(1)),
        ] {
            for direction in [
                -std::f32::consts::FRAC_PI_2 + 0.05,
                std::f32::consts::FRAC_PI_2 - 0.05,
            ] {
                let frame = render_connected_boundary_frame(y, direction, initial_sector, true);
                assert_no_fully_black_columns(&frame);
            }
        }
    }

    #[test]
    fn portal_boundary_views_perpendicular_to_shared_wall_keep_rows_filled() {
        for initial_sector in [SectorId(0), SectorId(1)] {
            for direction in [0.0_f32, std::f32::consts::PI] {
                let frame = render_connected_boundary_frame(1.0, direction, initial_sector, false);
                assert_no_fully_black_columns(&frame);
                assert_no_long_black_run_on_center_row(&frame);
            }
        }
    }

    #[test]
    fn stale_current_sector_near_portal_boundary_keeps_rows_filled() {
        for (y, initial_sector, direction) in [
            (0.99_f32, SectorId(1), 0.0_f32),
            (1.01_f32, SectorId(0), 0.0_f32),
            (0.99_f32, SectorId(1), std::f32::consts::PI),
            (1.01_f32, SectorId(0), std::f32::consts::PI),
        ] {
            let frame = render_connected_boundary_frame(y, direction, initial_sector, false);
            assert_no_fully_black_columns(&frame);
            assert_no_long_black_run_on_center_row(&frame);
        }
    }

    #[test]
    fn staircase_portal_walk_frames_stay_filled_in_both_directions() {
        for direction_sign in [1.0_f32, -1.0_f32] {
            for (index, frame) in staircase_portal_walk_frames(direction_sign)
                .into_iter()
                .enumerate()
            {
                let black_columns = (24..WIDTH as usize - 24)
                    .filter(|&x| {
                        (16..HEIGHT as usize - 16).all(|y| frame.pixel(x, y) == [0, 0, 0, 255])
                    })
                    .collect::<Vec<_>>();
                assert!(
                    black_columns.is_empty(),
                    "expected staircase portal walk frame {index} direction_sign {direction_sign} to avoid fully black columns, found {black_columns:?}"
                );
                let tall_black_columns = (1..WIDTH as usize - 1)
                    .filter_map(|x| {
                        let count = (16..HEIGHT as usize - 16)
                            .filter(|&y| frame.pixel(x, y) == [0, 0, 0, 255])
                            .count();
                        (count > 12).then_some(x)
                    })
                    .collect::<Vec<_>>();
                assert!(
                    tall_black_columns
                        .windows(2)
                        .all(|pair| pair[1] > pair[0] + 1),
                    "expected staircase portal walk frame {index} direction_sign {direction_sign} to keep tall black columns isolated, found {tall_black_columns:?}"
                );
            }
        }
    }

    #[test]
    fn default_map_initial_view_renders_non_black_frame() {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (initial_sector, sectors) = map_to_sectors(&map).unwrap();
        let initial_floor = sectors
            .iter()
            .find(|sector| sector.id == initial_sector.0)
            .unwrap()
            .floor
            .0;
        let mut player = Player::default();
        player.position = Position3(Vec3::new(
            map.initial_position.0,
            map.initial_position.1,
            initial_floor,
        ));
        player.direction.0 = map.initial_direction_radians();
        player.current_sector = Some(initial_sector.0);
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
    }

    #[test]
    fn brown_sector_view_back_to_spawn_keeps_portal_edges_single_column() {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        let mut player = Player::default();
        player.position = Position3(Vec3::new(5.5, -4.25, 0.2));
        player.direction.0 = 6.5_f32.atan2(4.25);
        player.current_sector = Some(SectorId(3));
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Automap::Off);

        assert_no_adjacent_tall_black_columns(&frame);
    }
}
