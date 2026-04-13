mod frame;
mod math;
mod minimap;
mod world;

use crate::{Position2, Position3, Sector, SectorId};

use bevy::{math::vec2, prelude::*};
use lazy_static::lazy_static;

pub use frame::{clear_frame, FrameBuffer, Pixel, FRAME_BYTES};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub const WINDOW_SCALE: u32 = 4;

pub(crate) const GAP: isize = 1;
pub(crate) const NEAR: f32 = 0.1;
pub(crate) const FAR: f32 = 50.0;
pub(crate) const SHADE_FAR: f32 = 20.0;
pub(crate) const SHADE_BANDS: usize = 14;
pub(crate) const BRIGHTNESS_NEAR: f32 = 1.0;
pub(crate) const BRIGHTNESS_FAR: f32 = 0.35;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const MINIMAP_SCALE: f32 = 8.0;

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
            x: FRAC_WIDTH_2 as isize + (MINIMAP_SCALE * position.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (MINIMAP_SCALE * position.0.y).round() as isize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Minimap {
    Off,
    FirstPerson,
    Absolute,
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
    minimap: Minimap,
) {
    clear_frame(frame);
    let sectors: Vec<_> = sectors.into_iter().collect();
    world::render_world(frame, view, &sectors);
    minimap::render_minimap(frame, view, &sectors, minimap);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{player_render_view, resolve_current_sector, Player},
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

    #[test]
    fn render_frame_without_current_sector_is_background_only() {
        let view = RenderView::new(
            Position3(Vec3::new(0.0, 0.0, crate::game::PLAYER_EYE_HEIGHT_METERS)),
            0.0,
            None,
        );
        let sectors = [room_with_front_wall(10.0)];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Minimap::Off);

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

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Minimap::Off);

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
            Minimap::Off,
        );
        render_frame(
            far_frame.as_mut_slice(),
            &view,
            far_room.iter(),
            Minimap::Off,
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
            Minimap::Off,
        );
        render_frame(
            far_frame.as_mut_slice(),
            &view,
            far_room.iter(),
            Minimap::Off,
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

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Minimap::Off);

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
    fn render_frame_absolute_minimap_draws_player_and_frustum() {
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
            Minimap::Absolute,
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
    fn boundary_transition_keeps_non_black_frame() {
        let sectors = connected_portal_sectors();
        let mut player = Player::default();
        player.position = Position3(Vec3::new(0.0, 1.0, 0.2));
        player.current_sector = Some(SectorId(0));
        player.current_sector =
            resolve_current_sector(player.position, player.current_sector, sectors.iter());
        let view = player_render_view(&player);
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Minimap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
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

        render_frame(frame.as_mut_slice(), &view, sectors.iter(), Minimap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 255]));
    }
}
