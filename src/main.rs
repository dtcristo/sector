mod draw;
mod utils;

use crate::{draw::*, utils::*};

use bevy::{
    app::AppExit,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
    math::vec2,
    math::vec3,
    prelude::*,
    scene::serde::SceneSerializer,
    tasks::IoTaskPool,
    utils::Duration,
    window::{CursorGrabMode, WindowResizeConstraints},
};
use bevy_pixels::prelude::*;
use bevy_render::color::Color;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

#[macro_use]
extern crate lazy_static;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const WINDOW_SCALE: u32 = 3;
const GAP: isize = 1;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const FOV_X_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
const NEAR: f32 = 0.1;
const FAR: f32 = 50.0;
const LIGHTNESS_NEAR: f32 = 0.5;
const LIGHTNESS_FAR: f32 = 0.0;
const MINIMAP_SCALE: f32 = 8.0;
const DEFAULT_SCENE_RON_FILE_PATH: &str = "scenes/default.scn.ron";
const DEFAULT_SCENE_MP_FILE_PATH: &str = "scenes/default.scn.mp";

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_reverse_rh(*FOV_Y_RADIANS, ASPECT_RATIO, NEAR);
    static ref TAN_FAC_FOV_X_2: f32 = (FOV_X_RADIANS / 2.0).tan();
    static ref X_NEAR: f32 = NEAR * *TAN_FAC_FOV_X_2;
    static ref X_FAR: f32 = FAR * *TAN_FAC_FOV_X_2;
    // Clip boundaries
    static ref BACK_CLIP_1: Vec2 = vec2(*X_NEAR, NEAR);
    static ref BACK_CLIP_2: Vec2 = vec2(-*X_NEAR, NEAR);
    static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_2;
    static ref LEFT_CLIP_2: Vec2 = vec2(-*X_FAR, FAR);
    static ref RIGHT_CLIP_1: Vec2 = vec2(*X_FAR, FAR);
    static ref RIGHT_CLIP_2: Vec2 = *BACK_CLIP_1;
    // Colors
    static ref CEILING_COLOR: Color = Color::SILVER;
    static ref FLOOR_COLOR: Color = Color::GRAY;
    static ref WALL_CLIPPED_COLOR: Color = Color::WHITE;
    static ref FRUSTUM_COLOR: Color = Color::DARK_GRAY;
    static ref PLAYER_COLOR: Color = Color::RED;
}

#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
pub struct Sector {
    vertices: Vec<Position2>,
    portal_sectors: Vec<Option<Entity>>,
    colors: Vec<Color>,
    floor: Length,
    ceil: Length,
}

impl Sector {
    fn to_walls(&self) -> Vec<Wall> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut portal_sector_iter = self.portal_sectors.iter();
        let mut color_iter = self.colors.iter();

        let Some(&initial) = vertex_iter.next() else { return walls };

