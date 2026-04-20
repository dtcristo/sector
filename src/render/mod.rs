mod automap;
mod frame;
mod math;
mod world;

use crate::{Position2, Position3, Sector, SectorId};

use bevy::{math::vec2, prelude::*};

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
const DEFAULT_ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const MIN_ASPECT_RATIO: f32 = 9.0 / 16.0;
const MAX_ASPECT_RATIO: f32 = 21.0 / 9.0;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const AUTOMAP_SCALE: f32 = 8.0;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RenderMetrics {
    pub width: u32,
    pub height: u32,
    pub(crate) perspective_matrix: Mat4,
    pub(crate) tan_fac_fov_y_2: f32,
    pub(crate) back_clip_1: Vec2,
    pub(crate) back_clip_2: Vec2,
    pub(crate) left_clip_1: Vec2,
    pub(crate) left_clip_2: Vec2,
    pub(crate) right_clip_1: Vec2,
    pub(crate) right_clip_2: Vec2,
    automap_scale: f32,
}

impl RenderMetrics {
    pub fn base() -> Self {
        Self::new(WIDTH, HEIGHT)
    }

    pub fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let aspect_ratio = width as f32 / height as f32;
        let base_tan_fac_fov_x_2 = (FOV_X_RADIANS / 2.0).tan();
        let base_tan_fac_fov_y_2 = base_tan_fac_fov_x_2 / DEFAULT_ASPECT_RATIO;
        let (tan_fac_fov_x_2, tan_fac_fov_y_2) = if aspect_ratio >= DEFAULT_ASPECT_RATIO {
            (base_tan_fac_fov_y_2 * aspect_ratio, base_tan_fac_fov_y_2)
        } else {
            (base_tan_fac_fov_x_2, base_tan_fac_fov_x_2 / aspect_ratio)
        };
        let fov_y_radians = 2.0 * tan_fac_fov_y_2.atan();
        let perspective_matrix =
            Mat4::perspective_infinite_reverse_rh(fov_y_radians, aspect_ratio, NEAR);
        let x_near = NEAR * tan_fac_fov_x_2;
        let x_far = FAR * tan_fac_fov_x_2;
        let back_clip_1 = vec2(x_near, NEAR);
        let back_clip_2 = vec2(-x_near, NEAR);

        Self {
            width,
            height,
            perspective_matrix,
            tan_fac_fov_y_2,
            back_clip_1,
            back_clip_2,
            left_clip_1: back_clip_2,
            left_clip_2: vec2(-x_far, FAR),
            right_clip_1: vec2(x_far, FAR),
            right_clip_2: back_clip_1,
            automap_scale: AUTOMAP_SCALE,
        }
    }

    pub fn from_window_size(window_width: f32, window_height: f32) -> Self {
        let window_width = window_width.max(WIDTH as f32);
        let window_height = window_height.max(HEIGHT as f32);
        let window_aspect_ratio = window_width / window_height;
        let (content_width, content_height) = if window_aspect_ratio > MAX_ASPECT_RATIO {
            (window_height * MAX_ASPECT_RATIO, window_height)
        } else if window_aspect_ratio < MIN_ASPECT_RATIO {
            (window_width, window_width / MIN_ASPECT_RATIO)
        } else {
            (window_width, window_height)
        };
        let scale = (content_width / WIDTH as f32)
            .min(content_height / HEIGHT as f32)
            .floor()
            .max(1.0);
        let width = (content_width / scale).floor().max(1.0) as u32;
        let height = (content_height / scale).floor().max(1.0) as u32;

        Self::new(width, height)
    }

    pub fn frame_bytes(self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    pub(crate) fn pixel_from_normalized(self, norm: Normalized) -> Pixel {
        let frac_width_2 = self.width as f32 * 0.5;
        let frac_height_2 = self.height as f32 * 0.5;
        Pixel {
            x: frac_width_2 as isize + (frac_width_2 * norm.0.x).round() as isize,
            y: frac_height_2 as isize - (frac_height_2 * norm.0.y).round() as isize,
        }
    }

    pub(crate) fn pixel_from_automap_position(self, position: Position2) -> Pixel {
        let frac_width_2 = self.width as f32 * 0.5;
        let frac_height_2 = self.height as f32 * 0.5;
        Pixel {
            x: frac_width_2 as isize + (self.automap_scale * position.0.x).round() as isize,
            y: frac_height_2 as isize - (self.automap_scale * position.0.y).round() as isize,
        }
    }
}

