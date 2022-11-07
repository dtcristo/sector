mod draw;
mod pixel;

use crate::{draw::*, pixel::*};

use bevy::{
    app::AppExit,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
    math::vec2,
    math::vec3,
    prelude::*,
    utils::Duration,
    window::WindowResizeConstraints,
};
use bevy_pixels::prelude::*;
use bevy_render::color::Color;

#[macro_use]
extern crate lazy_static;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const EDGE_GAP: isize = 1;
const JOIN_GAP: isize = 0;
const WIDTH_MINUS_EDGE_GAP: isize = WIDTH as isize - EDGE_GAP;
const HEIGHT_MINUS_EDGE_GAP: isize = HEIGHT as isize - EDGE_GAP;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const Z_NEAR: f32 = -0.1;
const Z_FAR: f32 = -50.0;
const LIGHTNESS_DISTANCE_NEAR: f32 = -Z_NEAR;
const LIGHTNESS_DISTANCE_FAR: f32 = -Z_FAR;
const DELTA_LIGHTNESS_DISTANCE: f32 = LIGHTNESS_DISTANCE_FAR - LIGHTNESS_DISTANCE_NEAR;
const LIGHTNESS_NEAR: f32 = 0.5;
const LIGHTNESS_FAR: f32 = 0.0;
const MINIMAP_SCALE: f32 = 8.0;

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_reverse_rh(*FOV_Y_RADIANS, ASPECT_RATIO, Z_NEAR);
    static ref TAN_FAC_FOV_X_2: f32 = (FOV_X_RADIANS / 2.0).tan();
    static ref X_NEAR: f32 = -Z_NEAR * *TAN_FAC_FOV_X_2;
    static ref X_FAR: f32 = -Z_FAR * *TAN_FAC_FOV_X_2;
    // Clip boundaries
    static ref BACK_CLIP_1: Vec2 = vec2(*X_NEAR, Z_NEAR);
    static ref BACK_CLIP_2: Vec2 = vec2(-*X_NEAR, Z_NEAR);
    static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_2;
    static ref LEFT_CLIP_2: Vec2 = vec2(-*X_FAR, Z_FAR);
    static ref RIGHT_CLIP_1: Vec2 = vec2(*X_FAR, Z_FAR);
    static ref RIGHT_CLIP_2: Vec2 = *BACK_CLIP_1;
    // Colors
    static ref CEILING_COLOR: Color = Color::SILVER;
    static ref FLOOR_COLOR: Color = Color::GRAY;
    static ref WALL_CLIPPED_COLOR: Color = Color::WHITE;
    static ref FRUSTUM_COLOR: Color = Color::DARK_GRAY;
    static ref PLAYER_COLOR: Color = Color::RED;
}

#[derive(Component, Debug)]
struct Sector {
    vertices: Vec<Vertex>,
    adjacent_sectors: Vec<Option<Entity>>,
    colors: Vec<Color>,
    floor: Length,
    ceiling: Length,
}

impl Sector {
    fn to_walls(&self) -> Vec<Wall> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut adjacent_sector_iter = self.adjacent_sectors.iter();
        let mut color_iter = self.colors.iter();

        let Some(&initial) = vertex_iter.next() else { return walls };

        let mut add_wall = |left: Vertex, right: Vertex| {
            walls.push(Wall {
                left,
                right,
                adjacent_sector: *adjacent_sector_iter.next().unwrap_or(&None),
                color: *color_iter.next().unwrap_or(&Color::RED),
            })
        };

        let mut previous = initial;

        for &vertex in vertex_iter {
            add_wall(previous, vertex);
            previous = vertex;
        }

        add_wall(previous, initial);

        walls
    }
}

#[derive(Debug, Copy, Clone)]
struct Wall {
    left: Vertex,
    right: Vertex,
    adjacent_sector: Option<Entity>,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
pub struct Length(f32);

// Position (https://bevy-cheatbook.github.io/features/coords.html)
// +y.---> +x
//   |
//   v
//   +z
#[derive(Debug, Copy, Clone)]
pub struct Position(Vec3);

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    x: f32,
    z: f32,
}

impl Vertex {
    fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }
}

