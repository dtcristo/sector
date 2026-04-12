mod draw;
mod math;

use crate::*;

use bevy::{
    ecs::system::Commands,
    input::ButtonInput,
    math::{vec2, vec3},
    prelude::{Component, KeyCode, Vec2, Vec3},
};
use palette::Hsv;
use std::collections::{HashMap, VecDeque};

use draw::*;
use math::*;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub const WINDOW_SCALE: u32 = 4;
pub const FRAME_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;

const GAP: isize = 1;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const NEAR: f32 = 0.1;
const FAR: f32 = 50.0;
const BRIGHTNESS_NEAR: f32 = 1.0;
const BRIGHTNESS_FAR: f32 = 0.0;
const MINIMAP_SCALE: f32 = 8.0;
const PLAYER_MOVE_STEP: f32 = 0.05;
const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;
const KEYBOARD_LOOK_STEP: f32 = 0.0001;

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_reverse_rh(*FOV_Y_RADIANS, ASPECT_RATIO, NEAR);
    static ref TAN_FAC_FOV_X_2: f32 = (FOV_X_RADIANS / 2.0).tan();
    static ref X_NEAR: f32 = NEAR * *TAN_FAC_FOV_X_2;
    static ref X_FAR: f32 = FAR * *TAN_FAC_FOV_X_2;
    static ref BACK_CLIP_1: Vec2 = vec2(*X_NEAR, NEAR);
    static ref BACK_CLIP_2: Vec2 = vec2(-*X_NEAR, NEAR);
    static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_2;
    static ref LEFT_CLIP_2: Vec2 = vec2(-*X_FAR, FAR);
    static ref RIGHT_CLIP_1: Vec2 = vec2(*X_FAR, FAR);
    static ref RIGHT_CLIP_2: Vec2 = *BACK_CLIP_1;
}

#[derive(Debug, Copy, Clone)]
pub struct Normalized(pub Vec3);

impl From<Normalized> for Pixel {
    fn from(norm: Normalized) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * norm.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * norm.0.y).round() as isize,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Direction(pub f32);

#[derive(Component, Debug, Copy, Clone, PartialEq)]
pub struct Player {
    pub position: Position3,
    pub velocity: Velocity,
    pub direction: Direction,
    pub current_sector: Option<SectorId>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: Position3(vec3(0.0, 0.0, 2.0)),
            velocity: Velocity(Vec3::ZERO),
            direction: Direction(0.0),
            current_sector: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Minimap {
    Off,
    FirstPerson,
    Absolute,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub strafe_left: bool,
    pub strafe_right: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub turn_left: bool,
    pub turn_right: bool,
    pub mouse_delta_x: f32,
    pub mouse_look_enabled: bool,
}

impl PlayerInput {
    pub fn from_keys(keys: &ButtonInput<KeyCode>) -> Self {
        Self {
            forward: keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW),
            backward: keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS),
            strafe_left: keys.pressed(KeyCode::KeyA),
            strafe_right: keys.pressed(KeyCode::KeyD),
            move_up: keys.pressed(KeyCode::Space),
            move_down: keys.pressed(KeyCode::ControlLeft),
            turn_left: keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyQ),
            turn_right: keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyE),
            ..default()
        }
    }

    pub fn with_mouse_look(mut self, mouse_delta_x: f32, enabled: bool) -> Self {
        self.mouse_delta_x = mouse_delta_x;
        self.mouse_look_enabled = enabled;
        self
    }
}