impl Default for RenderMetrics {
    fn default() -> Self {
        Self::base()
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Normalized(pub Vec3);

impl From<Normalized> for Pixel {
    fn from(norm: Normalized) -> Self {
        RenderMetrics::base().pixel_from_normalized(norm)
    }
}

impl From<Position2> for Pixel {
    fn from(position: Position2) -> Self {
        RenderMetrics::base().pixel_from_automap_position(position)
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

pub fn render_frame(frame: &mut [u8], view: &RenderView, sectors: &[Sector], automap: Automap) {
    render_frame_with_metrics(frame, &RenderMetrics::base(), view, sectors, automap);
}

pub fn render_frame_with_metrics(
    frame: &mut [u8],
    metrics: &RenderMetrics,
    view: &RenderView,
    sectors: &[Sector],
    automap: Automap,
) {
    clear_frame(frame);
    world::render_world_with_metrics(frame, metrics, view, sectors);
    automap::render_automap_with_metrics(frame, metrics, view, sectors, automap);
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
        Length, RawColor, CEILING_COLOR, FLOOR_COLOR,
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
        render_connected_boundary_frame_for_sectors(
            connected_portal_sectors(),
            y,
            direction,
            initial_sector,
            resolve_sector,
        )
    }

    fn render_connected_boundary_frame_for_sectors(
        sectors: Vec<Sector>,
        y: f32,
        direction: f32,
        initial_sector: SectorId,
        resolve_sector: bool,
    ) -> FrameBuffer {
        let mut player = Player {
            position: Position3(Vec3::new(0.0, y, 0.0)),
            direction: Direction(direction),
            current_sector: Some(initial_sector),
            ..Player::default()
        };
        if resolve_sector {
            player.current_sector =
                resolve_current_sector(player.position, player.current_sector, &sectors);
        }
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

    fn sector_centroid(sector: &Sector) -> Vec2 {
        sector
            .vertices
            .iter()
            .fold(Vec2::ZERO, |sum, vertex| sum + vertex.0)
            / sector.vertices.len() as f32
    }

    fn staircase_portal_walk_frames(direction_sign: f32) -> Vec<FrameBuffer> {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        let source_sector_id = if direction_sign > 0.0 {
            SectorId(3)
        } else {
            SectorId(4)
        };
        let target_sector_id = if direction_sign > 0.0 {
            SectorId(4)
        } else {
            SectorId(3)
        };
        let source_sector = sectors
            .iter()
            .find(|sector| sector.id == source_sector_id)
            .unwrap();
        let target_sector = sectors
            .iter()
            .find(|sector| sector.id == target_sector_id)
            .unwrap();
        let wall = source_sector
            .wall_segments()
            .into_iter()
            .find(|wall| wall.portal_sector == Some(target_sector_id))
            .unwrap();
        let portal_midpoint = (wall.left.0 + wall.right.0) * 0.5;
        let portal_normal =
            (sector_centroid(target_sector) - sector_centroid(source_sector)).normalize();
        let feet_z = source_sector.floor.0;
        let mut player = Player {
            position: Position3(Vec3::new(
                portal_midpoint.x - direction_sign * portal_normal.x * 0.18,
                portal_midpoint.y - direction_sign * portal_normal.y * 0.18,
                feet_z,
            )),
            direction: Direction(
                (-direction_sign * portal_normal.x).atan2(direction_sign * portal_normal.y),
            ),
            current_sector: Some(source_sector_id),
            grounded: true,
            ..Player::default()
        };

        let mut frames = Vec::new();
        for _ in 0..128 {
            let mut frame = FrameBuffer::new();
            render_frame(
                frame.as_mut_slice(),
                &player_render_view(&player),
                &sectors,
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
                &sectors,
            );
        }

        frames
    }

    #[test]
    fn render_metrics_clamp_extreme_window_aspects() {
        let wide = RenderMetrics::from_window_size(4000.0, 200.0);
        let tall = RenderMetrics::from_window_size(200.0, 4000.0);

        let wide_aspect = wide.width as f32 / wide.height as f32;
        let tall_aspect = tall.width as f32 / tall.height as f32;

        assert!(wide_aspect <= MAX_ASPECT_RATIO + 0.01);
        assert!(tall_aspect >= MIN_ASPECT_RATIO - 0.01);
    }

    #[test]
    fn render_metrics_show_more_between_integer_scale_steps_then_snap_back() {
        let intermediate = RenderMetrics::from_window_size(700.0, 525.0);
        let snapped = RenderMetrics::from_window_size(960.0, 720.0);

        assert!(intermediate.width > WIDTH);
        assert!(intermediate.height > HEIGHT);
        assert_eq!(snapped.width, WIDTH);
        assert_eq!(snapped.height, HEIGHT);
    }

    #[test]
    fn render_metrics_expand_fov_with_aspect_changes() {
        let base = RenderMetrics::base();
        let wide = RenderMetrics::from_window_size(840.0, 360.0);
        let tall = RenderMetrics::from_window_size(360.0, 840.0);
        let sample = Position2(vec2(1.0, 5.0));

        let base_projection = math::project_with_metrics(&base, sample, Length(0.0));
        let wide_projection = math::project_with_metrics(&wide, sample, Length(0.0));
        let tall_projection = math::project_with_metrics(&tall, sample, Length(1.0));
        let base_tall_projection = math::project_with_metrics(&base, sample, Length(1.0));

        assert!(wide_projection.0.x.abs() < base_projection.0.x.abs());
        assert!(tall_projection.0.y.abs() < base_tall_projection.0.y.abs());
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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

        render_frame(near_frame.as_mut_slice(), &view, &near_room, Automap::Off);
        render_frame(far_frame.as_mut_slice(), &view, &far_room, Automap::Off);

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

        render_frame(near_frame.as_mut_slice(), &view, &near_room, Automap::Off);
        render_frame(far_frame.as_mut_slice(), &view, &far_room, Automap::Off);

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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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
    fn render_frame_without_ceiling_leaves_sky_black() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let mut sector = room_with_front_wall(10.0);
        sector.no_ceiling = true;
        let sectors = [sector];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

        let center_x = WIDTH as usize / 2;
        assert_eq!(frame.pixel(center_x, 10), [0, 0, 0, 255]);
        assert_ne!(frame.pixel(center_x, HEIGHT as usize / 2), [0, 0, 0, 255]);
        assert_ne!(frame.pixel(center_x, HEIGHT as usize - 10), [0, 0, 0, 255]);
    }

    #[test]
    fn render_frame_without_ceiling_uses_sky_color_when_present() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let mut sector = room_with_front_wall(10.0);
        sector.no_ceiling = true;
        sector.sky_color = Some(RawColor([72, 96, 140]));
        let sectors = [sector];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

        let center_x = WIDTH as usize / 2;
        assert_eq!(frame.pixel(center_x, 10), [72, 96, 140, 255]);
    }

    #[test]
    fn render_frame_uses_sector_floor_and_ceiling_colors() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let view = player_render_view(&player);
        let mut sector = room_with_front_wall(10.0);
        sector.floor_color = RawColor([40, 220, 60]);
        sector.ceil_color = RawColor([220, 60, 40]);
        let sectors = [sector];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

        let center_x = WIDTH as usize / 2;
        let ceiling_pixel = frame.pixel(center_x, 10);
        let floor_pixel = frame.pixel(center_x, HEIGHT as usize - 10);

        assert!(ceiling_pixel[0] > ceiling_pixel[1] && ceiling_pixel[0] > ceiling_pixel[2]);
        assert!(floor_pixel[1] > floor_pixel[0] && floor_pixel[1] > floor_pixel[2]);
    }

    #[test]
    fn render_frame_keeps_view_only_portals_visible() {
        let mut sectors = connected_portal_sectors();
        sectors[0].portal_walkable[0] = false;
        sectors[1].portal_walkable[0] = false;

        let frame =
            render_connected_boundary_frame_for_sectors(sectors, 0.5, 0.0, SectorId(0), false);

        assert_no_long_black_run_on_center_row(&frame);
    }

    #[test]
    fn render_frame_skips_upper_trim_between_adjacent_sky_sectors() {
        let mut sectors = connected_portal_sectors();
        sectors[0].no_ceiling = true;
        sectors[1].no_ceiling = true;
        sectors[0].sky_color = Some(RawColor([80, 110, 160]));
        sectors[1].sky_color = Some(RawColor([80, 110, 160]));
        sectors[0].ceil = Length(8.0);
        sectors[1].ceil = Length(4.0);

        let baseline = render_connected_boundary_frame_for_sectors(
            sectors.clone(),
            0.5,
            0.0,
            SectorId(0),
            false,
        );

        sectors[0].portal_upper_colors[0] = Some(RawColor([255, 0, 0]));
        let with_upper_trim =
            render_connected_boundary_frame_for_sectors(sectors, 0.5, 0.0, SectorId(0), false);

        assert_eq!(with_upper_trim.as_slice(), baseline.as_slice());
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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::NorthUpFull);

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
            resolve_current_sector(player.position, player.current_sector, &sectors);
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

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
                assert_no_long_black_run_on_center_row(&frame);
            }
        }
    }

    #[test]
    fn default_map_snapshot_near_portal_boundary_keeps_columns_filled() {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        let player = Player {
            position: Position3(Vec3::new(-0.4816283, 0.17646588, 0.0)),
            direction: Direction(-0.99712837),
            current_sector: Some(SectorId(0)),
            grounded: true,
            ..Player::default()
        };
        let mut frame = FrameBuffer::new();

        render_frame(
            frame.as_mut_slice(),
            &player_render_view(&player),
            &sectors,
            Automap::Off,
        );

        assert_no_fully_black_columns(&frame);
        assert_no_long_black_run_on_center_row(&frame);
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

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
    }

    #[test]
    fn brown_sector_view_back_to_spawn_avoids_wide_black_gaps() {
        let map = ron::de::from_str::<SectorMap>(include_str!("../../assets/maps/default.map.ron"))
            .unwrap();
        let (_, sectors) = map_to_sectors(&map).unwrap();
        let sector = sectors
            .iter()
            .find(|sector| sector.id == SectorId(2))
            .unwrap();
        let centroid = sector_centroid(sector);
        let to_spawn = Vec2::new(map.initial_position.0, map.initial_position.1) - centroid;
        let mut player = Player::default();
        player.position = Position3(Vec3::new(centroid.x, centroid.y, sector.floor.0));
        player.direction.0 = (-to_spawn.x).atan2(to_spawn.y);
        player.current_sector = Some(SectorId(2));
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, &sectors, Automap::Off);

        assert_no_long_black_run_on_center_row(&frame);
    }
}
