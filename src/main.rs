mod draw;
mod pixel;
mod utils;

use crate::{draw::*, pixel::*, utils::*};

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
use std::fs::File;
use std::io::Write;

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
const LIGHTNESS_NEAR: f32 = 0.5;
const LIGHTNESS_FAR: f32 = 0.0;
const MINIMAP_SCALE: f32 = 8.0;
const DEFAULT_SCENE_RON_FILE_PATH: &str = "scenes/default.scn.ron";
const DEFAULT_SCENE_MP_FILE_PATH: &str = "scenes/default.scn.mp";

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_rh(*FOV_Y_RADIANS, ASPECT_RATIO, -Z_NEAR);
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

#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
pub struct Sector {
    vertices: Vec<Vertex>,
    adj_sectors: Vec<Option<Entity>>,
    colors: Vec<Color>,
    floor: Length,
    ceil: Length,
}

impl Sector {
    fn to_walls(&self) -> Vec<Wall> {
        let mut walls = Vec::with_capacity(self.vertices.len());

        let mut vertex_iter = self.vertices.iter();
        let mut adj_sector_iter = self.adj_sectors.iter();
        let mut color_iter = self.colors.iter();

        let Some(&initial) = vertex_iter.next() else { return walls };

        let mut add_wall = |left: Vertex, right: Vertex| {
            walls.push(Wall {
                left,
                right,
                adj_sector: *adj_sector_iter.next().unwrap_or(&None),
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

struct Wall {
    left: Vertex,
    right: Vertex,
    adj_sector: Option<Entity>,
    color: Color,
}

#[derive(Reflect, Debug, Copy, Clone, Default)]
pub struct Length(f32);

// Position (https://bevy-cheatbook.github.io/features/coords.html)
// +y.---> +x
//   |
//   v
//   +z
#[derive(Debug, Copy, Clone)]
struct Position(Vec3);

#[derive(Reflect, FromReflect, Debug, Copy, Clone, Default)]
pub struct Vertex {
    x: f32,
    y: f32,
}

impl Vertex {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<Vec2> for Vertex {
    fn from(v: Vec2) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<Vertex> for Vec2 {
    fn from(v: Vertex) -> Self {
        vec2(v.x, v.y)
    }
}

#[derive(Debug, Copy, Clone)]
struct Velocity(Vec3);

// Direction
//   ^   ^
//    \+θ|
//     \ |
//       .
#[derive(Debug, Copy, Clone)]
struct Direction(f32);

#[derive(Debug, PartialEq)]
enum Minimap {
    Off,
    FirstPerson,
    Absolute,
}

#[derive(Resource, Debug)]
struct AppState {
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
        .register_type::<Sector>()
        .register_type::<Vertex>()
        .register_type::<Length>()
        .register_type::<Option<Entity>>()
        .register_type::<Color>()
        .insert_resource(AppState {
            minimap: Minimap::Off,
            position: Position(vec3(0.0, 0.0, 2.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
            current_sector: Entity::from_raw(u32::MAX), // Initial invalid Entity, correctly set within setup
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
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
    let v0 = Vertex::new(-4.0, -10.0);
    let v1 = Vertex::new(-2.0, -10.0);
    let v2 = Vertex::new(2.0, -5.0);
    let v3 = Vertex::new(4.0, -1.0);
    let v4 = Vertex::new(4.0, 8.0);
    let v5 = Vertex::new(-11.0, 8.0);
    let v6 = Vertex::new(-4.0, -15.0);
    let v7 = Vertex::new(4.0, -15.0);

    // Sectors
    let s0 = world.spawn_empty().id();
    let s1 = world.spawn_empty().id();

    // Get mutable `AppState` resource
    let mut state = world.resource_mut::<AppState>();

    // Player starts in sector 0
    state.current_sector = s0;

    world.entity_mut(s0).insert(Sector {
        vertices: vec![v0, v1, v2, v3, v4, v5],
        adj_sectors: vec![None, Some(s1), None, None, None, None],
        colors: vec![
            Color::BLUE,
            Color::RED,
            Color::GREEN,
            Color::ORANGE,
            Color::FUCHSIA,
            Color::YELLOW,
        ],
        floor: Length(0.0),
        ceil: Length(4.0),
    });

    world.entity_mut(s1).insert(Sector {
        vertices: vec![v2, v1, v6, v7],
        adj_sectors: vec![Some(s0), None, None, None],
        colors: vec![Color::RED, Color::YELLOW, Color::GREEN, Color::FUCHSIA],
        floor: Length(0.25),
        ceil: Length(3.75),
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
        state.direction.0 += 0.05;
    }
    if key.pressed(KeyCode::Right) || key.pressed(KeyCode::E) {
        state.direction.0 -= 0.05;
    }

    state.velocity.0.x = 0.0;
    state.velocity.0.y = 0.0;
    state.velocity.0.z = 0.0;

    if key.pressed(KeyCode::Up) || key.pressed(KeyCode::W) {
        state.velocity.0.x -= state.direction.0.sin();
        state.velocity.0.y -= state.direction.0.cos();
    }
    if key.pressed(KeyCode::Down) || key.pressed(KeyCode::S) {
        state.velocity.0.x += state.direction.0.sin();
        state.velocity.0.y += state.direction.0.cos();
    }
    if key.pressed(KeyCode::A) {
        state.velocity.0.x -= state.direction.0.cos();
        state.velocity.0.y += state.direction.0.sin();
    }
    if key.pressed(KeyCode::D) {
        state.velocity.0.x += state.direction.0.cos();
        state.velocity.0.y -= state.direction.0.sin();
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

    let x_min = EDGE_GAP;
    let x_max = WIDTH_MINUS_EDGE_GAP;
    let y_min_vec = vec![EDGE_GAP; WIDTH as usize];
    let y_max_vec = vec![HEIGHT_MINUS_EDGE_GAP as isize; WIDTH as usize];

    let Ok(sector) = sector_query.get(state.current_sector) else { return };

    let view_floor = Length(sector.floor.0 - state.position.0.z);
    let view_ceil = Length(sector.ceil.0 - state.position.0.z);

    let view_matrix = Mat3::from_rotation_z(state.direction.0)
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.y));

    for wall in sector.to_walls() {
        let view_left = view_matrix.transform_point2(wall.left.into()).into();
        let view_right = view_matrix.transform_point2(wall.right.into()).into();

        if let Some((view_left, view_right)) = clip_wall(view_left, view_right) {
            let norm_left_top = project(vec3(view_left.x, view_ceil.0, view_left.y));
            let norm_left_bottom = project(vec3(view_left.x, view_floor.0, view_left.y));
            let norm_right_top = project(vec3(view_right.x, view_ceil.0, view_right.y));
            let norm_right_bottom = project(vec3(view_right.x, view_floor.0, view_right.y));

            let left_top = Pixel::from_norm(norm_left_top.truncate());
            let left_bottom = Pixel::from_norm(norm_left_bottom.truncate());
            let right_top = Pixel::from_norm(norm_right_top.truncate());
            let right_bottom = Pixel::from_norm(norm_right_bottom.truncate());

            let dx = right_top.x - left_top.x;

            // Skip drawing backside
            if dx <= 0 {
                return;
            }

            let adj_sector = wall
                .adj_sector
                .and_then(|adj_sector_id| sector_query.get(adj_sector_id).ok());

            let (adj_top_y, adj_bottom_y) = if let Some(adj_sector) = adj_sector {
                let view_adj_ceil = Length(adj_sector.ceil.0 - state.position.0.z);
                let view_adj_floor = Length(adj_sector.floor.0 - state.position.0.z);

                let adj_top_y = if view_adj_ceil.0 < view_ceil.0 {
                    let adj_ceil_t = (view_adj_ceil.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                    Some((
                        lerpi(left_top.y, left_bottom.y, adj_ceil_t),
                        lerpi(right_top.y, right_bottom.y, adj_ceil_t),
                    ))
                } else {
                    None
                };

                let adj_bottom_y = if view_adj_floor.0 > view_floor.0 {
                    let adj_floor_t =
                        (view_adj_floor.0 - view_ceil.0) / (view_floor.0 - view_ceil.0);
                    Some((
                        lerpi(left_top.y, left_bottom.y, adj_floor_t),
                        lerpi(right_top.y, right_bottom.y, adj_floor_t),
                    ))
                } else {
                    None
                };

                (adj_top_y, adj_bottom_y)
            } else {
                (None, None)
            };

            // TODO: Use `view_y_middle` in `distance` calculation below
            // let view_y_middle = view_left_bottom.y + (view_y_top - view_left_bottom.y) / 2.0;

            // TODO: Refactor colors to use HSV instead of HSL
            let color_hsla_raw = wall.color.as_hsla_f32();

            // Clip x
            let x_left = x_min.max(left_top.x);
            let x_right = right_top.x.min(x_max);

            for x in x_left..(x_right - JOIN_GAP) {
                let x_t = (x - left_top.x) as f32 / dx as f32;

                // Interpolate z for distance
                let view_z = lerp(view_left.y, view_right.y, x_t);
                let distance = view_z.abs();

                // Lightness for distance
                let lightness = if distance > LIGHTNESS_DISTANCE_FAR {
                    LIGHTNESS_FAR
                } else if distance < LIGHTNESS_DISTANCE_NEAR {
                    LIGHTNESS_NEAR
                } else {
                    // Interpolate lightness
                    let distance_t = (distance - LIGHTNESS_DISTANCE_NEAR)
                        / (LIGHTNESS_DISTANCE_FAR - LIGHTNESS_DISTANCE_NEAR);
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
                let y_top = y_min.max(y_top);
                let y_bottom = y_bottom.min(y_max);

                // Draw ceiling
                draw_vertical_line(
                    frame,
                    x,
                    y_min,
                    (y_top - JOIN_GAP).min(y_max),
                    *CEILING_COLOR,
                );

                match adj_sector {
                    Some(_adj_sector) => {
                        // Draw adjacent ceiling wall
                        if let Some((adj_left_top_y, adj_right_top_y)) = adj_top_y {
                            let y_adj_top = lerpi(adj_left_top_y, adj_right_top_y, x_t);
                            draw_vertical_line(
                                frame,
                                x,
                                y_top,
                                (y_adj_top - JOIN_GAP).min(y_bottom - JOIN_GAP),
                                color,
                            )
                        }

                        // Draw adjacent floor wall
                        if let Some((adj_left_bottom_y, adj_right_bottom_y)) = adj_bottom_y {
                            let y_adj_bottom = lerpi(adj_left_bottom_y, adj_right_bottom_y, x_t);
                            draw_vertical_line(
                                frame,
                                x,
                                y_top.max(y_adj_bottom),
                                y_bottom - JOIN_GAP,
                                color,
                            )
                        }
                    }
                    // Draw complete wall
                    None => draw_vertical_line(frame, x, y_top, y_bottom - JOIN_GAP, color),
                }

                // Draw floor
                draw_vertical_line(frame, x, y_min.max(y_bottom), y_max, *FLOOR_COLOR);
            }
        };
    }
}

fn clip_wall(mut view_left: Vertex, mut view_right: Vertex) -> Option<(Vertex, Vertex)> {
    // Skip entirely behind back
    if view_left.y > Z_NEAR && view_right.y > Z_NEAR {
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
    if view_left.y > Z_NEAR || view_right.y > Z_NEAR {
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
        * Mat3::from_translation(-vec2(state.position.0.x, state.position.0.y));
    let reverse_view_matrix = Mat3::from_translation(vec2(state.position.0.x, state.position.0.y))
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
                    Pixel::from_abs(view_left.into()),
                    Pixel::from_abs(view_right.into()),
                    Pixel::from_abs(view_left_after_clip.into()),
                    Pixel::from_abs(view_right_after_clip.into()),
                )),
                Minimap::Absolute => {
                    let abs_left = reverse_view_matrix.transform_point2(view_left.into());
                    let abs_right = reverse_view_matrix.transform_point2(view_right.into());
                    let abs_left_after_clip =
                        reverse_view_matrix.transform_point2(view_left_after_clip.into());
                    let abs_right_after_clip =
                        reverse_view_matrix.transform_point2(view_right_after_clip.into());

                    Some((
                        Pixel::from_abs(abs_left),
                        Pixel::from_abs(abs_right),
                        Pixel::from_abs(abs_left_after_clip),
                        Pixel::from_abs(abs_right_after_clip),
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
            Pixel::from_abs(view_player),
            Pixel::from_abs(view_near_left),
            Pixel::from_abs(view_near_right),
            Pixel::from_abs(view_far_left),
            Pixel::from_abs(view_far_right),
        )),
        Minimap::Absolute => {
            let abs_player = vec2(state.position.0.x, state.position.0.y);
            let abs_near_left = reverse_view_matrix.transform_point2(view_near_left);
            let abs_near_right = reverse_view_matrix.transform_point2(view_near_right);
            let abs_far_left = reverse_view_matrix.transform_point2(view_far_left);
            let abs_far_right = reverse_view_matrix.transform_point2(view_far_right);

            Some((
                Pixel::from_abs(abs_player),
                Pixel::from_abs(abs_near_left),
                Pixel::from_abs(abs_near_right),
                Pixel::from_abs(abs_far_left),
                Pixel::from_abs(abs_far_right),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, *FRUSTUM_COLOR);
        draw_line(frame, near_right, far_right, *FRUSTUM_COLOR);
        draw_line(frame, near_left, near_right, *FRUSTUM_COLOR);
        draw_pixel(frame, player, *PLAYER_COLOR);
    }
}