pub struct FrameBuffer {
    bytes: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        let mut buffer = Self {
            bytes: vec![0; FRAME_BYTES],
        };
        clear_frame(buffer.as_mut_slice());
        buffer
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let offset = (y * WIDTH as usize + x) * 4;
        [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
        ]
    }

    pub fn count_color(&self, color: RawColor) -> usize {
        self.bytes
            .chunks_exact(4)
            .filter(|chunk| {
                chunk[0] == color.0[0] && chunk[1] == color.0[1] && chunk[2] == color.0[2]
            })
            .count()
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn setup_player_system(mut commands: Commands) {
    commands.spawn(Player::default());
}

pub fn apply_player_look(player: &mut Player, input: PlayerInput) {
    if input.mouse_look_enabled {
        player.direction.0 += -input.mouse_delta_x * MOUSE_LOOK_SENSITIVITY;
    }

    if input.turn_left {
        player.direction.0 += KEYBOARD_LOOK_STEP;
    }
    if input.turn_right {
        player.direction.0 -= KEYBOARD_LOOK_STEP;
    }
}

pub fn apply_player_translation_input(player: &mut Player, input: PlayerInput) {
    player.velocity.0 = Vec3::ZERO;

    if input.forward {
        player.velocity.0.x -= player.direction.0.sin();
        player.velocity.0.y += player.direction.0.cos();
    }
    if input.backward {
        player.velocity.0.x += player.direction.0.sin();
        player.velocity.0.y -= player.direction.0.cos();
    }
    if input.strafe_left {
        player.velocity.0.x -= player.direction.0.cos();
        player.velocity.0.y -= player.direction.0.sin();
    }
    if input.strafe_right {
        player.velocity.0.x += player.direction.0.cos();
        player.velocity.0.y += player.direction.0.sin();
    }
    if input.move_up {
        player.velocity.0.z += 1.0;
    }
    if input.move_down {
        player.velocity.0.z -= 1.0;
    }
}

pub fn move_player(player: &mut Player) {
    player.position.0 += PLAYER_MOVE_STEP * player.velocity.0;
}

pub fn update_current_sector<'a>(
    player: &mut Player,
    sectors: impl IntoIterator<Item = &'a Sector>,
) {
    if let Some(sector_id) = sectors
        .into_iter()
        .find(|sector| sector_contains_position(sector, player.position))
        .map(|sector| sector.id)
    {
        player.current_sector = Some(sector_id);
    }
}

pub fn sector_contains_position(sector: &Sector, position: Position3) -> bool {
    if position.0.z < sector.floor.0 || position.0.z > sector.ceil.0 {
        return false;
    }

    let point = position.truncate().0;
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
        if point.x < intersect_x {
            inside = !inside;
        }
    }

    inside
}

pub fn clear_frame(frame: &mut [u8]) {
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

pub fn render_frame<'a>(
    frame: &mut [u8],
    player: &Player,
    sectors: impl IntoIterator<Item = &'a Sector>,
    minimap: Minimap,
) {
    clear_frame(frame);
    let sectors: Vec<_> = sectors.into_iter().collect();
    render_world(frame, player, &sectors);
    render_minimap(frame, player, &sectors, minimap);
}