        let mut add_wall = |left: Position2, right: Position2| {
            walls.push(Wall {
                left,
                right,
                portal_sector: *portal_sector_iter.next().unwrap_or(&None),
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

struct Portal<'a> {
    sector: &'a Sector,
    x_min: isize,
    x_max: isize,
}

struct Wall {
    left: Position2,
    right: Position2,
    portal_sector: Option<Entity>,
    color: Color,
}

#[derive(Reflect, Debug, Copy, Clone, Default)]
pub struct Length(f32);

/// World position in 3D, right-handed coordinate system with z up.
///
///   +y
///   ^
///   |
/// +z.---> +x
#[derive(Debug, Copy, Clone)]
pub struct Position3(Vec3);

impl Position3 {
    pub fn truncate(self: Self) -> Position2 {
        Position2(self.0.truncate())
    }
}

/// World position in 2D.
///
///  +y
///  ^
///  |
///  .---> +x
#[derive(Reflect, FromReflect, Debug, Copy, Clone, Default)]
pub struct Position2(Vec2);

impl Position2 {
    pub fn to_pixel(self: Self) -> Pixel {
        Pixel {
            x: FRAC_WIDTH_2 as isize + (MINIMAP_SCALE * self.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (MINIMAP_SCALE * self.0.y).round() as isize,
        }
    }

    pub fn transform(self: Self, matrix: Mat3) -> Self {
        Position2(matrix.transform_point2(self.0))
    }
}

/// Normalized screen coordinates, right-handed coordinate system with z towards,
/// origin at centre.
///
///   +y
///   ^
///   |
/// +z.---> +x
#[derive(Debug, Copy, Clone)]
pub struct Normalized(Vec3);

impl Normalized {
    pub fn to_pixel(self: Self) -> Pixel {
        Pixel {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * self.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * self.0.y).round() as isize,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Velocity(Vec3);

/// Direction, positive right-handed around z-axis. Zero in direction of y-axis.
///
///   ^   ^
///    \+θ|
///     \ |
///     +z.
#[derive(Debug, Copy, Clone)]
pub struct Direction(f32);

/// Pixel location, origin at top left.
///
///  .---> +x
///  |
///  v
///  +y
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    pub x: isize,
    pub y: isize,
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

#[derive(Debug, PartialEq)]
enum Minimap {
    Off,
    FirstPerson,
    Absolute,
}

#[derive(Resource, Debug)]
struct AppState {
    minimap: Minimap,
    position: Position3,
    velocity: Velocity,
    direction: Direction,
    update_title_timer: Timer,
    current_sector: Entity,
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    App::new()
        .register_type::<Sector>()
        .register_type::<Position2>()
        .register_type::<Length>()
        .register_type::<Option<Entity>>()
        .register_type::<Color>()
        .insert_resource(AppState {
            minimap: Minimap::Off,
            position: Position3(vec3(0.0, 0.0, 2.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
            current_sector: Entity::from_raw(u32::MAX), // Initial invalid Entity, correctly set within setup
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: "sector".to_string(),
                width: (WINDOW_SCALE * WIDTH) as f32,
                height: (WINDOW_SCALE * HEIGHT) as f32,
                resize_constraints: WindowResizeConstraints {
                    min_width: WIDTH as f32,
                    min_height: HEIGHT as f32,
                    ..default()
                },
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_plugin(PixelsPlugin {
            width: WIDTH,
            height: HEIGHT,
            ..default()
        })
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        // .add_plugin(LogDiagnosticsPlugin::default())
        .add_startup_system(setup_system)
        .add_startup_system(save_scene_system.after(setup_system))
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

fn setup_system(world: &mut World) {
    // Vertices
    let v0 = Position2(vec2(2.0, 10.0));
    let v1 = Position2(vec2(4.0, 10.0));
    let v2 = Position2(vec2(11.0, -8.0));
    let v3 = Position2(vec2(-4.0, -8.0));
    let v4 = Position2(vec2(-4.0, 1.0));
    let v5 = Position2(vec2(-2.0, 5.0));
    let v6 = Position2(vec2(-4.0, 15.0));
    let v7 = Position2(vec2(4.0, 15.0));
    let v8 = Position2(vec2(-7.0, -9.0));
    let v9 = Position2(vec2(-10.0, -5.0));

    // Sectors
    let s0 = world.spawn_empty().id();
    let s1 = world.spawn_empty().id();
    let s2 = world.spawn_empty().id();

    // Get mutable `AppState` resource
    let mut state = world.resource_mut::<AppState>();

    // Player starts in sector 0
    state.current_sector = s0;

    world.entity_mut(s0).insert(Sector {
        vertices: vec![v0, v1, v2, v3, v4, v5],
        portal_sectors: vec![None, None, None, Some(s2), None, Some(s1)],
        colors: vec![
            Color::BLUE,
            Color::GREEN,
            Color::ORANGE,
            Color::FUCHSIA,
            Color::YELLOW,
            Color::RED,
        ],
        floor: Length(0.0),
        ceil: Length(4.0),
    });

    world.entity_mut(s1).insert(Sector {
        vertices: vec![v0, v5, v6, v7],
        portal_sectors: vec![Some(s0), None, None, None],
        colors: vec![Color::RED, Color::FUCHSIA, Color::GREEN, Color::YELLOW],
        floor: Length(0.25),
        ceil: Length(3.75),
    });

    world.entity_mut(s2).insert(Sector {
        vertices: vec![v4, v3, v8, v9],
        portal_sectors: vec![Some(s0), None, None, None],
        colors: vec![Color::RED, Color::FUCHSIA, Color::GREEN, Color::BLUE],
        floor: Length(-0.5),
        ceil: Length(4.5),
    });
}

fn save_scene_system(world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>();
    let scene = DynamicScene::from_world(&world, type_registry);

    let scene_ron = scene.serialize_ron(type_registry).unwrap();
    info!("{}", scene_ron);

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/{DEFAULT_SCENE_RON_FILE_PATH}"))
                .and_then(|mut file| file.write(scene_ron.as_bytes()))
                .expect("failed to write `scene_ron` to file");
        })
        .detach();

    let scene_serializer = SceneSerializer::new(&scene, type_registry);
    let scene_mp: Vec<u8> = rmp_serde::to_vec(&scene_serializer).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/{DEFAULT_SCENE_MP_FILE_PATH}"))
                .and_then(|mut file| file.write(&scene_mp))
                .expect("failed to write `scene_mp` to file");
        })
        .detach();
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

    if window.cursor_grab_mode() == CursorGrabMode::None {
        if mouse_button.just_pressed(MouseButton::Left) {
            window.set_cursor_grab_mode(CursorGrabMode::Locked);
            window.set_cursor_visibility(false);
        }
    } else {
        if mouse_button.just_pressed(MouseButton::Right) {
            window.set_cursor_grab_mode(CursorGrabMode::None);
            window.set_cursor_visibility(true);
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

        if window.cursor_grab_mode() == CursorGrabMode::None {
            #[cfg(not(target_arch = "wasm32"))]
            app_exit_events.send(AppExit);
        } else {
            window.set_cursor_grab_mode(CursorGrabMode::None);
            window.set_cursor_visibility(true);
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

    if window.cursor_grab_mode() == CursorGrabMode::Locked {
        for mouse_motion in mouse_motion_events.iter() {
            state.direction.0 += -mouse_motion.delta.x * 0.005;
        }
    }

    if key.pressed(KeyCode::Left) || key.pressed(KeyCode::Q) {
        state.direction.0 += 0.0001;
    }
    if key.pressed(KeyCode::Right) || key.pressed(KeyCode::E) {
        state.direction.0 -= 0.0001;
    }

    state.velocity.0.x = 0.0;
    state.velocity.0.y = 0.0;
    state.velocity.0.z = 0.0;

    if key.pressed(KeyCode::Up) || key.pressed(KeyCode::W) {
        state.velocity.0.x -= state.direction.0.sin();
        state.velocity.0.y += state.direction.0.cos();
    }
    if key.pressed(KeyCode::Down) || key.pressed(KeyCode::S) {
        state.velocity.0.x += state.direction.0.sin();
        state.velocity.0.y -= state.direction.0.cos();
    }
    if key.pressed(KeyCode::A) {
        state.velocity.0.x -= state.direction.0.cos();
        state.velocity.0.y -= state.direction.0.sin();
    }
    if key.pressed(KeyCode::D) {
        state.velocity.0.x += state.direction.0.cos();
        state.velocity.0.y += state.direction.0.sin();
    }
    if key.pressed(KeyCode::Space) {
        state.velocity.0.z += 1.0;
    }
    if key.pressed(KeyCode::LControl) {
        state.velocity.0.z -= 1.0;
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
    let view_matrix = Mat3::from_rotation_z(-state.direction.0)
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.y));

    let Ok(current_sector) = sector_query.get(state.current_sector) else { return };

    let mut portal_queue = VecDeque::<Portal>::new();
    let mut y_min_vec = vec![GAP; WIDTH as usize];
    let mut y_max_vec = vec![HEIGHT as isize; WIDTH as usize];

    // Push current sector on portal queue
    portal_queue.push_back(Portal {
        sector: current_sector,
        x_min: GAP,
        x_max: WIDTH as isize,
    });

    // Process all portals until queue is empty, processing a portal may enqueue more
    '_portals: while !portal_queue.is_empty() {
        let self_portal = portal_queue.pop_front().unwrap();
        let sector = self_portal.sector;

        // View relative floor and ceiling locations
        let view_floor = Length(sector.floor.0 - state.position.0.z);
        let view_ceil = Length(sector.ceil.0 - state.position.0.z);

        // Iterate through each wall within the sector
        'walls: for wall in sector.to_walls() {
            // Transform wall ends to view relative positions
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

            // Clip wall by view frustum, will be `None` if outside of frustum
            if let Some((view_left, view_right)) = clip_wall(view_left, view_right) {
                // Project from view to normalized screen coordinates
                let norm_left_top = project(view_left, view_ceil);
                let norm_left_bottom = project(view_left, view_floor);
                let norm_right_top = project(view_right, view_ceil);
                let norm_right_bottom = project(view_right, view_floor);

                // Convert to pixel locations
                let left_top = norm_left_top.to_pixel();
                let left_bottom = norm_left_bottom.to_pixel();
                let right_top = norm_right_top.to_pixel();
                let right_bottom = norm_right_bottom.to_pixel();

                let dx = right_top.x - left_top.x;

                // Skip drawing wall if looking at backside
                if dx <= 0 {
                    continue 'walls;
                }

                // TODO: Use `view_y_middle` in `distance` calculation below
                // let view_y_middle = view_left_bottom.y + (view_y_top - view_left_bottom.y) / 2.0;

                // TODO: Refactor colors to use HSV instead of HSL
                let color_hsla_raw = wall.color.as_hsla_f32();

                // Clip x by portal sides
                let x_left = left_top.x.clamp(self_portal.x_min, self_portal.x_max);
                let x_right = right_top.x.clamp(self_portal.x_min, self_portal.x_max);

                // Fetch adjacent portal sector
                let portal_sector = wall
                    .portal_sector
                    .and_then(|portal_sector_id| sector_query.get(portal_sector_id).ok());

                // Process adjacent portal sector
                let (y_portal_top, y_portal_bottom) = if let Some(portal_sector) = portal_sector {
                    // Push adjacent sector on portal queue to render later
                    portal_queue.push_back(Portal {
                        sector: portal_sector,
                        x_min: x_left,
                        x_max: x_right,
                    });

                    let view_portal_ceil = Length(portal_sector.ceil.0 - state.position.0.z);
                    let view_portal_floor = Length(portal_sector.floor.0 - state.position.0.z);

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

                // Iterate through pixel columns
                '_columns: for x in x_left..x_right {
                    let skip_floor_ceil = x >= self_portal.x_max as isize - GAP;
                    let skip_wall = x >= x_right - GAP;

                    let x_t = (x - left_top.x) as f32 / dx as f32;

                    // Interpolate z for distance
                    let view_z = lerp(view_left.0.y, view_right.0.y, x_t);
                    let distance = view_z.abs();

                    // Lightness for distance
                    let lightness = if distance > FAR {
                        LIGHTNESS_FAR
                    } else if distance < NEAR {
                        LIGHTNESS_NEAR
                    } else {
                        // Interpolate lightness
                        let distance_t = (distance - NEAR) / (FAR - NEAR);
                        lerp(LIGHTNESS_NEAR, LIGHTNESS_FAR, distance_t)
                    };
                    let lightness_rounded = (lightness * 100.0).round() / 100.0;

                    // Color for lightness
                    let color = Color::hsla(
                        color_hsla_raw[0],
                        color_hsla_raw[1],
                        lightness_rounded,
                        color_hsla_raw[3],
                    );

                    // Interpolate y
                    let y_top = lerpi(left_top.y, right_top.y, x_t);
                    let y_bottom = lerpi(left_bottom.y, right_bottom.y, x_t);

                    // Get y bounds
                    let y_min = y_min_vec[x as usize];
                    let y_max = y_max_vec[x as usize];

                    // Clip y
                    let y_top = y_top.clamp(y_min, y_max);
                    let y_bottom = y_bottom.clamp(y_min, y_max);

                    let y_ceil_top = y_min;
                    let y_ceil_bottom = y_top;
                    let y_floor_top = y_bottom;
                    let y_floor_bottom = y_max;

                    // Draw ceiling
                    if !skip_floor_ceil {
                        draw_vertical_line(
                            frame,
                            x,
                            y_ceil_top,
                            y_ceil_bottom - GAP,
                            *CEILING_COLOR,
                        );
                    }

                    // if join_gap_column {
                    //     continue '_columns;
                    // }

                    if portal_sector.is_some() {
                        // Draw wall above portal if required
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

                        // Draw wall below portal if required
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
                    } else {
                        // Draw complete wall
                        if !skip_wall {
                            draw_vertical_line(frame, x, y_top, y_bottom - GAP, color);
                        }
                    }

                    // Draw floor
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
            };
        }
    }
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
    let view_matrix = Mat3::from_rotation_z(-state.direction.0)
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.y));
    let reverse_view_matrix = Mat3::from_translation(vec2(state.position.0.x, state.position.0.y))
        * Mat3::from_rotation_z(state.direction.0);

    // Draw walls
    for sector in sector_query.iter() {
        for wall in sector.to_walls() {
            let view_left = wall.left.transform(view_matrix);
            let view_right = wall.right.transform(view_matrix);

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
                    view_left.to_pixel(),
                    view_right.to_pixel(),
                    view_left_after_clip.to_pixel(),
                    view_right_after_clip.to_pixel(),
                )),
                Minimap::Absolute => {
                    let abs_left = wall.left.transform(reverse_view_matrix);
                    let abs_right = wall.right.transform(reverse_view_matrix);

                    let abs_left_after_clip = view_left_after_clip.transform(reverse_view_matrix);
                    let abs_right_after_clip = view_right_after_clip.transform(reverse_view_matrix);

                    Some((
                        abs_left.to_pixel(),
                        abs_right.to_pixel(),
                        abs_left_after_clip.to_pixel(),
                        abs_right_after_clip.to_pixel(),
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
    let view_player = Position2(vec2(0.0, 0.0));
    let view_near_left = Position2(*LEFT_CLIP_1);
    let view_near_right = Position2(*RIGHT_CLIP_2);
    let view_far_left = Position2(*LEFT_CLIP_2);
    let view_far_right = Position2(*RIGHT_CLIP_1);

    if let Some((player, near_left, near_right, far_left, far_right)) = match state.minimap {
        Minimap::Off => None,
        Minimap::FirstPerson => Some((
            view_player.to_pixel(),
            view_near_left.to_pixel(),
            view_near_right.to_pixel(),
            view_far_left.to_pixel(),
            view_far_right.to_pixel(),
        )),
        Minimap::Absolute => {
            let abs_player = state.position.truncate();
            let abs_near_left = view_near_left.transform(reverse_view_matrix);
            let abs_near_right = view_near_right.transform(reverse_view_matrix);
            let abs_far_left = view_far_left.transform(reverse_view_matrix);
            let abs_far_right = view_far_right.transform(reverse_view_matrix);

            Some((
                abs_player.to_pixel(),
                abs_near_left.to_pixel(),
                abs_near_right.to_pixel(),
                abs_far_left.to_pixel(),
                abs_far_right.to_pixel(),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_pixel(frame, player, *PLAYER_COLOR);
    }
}