impl From<Vec2> for Vertex {
    fn from(v: Vec2) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<Vertex> for Vec2 {
    fn from(v: Vertex) -> Self {
        vec2(v.x, v.z)
    }
}

impl From<Vec3> for Vertex {
    fn from(v: Vec3) -> Self {
        Self::new(v.x, v.z)
    }
}

impl From<Vertex> for Vec3 {
    fn from(v: Vertex) -> Self {
        vec3(v.x, 0.0, v.z)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Velocity(Vec3);

// Direction
//   ^   ^
//    \+θ|
//     \ |
//       .
#[derive(Debug, Copy, Clone)]
pub struct Direction(f32);

#[derive(Debug, PartialEq)]
enum Minimap {
    Off,
    FirstPerson,
    Absolute,
}

#[derive(Debug)]
pub struct AppState {
    minimap: Minimap,
    position: Position,
    velocity: Velocity,
    direction: Direction,
    update_title_timer: Timer,
    current_sector: Entity,
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    App::new()
        .insert_resource(WindowDescriptor {
            title: "sector".to_string(),
            width: (3 * WIDTH) as f32,
            height: (3 * HEIGHT) as f32,
            resize_constraints: WindowResizeConstraints {
                min_width: WIDTH as f32,
                min_height: HEIGHT as f32,
                ..default()
            },
            fit_canvas_to_parent: true,
            ..default()
        })
        .insert_resource(PixelsOptions {
            width: WIDTH,
            height: HEIGHT,
        })
        .insert_resource(AppState {
            minimap: Minimap::FirstPerson,
            position: Position(vec3(0.0, 2.0, 0.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            update_title_timer: Timer::new(Duration::from_millis(500), true),
            current_sector: Entity::from_raw(u32::MAX), // Initial invalid Entity, correctly set within setup
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PixelsPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        // .add_plugin(LogDiagnosticsPlugin::default())
        .add_startup_system(setup_system)
        .add_system(update_title_system)
        .add_system(mouse_capture_system)
        .add_system(escape_system)
        .add_system(switch_minimap_system)
        .add_system(player_movement_system)
        .add_system_to_stage(PixelsStage::Draw, draw_background_system)
        .add_system_to_stage(
            PixelsStage::Draw,
            draw_wall_system.after(draw_background_system),
        )
        .add_system_to_stage(
            PixelsStage::Draw,
            draw_minimap_system.after(draw_wall_system),
        )
        .run();
}

fn setup_system(mut commands: Commands, mut state: ResMut<AppState>) {
    // Vertices
    let v0 = Vertex::new(-4.0, -10.0);
    let v1 = Vertex::new(-2.0, -10.0);
    let v2 = Vertex::new(2.0, -5.0);
    let v3 = Vertex::new(4.0, -1.0);
    let v4 = Vertex::new(4.0, 8.0);
    let v5 = Vertex::new(-11.0, 8.0);
    let v6 = Vertex::new(-4.0, -15.0);
    let v7 = Vertex::new(4.0, -15.0);

    // Sectors
    let s0 = commands.spawn().id();
    let s1 = commands.spawn().id();

    // Player starts in sector 0
    state.current_sector = s0;

    commands.entity(s0).insert(Sector {
        vertices: vec![v0, v1, v2, v3, v4, v5],
        adjacent_sectors: vec![None, Some(s1), None, None, None, None],
        colors: vec![
            Color::BLUE,
            Color::WHITE,
            Color::GREEN,
            Color::ORANGE,
            Color::FUCHSIA,
            Color::YELLOW,
        ],
        floor: Length(0.0),
        ceiling: Length(4.0),
    });

    commands.entity(s1).insert(Sector {
        vertices: vec![v2, v1, v6, v7],
        adjacent_sectors: vec![Some(s0), None, None, None],
        colors: vec![Color::WHITE, Color::YELLOW, Color::GREEN, Color::FUCHSIA],
        floor: Length(0.25),
        ceiling: Length(3.75),
    });
}

fn update_title_system(
    mut app_state: ResMut<AppState>,
    mut windows: ResMut<Windows>,
    time: Res<Time>,
    diagnostics: Res<Diagnostics>,
) {
    if app_state.update_title_timer.tick(time.delta()).finished() {
        let window = windows.primary_mut();

        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.value() {
                window.set_title(format!("sector: {value:.0} fps"));
            }
        }
    }
}

fn mouse_capture_system(mut windows: ResMut<Windows>, mouse_button: Res<Input<MouseButton>>) {
    let window = windows.get_primary_mut().unwrap();

    if window.cursor_locked() {
        if mouse_button.just_pressed(MouseButton::Right) {
            window.set_cursor_lock_mode(false);
            window.set_cursor_visibility(true);
        }
    } else {
        if mouse_button.just_pressed(MouseButton::Left) {
            window.set_cursor_lock_mode(true);
            window.set_cursor_visibility(false);
        }
    }
}

fn escape_system(
    mut app_exit_events: EventWriter<AppExit>,
    mut windows: ResMut<Windows>,
    key: Res<Input<KeyCode>>,
) {
    if key.just_pressed(KeyCode::Escape) {
        let window = windows.get_primary_mut().unwrap();

        if window.cursor_locked() {
            window.set_cursor_lock_mode(false);
            window.set_cursor_visibility(true);
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            app_exit_events.send(AppExit);
        }
    }
}

fn switch_minimap_system(mut state: ResMut<AppState>, key: Res<Input<KeyCode>>) {
    if key.just_pressed(KeyCode::Tab) {
        state.minimap = match state.minimap {
            Minimap::Off => Minimap::FirstPerson,
            Minimap::FirstPerson => Minimap::Absolute,
            Minimap::Absolute => Minimap::Off,
        }
    }
}

fn player_movement_system(
    mut state: ResMut<AppState>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    key: Res<Input<KeyCode>>,
    windows: Res<Windows>,
) {
    let window = windows.get_primary().unwrap();

    if window.cursor_locked() {
        for mouse_motion in mouse_motion_events.iter() {
            state.direction.0 += -mouse_motion.delta.x * 0.005;
        }
    }

    if key.pressed(KeyCode::Left) || key.pressed(KeyCode::Q) {
        state.direction.0 += 0.05;
    }
    if key.pressed(KeyCode::Right) || key.pressed(KeyCode::E) {
        state.direction.0 -= 0.05;
    }

    state.velocity.0.x = 0.0;
    state.velocity.0.z = 0.0;
    state.velocity.0.y = 0.0;

    if key.pressed(KeyCode::Up) || key.pressed(KeyCode::W) {
        state.velocity.0.x -= state.direction.0.sin();
        state.velocity.0.z -= state.direction.0.cos();
    }
    if key.pressed(KeyCode::Down) || key.pressed(KeyCode::S) {
        state.velocity.0.x += state.direction.0.sin();
        state.velocity.0.z += state.direction.0.cos();
    }
    if key.pressed(KeyCode::A) {
        state.velocity.0.x -= state.direction.0.cos();
        state.velocity.0.z += state.direction.0.sin();
    }
    if key.pressed(KeyCode::D) {
        state.velocity.0.x += state.direction.0.cos();
        state.velocity.0.z -= state.direction.0.sin();
    }
    if key.pressed(KeyCode::Space) {
        state.velocity.0.y += 1.0;
    }
    if key.pressed(KeyCode::LControl) {
        state.velocity.0.y -= 1.0;
    }

    state.position.0.x += 0.05 * state.velocity.0.x;
    state.position.0.y += 0.05 * state.velocity.0.y;
    state.position.0.z += 0.05 * state.velocity.0.z;
}

fn draw_background_system(mut pixels_resource: ResMut<PixelsResource>) {
    let frame = pixels_resource.pixels.get_frame_mut();
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_wall_system(
    mut pixels_resource: ResMut<PixelsResource>,
    state: Res<AppState>,
    sector_query: Query<&Sector>,
) {
    let frame = pixels_resource.pixels.get_frame_mut();
    let view_matrix = Mat3::from_rotation_z(state.direction.0)
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.z));

    let Ok(sector) = sector_query.get(state.current_sector) else { return };

    let view_floor = Length(sector.floor.0 - state.position.0.y);
    let view_ceiling = Length(sector.ceiling.0 - state.position.0.y);

    for wall in sector.to_walls() {
        let view_left = view_matrix.transform_point2(wall.left.into()).into();
        let view_right = view_matrix.transform_point2(wall.right.into()).into();

        if let Some((view_left, view_right)) = clip_wall(view_left, view_right) {
            draw_wall(
                frame,
                view_left,
                view_right,
                view_floor,
                view_ceiling,
                wall.color,
            );
        };
    }
}

fn clip_wall(mut view_left: Vertex, mut view_right: Vertex) -> Option<(Vertex, Vertex)> {
    // Skip entirely behind back
    if view_left.z > Z_NEAR && view_right.z > Z_NEAR {
        return None;
    }

    // Clip left side
    if let Some(intersection) = intersect(
        view_left.into(),
        view_right.into(),
        *LEFT_CLIP_1,
        *LEFT_CLIP_2,
    ) {
        if intersection.x < -*X_NEAR {
            if point_behind(view_left.into(), *LEFT_CLIP_1, *LEFT_CLIP_2) {
                view_left = intersection.into();
            } else {
                view_right = intersection.into();
            }
        }
    }

    // Clip right side
    if let Some(intersection) = intersect(
        view_left.into(),
        view_right.into(),
        *RIGHT_CLIP_1,
        *RIGHT_CLIP_2,
    ) {
        if intersection.x > *X_NEAR {
            if point_behind(view_left.into(), *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
                view_left = intersection.into();
            } else {
                view_right = intersection.into();
            }
        }
    }

    // Clip behind back
    if view_left.z > Z_NEAR || view_right.z > Z_NEAR {
        if let Some(intersection) = intersect(
            view_left.into(),
            view_right.into(),
            *BACK_CLIP_1,
            *BACK_CLIP_2,
        ) {
            if point_behind(view_left.into(), *BACK_CLIP_1, *BACK_CLIP_2) {
                view_left = intersection.into();
            } else {
                view_right = intersection.into();
            }
        }
    }

    // Skip entirely behind left side
    if point_behind(view_left.into(), *LEFT_CLIP_1, *LEFT_CLIP_2) {
        return None;
    }

    // Skip entirely behind right side
    if point_behind(view_left.into(), *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
        return None;
    }

    Some((view_left, view_right))
}

fn intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
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

fn between(test: f32, a: f32, b: f32) -> bool {
    test >= a.min(b) && test <= a.max(b)
}

fn point_behind(point: Vec2, a: Vec2, b: Vec2) -> bool {
    vec2(b.x - a.x, b.y - a.y).perp_dot(vec2(point.x - a.x, point.y - a.y)) < 0.0
}

fn draw_minimap_system(
    mut pixels_resource: ResMut<PixelsResource>,
    state: Res<AppState>,
    sector_query: Query<&Sector>,
) {
    if state.minimap == Minimap::Off {
        return;
    }

    let frame = pixels_resource.pixels.get_frame_mut();
    let view_matrix = Mat3::from_rotation_z(state.direction.0)
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.z));
    let reverse_view_matrix = Mat3::from_translation(vec2(state.position.0.x, state.position.0.z))
        * Mat3::from_rotation_z(-state.direction.0);

    // Draw walls
    for sector in sector_query.iter() {
        for wall in sector.to_walls() {
            let view_left = view_matrix.transform_point2(wall.left.into()).into();
            let view_right = view_matrix.transform_point2(wall.right.into()).into();

            let mut view_left_after_clip = view_left;
            let mut view_right_after_clip = view_right;

            let clipping = clip_wall(view_left, view_right);
            if let Some((l, r)) = clipping {
                view_left_after_clip = l;
                view_right_after_clip = r;
            }

            if let Some((left, right, left_after_clip, right_after_clip)) = match state.minimap {
                Minimap::Off => None,
                Minimap::FirstPerson => Some((
                    Pixel::from_absolute(view_left.into()),
                    Pixel::from_absolute(view_right.into()),
                    Pixel::from_absolute(view_left_after_clip.into()),
                    Pixel::from_absolute(view_right_after_clip.into()),
                )),
                Minimap::Absolute => {
                    let absolute_left = reverse_view_matrix.transform_point2(view_left.into());
                    let absolute_right = reverse_view_matrix.transform_point2(view_right.into());
                    let absolute_left_after_clip =
                        reverse_view_matrix.transform_point2(view_left_after_clip.into());
                    let absolute_right_after_clip =
                        reverse_view_matrix.transform_point2(view_right_after_clip.into());

                    Some((
                        Pixel::from_absolute(absolute_left),
                        Pixel::from_absolute(absolute_right),
                        Pixel::from_absolute(absolute_left_after_clip),
                        Pixel::from_absolute(absolute_right_after_clip),
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
                draw_line(frame, left_after_clip, right_after_clip, wall.color);
            }
        }
    }

    // Draw frustum and player
    let view_player = vec2(0.0, 0.0);
    let view_near_left = vec2(-*X_NEAR, Z_NEAR);
    let view_near_right = vec2(*X_NEAR, Z_NEAR);
    let view_far_left = vec2(-*X_FAR, Z_FAR);
    let view_far_right = vec2(*X_FAR, Z_FAR);

    if let Some((player, near_left, near_right, far_left, far_right)) = match state.minimap {
        Minimap::Off => None,
        Minimap::FirstPerson => Some((
            Pixel::from_absolute(view_player),
            Pixel::from_absolute(view_near_left),
            Pixel::from_absolute(view_near_right),
            Pixel::from_absolute(view_far_left),
            Pixel::from_absolute(view_far_right),
        )),
        Minimap::Absolute => {
            let absolute_player = vec2(state.position.0.x, state.position.0.z);
            let absolute_near_left = reverse_view_matrix.transform_point2(view_near_left);
            let absolute_near_right = reverse_view_matrix.transform_point2(view_near_right);
            let absolute_far_left = reverse_view_matrix.transform_point2(view_far_left);
            let absolute_far_right = reverse_view_matrix.transform_point2(view_far_right);

            Some((
                Pixel::from_absolute(absolute_player),
                Pixel::from_absolute(absolute_near_left),
                Pixel::from_absolute(absolute_near_right),
                Pixel::from_absolute(absolute_far_left),
                Pixel::from_absolute(absolute_far_right),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_pixel(frame, player, *PLAYER_COLOR);
    }
}