fn render_world(frame: &mut [u8], player: &Player, sectors: &[&Sector]) {
    let sectors_by_id: HashMap<_, _> = sectors.iter().map(|sector| (sector.id, *sector)).collect();
    let Some(current_sector) = player
        .current_sector
        .and_then(|id| sectors_by_id.get(&id).copied())
    else {
        return;
    };

    let view_matrix = Mat3::from_rotation_z(-player.direction.0)
        * Mat3::from_translation(-vec2(player.position.0.x, player.position.0.y));

    let mut portal_queue = VecDeque::<Portal>::new();
    let mut y_min_vec = vec![GAP; WIDTH as usize];
    let mut y_max_vec = vec![HEIGHT as isize; WIDTH as usize];

    portal_queue.push_back(Portal {
        sector: current_sector,
        x_min: GAP,
        x_max: WIDTH as isize,
    });

    while let Some(self_portal) = portal_queue.pop_front() {
        let sector = self_portal.sector;
        let view_floor = Length(sector.floor.0 - player.position.0.z);
        let view_ceil = Length(sector.ceil.0 - player.position.0.z);

        'walls: for wall in sector.to_walls() {
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

                let (y_portal_top, y_portal_bottom) = if let Some(portal_sector) = portal_sector {
                    portal_queue.push_back(Portal {
                        sector: portal_sector,
                        x_min: x_left,
                        x_max: x_right,
                    });

                    let view_portal_ceil = Length(portal_sector.ceil.0 - player.position.0.z);
                    let view_portal_floor = Length(portal_sector.floor.0 - player.position.0.z);

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

                for x in x_left..x_right {
                    let skip_floor_ceil = x >= self_portal.x_max as isize - GAP;
                    let skip_wall = x >= x_right - GAP;
                    let x_t = (x - left_top.x) as f32 / dx as f32;

                    let view_z = lerp(view_left.0.y, view_right.0.y, x_t);
                    let distance = view_z.abs();
                    let brightness = if distance > FAR {
                        BRIGHTNESS_FAR
                    } else if distance < NEAR {
                        BRIGHTNESS_NEAR
                    } else {
                        let distance_t = (distance - NEAR) / (FAR - NEAR);
                        lerp(BRIGHTNESS_NEAR, BRIGHTNESS_FAR, distance_t)
                    };
                    let brightness_rounded = (brightness * 100.0).round() / 100.0;
                    let color: RawColor =
                        Hsv::new(wall.color.hue, wall.color.saturation, brightness_rounded).into();

                    let y_top = lerpi(left_top.y, right_top.y, x_t);
                    let y_bottom = lerpi(left_bottom.y, right_bottom.y, x_t);

                    let y_min = y_min_vec[x as usize];
                    let y_max = y_max_vec[x as usize];

                    let y_top = y_top.clamp(y_min, y_max);
                    let y_bottom = y_bottom.clamp(y_min, y_max);

                    let y_ceil_top = y_min;
                    let y_ceil_bottom = y_top;
                    let y_floor_top = y_bottom;
                    let y_floor_bottom = y_max;

                    if !skip_floor_ceil {
                        draw_vertical_line(
                            frame,
                            x,
                            y_ceil_top,
                            y_ceil_bottom - GAP,
                            *CEILING_COLOR,
                        );
                    }

                    if portal_sector.is_some() {
                        if let Some((y_portal_left_top, y_portal_right_top)) = y_portal_top {
                            let y_portal_top = lerpi(y_portal_left_top, y_portal_right_top, x_t)
                                .clamp(y_min, y_bottom);
                            if !skip_wall {
                                draw_vertical_line(frame, x, y_top, y_portal_top - GAP, color);
                            }
                            y_min_vec[x as usize] = y_portal_top;
                        } else {
                            y_min_vec[x as usize] = y_top;
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
                            y_max_vec[x as usize] = y_portal_bottom;
                        } else {
                            y_max_vec[x as usize] = y_bottom;
                        }
                    } else if !skip_wall {
                        draw_vertical_line(frame, x, y_top, y_bottom - GAP, color);
                    }

                    if !skip_floor_ceil {
                        draw_vertical_line(
                            frame,
                            x,
                            y_floor_top,
                            y_floor_bottom - GAP,
                            *FLOOR_COLOR,
                        );
                    }
                }
            }
        }
    }
}

fn render_minimap(frame: &mut [u8], player: &Player, sectors: &[&Sector], minimap: Minimap) {
    if minimap == Minimap::Off {
        return;
    }

    let view_matrix = Mat3::from_rotation_z(-player.direction.0)
        * Mat3::from_translation(-vec2(player.position.0.x, player.position.0.y));
    let reverse_view_matrix =
        Mat3::from_translation(vec2(player.position.0.x, player.position.0.y))
            * Mat3::from_rotation_z(player.direction.0);

    for sector in sectors {
        for wall in sector.to_walls() {
            let color: RawColor = wall.color.into();
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            let mut view_left_after_clip = view_left;
            let mut view_right_after_clip = view_right;

            let clipping = clip_wall(view_left, view_right);
            if let Some((l, r)) = clipping {
                view_left_after_clip = l;
                view_right_after_clip = r;
            }

            if let Some((left, right, left_after_clip, right_after_clip)) = match minimap {
                Minimap::Off => None,
                Minimap::FirstPerson => Some((
                    view_left.into(),
                    view_right.into(),
                    view_left_after_clip.into(),
                    view_right_after_clip.into(),
                )),
                Minimap::Absolute => {
                    let abs_left = wall.left;
                    let abs_right = wall.right;
                    let abs_left_after_clip = view_left_after_clip.transform(reverse_view_matrix);
                    let abs_right_after_clip = view_right_after_clip.transform(reverse_view_matrix);

                    Some((
                        abs_left.into(),
                        abs_right.into(),
                        abs_left_after_clip.into(),
                        abs_right_after_clip.into(),
                    ))
                }
            } {
                if clipping.is_none() {
                    draw_line(frame, left, right, *WALL_CLIPPED_COLOR);
                    continue;
                }
                if left_after_clip != left {
                    draw_line(frame, left, left_after_clip, *WALL_CLIPPED_COLOR);
                }
                if right_after_clip != right {
                    draw_line(frame, right_after_clip, right, *WALL_CLIPPED_COLOR);
                }
                draw_line(frame, left_after_clip, right_after_clip, color);
            }
        }
    }

    let view_player = Position2(vec2(0.0, 0.0));
    let view_near_left = Position2(*LEFT_CLIP_1);
    let view_near_right = Position2(*RIGHT_CLIP_2);
    let view_far_left = Position2(*LEFT_CLIP_2);
    let view_far_right = Position2(*RIGHT_CLIP_1);

    if let Some((player_pixel, near_left, near_right, far_left, far_right)) = match minimap {
        Minimap::Off => None,
        Minimap::FirstPerson => Some((
            view_player.into(),
            view_near_left.into(),
            view_near_right.into(),
            view_far_left.into(),
            view_far_right.into(),
        )),
        Minimap::Absolute => {
            let abs_player = player.position.truncate();
            let abs_near_left = view_near_left.transform(reverse_view_matrix);
            let abs_near_right = view_near_right.transform(reverse_view_matrix);
            let abs_far_left = view_far_left.transform(reverse_view_matrix);
            let abs_far_right = view_far_right.transform(reverse_view_matrix);

            Some((
                abs_player.into(),
                abs_near_left.into(),
                abs_near_right.into(),
                abs_far_left.into(),
                abs_far_right.into(),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_pixel(frame, player_pixel, *PLAYER_COLOR);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub x: isize,
    pub y: isize,
}

impl From<Position2> for Pixel {
    fn from(position: Position2) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (MINIMAP_SCALE * position.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (MINIMAP_SCALE * position.0.y).round() as isize,
        }
    }
}

impl Pixel {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    pub fn to_tuple(self) -> (isize, isize) {
        (self.x, self.y)
    }

    pub fn to_offset(self) -> Option<usize> {
        if self.x >= 0 && self.x < WIDTH as isize && self.y >= 0 && self.y < HEIGHT as isize {
            Some((self.y as u32 * WIDTH * 4 + self.x as u32 * 4) as usize)
        } else {
            None
        }
    }

    pub fn to_offset_unchecked(self) -> usize {
        (self.y as u32 * WIDTH * 4 + self.x as u32 * 4) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{app::App, ecs::system::{Query, Res}, prelude::Update};

    fn color(r: u8, g: u8, b: u8) -> RawColor {
        RawColor([r, g, b])
    }

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

    fn simple_room() -> Sector {
        room_with_front_wall(10.0)
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
    fn player_moves_forward_from_default_direction() {
        let mut player = Player::default();
        apply_player_translation_input(
            &mut player,
            PlayerInput {
                forward: true,
                ..default()
            },
        );
        move_player(&mut player);
        assert_eq!(player.position.0, vec3(0.0, 0.05, 2.0));
    }

    #[test]
    fn mouse_look_only_applies_when_enabled() {
        let mut player = Player::default();
        apply_player_look(
            &mut player,
            PlayerInput::default().with_mouse_look(10.0, false),
        );
        assert_eq!(player.direction.0, 0.0);

        apply_player_look(
            &mut player,
            PlayerInput::default().with_mouse_look(10.0, true),
        );
        assert!((player.direction.0 + 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn sector_contains_position_checks_polygon_and_height() {
        let sector = simple_room();
        assert!(sector_contains_position(
            &sector,
            Position3(vec3(0.0, 0.0, 2.0))
        ));
        assert!(!sector_contains_position(
            &sector,
            Position3(vec3(20.0, 0.0, 2.0))
        ));
        assert!(!sector_contains_position(
            &sector,
            Position3(vec3(0.0, 0.0, 10.0))
        ));
    }

    #[test]
    fn update_current_sector_preserves_existing_sector_if_outside_all() {
        let mut player = Player {
            current_sector: Some(SectorId(7)),
            position: Position3(vec3(100.0, 100.0, 2.0)),
            ..default()
        };

        let sectors = [simple_room()];
        update_current_sector(&mut player, sectors.iter());

        assert_eq!(player.current_sector, Some(SectorId(7)));
    }

    #[test]
    fn render_frame_without_current_sector_is_background_only() {
        let player = Player::default();
        let sectors = [simple_room()];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &player, sectors.iter(), Minimap::Off);

        assert!(frame
            .as_slice()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255]));
    }

    #[test]
    fn render_frame_draws_ceiling_wall_and_floor_on_center_column() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let sectors = [simple_room()];
        let mut frame = FrameBuffer::new();

        render_frame(frame.as_mut_slice(), &player, sectors.iter(), Minimap::Off);

        let center_x = WIDTH as usize / 2;
        assert_eq!(
            frame.pixel(center_x, 10),
            [
                CEILING_COLOR.0[0],
                CEILING_COLOR.0[1],
                CEILING_COLOR.0[2],
                255
            ]
        );
        assert_eq!(frame.pixel(center_x, HEIGHT as usize / 2), [204, 0, 0, 255]);
        assert_eq!(
            frame.pixel(center_x, HEIGHT as usize - 10),
            [FLOOR_COLOR.0[0], FLOOR_COLOR.0[1], FLOOR_COLOR.0[2], 255]
        );
    }

    #[test]
    fn render_frame_shades_nearer_wall_brighter_than_farther_wall() {
        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        let near_room = [room_with_front_wall(2.0)];
        let far_room = [room_with_front_wall(10.0)];
        let mut near_frame = FrameBuffer::new();
        let mut far_frame = FrameBuffer::new();

        render_frame(
            near_frame.as_mut_slice(),
            &player,
            near_room.iter(),
            Minimap::Off,
        );
        render_frame(
            far_frame.as_mut_slice(),
            &player,
            far_room.iter(),
            Minimap::Off,
        );

        let center_x = WIDTH as usize / 2;
        let center_y = HEIGHT as usize / 2;
        assert!(near_frame.pixel(center_x, center_y)[0] > far_frame.pixel(center_x, center_y)[0]);
    }

    #[test]
    fn render_frame_absolute_minimap_draws_player_and_frustum() {
        let player = Player::default();
        let sectors = [simple_room()];
        let mut frame = FrameBuffer::new();

        render_frame(
            frame.as_mut_slice(),
            &player,
            sectors.iter(),
            Minimap::Absolute,
        );

        assert_eq!(
            frame.pixel(WIDTH as usize / 2, HEIGHT as usize / 2),
            [PLAYER_COLOR.0[0], PLAYER_COLOR.0[1], PLAYER_COLOR.0[2], 255]
        );
        assert!(frame.count_color(*FRUSTUM_COLOR) > 0);
    }

    #[test]
    fn headless_app_can_move_player_between_sectors() {
        fn headless_step_system(
            keys: Res<ButtonInput<KeyCode>>,
            mut player_query: Query<&mut Player>,
            sector_query: Query<&Sector>,
        ) {
            let mut player = player_query.single_mut().unwrap();
            let input = PlayerInput::from_keys(&keys);
            apply_player_look(&mut player, input);
            apply_player_translation_input(&mut player, input);
            move_player(&mut player);
            update_current_sector(&mut player, sector_query.iter());
        }

        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_systems(Update, headless_step_system);

        let mut player = Player::default();
        player.current_sector = Some(SectorId(0));
        app.world_mut().spawn(player);
        for sector in connected_portal_sectors() {
            app.world_mut().spawn(sector);
        }

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
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
    fn to_walls_wraps_last_vertex_back_to_start() {
        let sector = sector(
            0,
            &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
            &[None, None, None],
            &[[1, 2, 3], [4, 5, 6], [7, 8, 9]],
            0.0,
            1.0,
        );

        let walls = sector.to_walls();

        assert_eq!(walls.len(), 3);
        assert_eq!(walls[2].left, Position2(vec2(1.0, 1.0)));
        assert_eq!(walls[2].right, Position2(vec2(0.0, 0.0)));
        assert_eq!(walls[2].raw_color, color(7, 8, 9));
    }
}
