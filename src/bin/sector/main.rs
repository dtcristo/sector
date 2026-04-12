mod draw;
mod utils;

use crate::{draw::*, utils::*};
use sector::*;

use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::message::{MessageReader, MessageWriter},
    input::mouse::MouseMotion,
    input::ButtonInput,
    math::vec2,
    math::vec3,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, WindowResizeConstraints, WindowResolution},
};
use bevy_pixels::prelude::*;
use palette::Hsv;
use sector::map::{map_to_sectors, SectorMap, SectorMapLoader};
use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

#[macro_use]
extern crate lazy_static;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const WINDOW_SCALE: u32 = 4;
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

impl From<Normalized> for Pixel {
    fn from(norm: Normalized) -> Self {
        Self {
            x: FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * norm.0.x).round() as isize,
            y: FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * norm.0.y).round() as isize,
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

#[derive(Debug, PartialEq)]
enum Minimap {
    Off,
    FirstPerson,
    Absolute,
}

#[derive(Resource, Debug, PartialEq)]
struct MinimapMode(Minimap);

#[derive(Resource, Debug)]
struct WindowTitleTimer(Timer);

#[derive(Resource, Debug)]
struct MapHandle(Handle<SectorMap>);

#[derive(Resource, Debug, Default)]
struct MapSpawnState {
    spawned: bool,
}

#[derive(Component, Debug)]
struct Player {
    position: Position3,
    velocity: Velocity,
    direction: Direction,
    current_sector: Option<SectorId>,
}

struct SectorRuntimePlugin;

impl Plugin for SectorRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SectorMap>()
            .init_asset_loader::<SectorMapLoader>()
            .insert_resource(MinimapMode(Minimap::Off))
            .insert_resource(WindowTitleTimer(Timer::new(
                Duration::from_millis(500),
                TimerMode::Repeating,
            )))
            .insert_resource(MapSpawnState::default())
            .add_systems(Startup, (setup_player_system, queue_map_load_system))
            .add_systems(
                Update,
                (
                    spawn_loaded_map_system,
                    update_title_system,
                    mouse_capture_system,
                    escape_system,
                    switch_minimap_system,
                    (
                        player_look_system,
                        player_translation_input_system,
                        move_player_system,
                        update_current_sector_system,
                    )
                        .chain(),
                ),
            )
            .add_systems(
                Draw,
                (
                    draw_background_system,
                    draw_wall_system,
                    draw_minimap_system,
                )
                    .chain(),
            );
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    App::new()
        .register_type::<SectorId>()
        .register_type::<Option<SectorId>>()
        .register_type::<Vec<Option<SectorId>>>()
        .register_type::<Sector>()
        .register_type::<InitialSector>()
        .register_type::<Sector>()
        .register_type::<Position2>()
        .register_type::<Vec<Position2>>()
        .register_type::<Length>()
        .register_type::<RawColor>()
        .register_type::<Vec<RawColor>>()
        .register_type::<[u8; 3]>()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    watch_for_changes_override: Some(true),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "sector".to_string(),
                        resolution: WindowResolution::new(
                            WINDOW_SCALE * WIDTH,
                            WINDOW_SCALE * HEIGHT,
                        ),
                        resize_constraints: WindowResizeConstraints {
                            min_width: WIDTH as f32,
                            min_height: HEIGHT as f32,
                            ..default()
                        },
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PixelsPlugin {
            primary_window: Some(PixelsOptions {
                width: WIDTH,
                height: HEIGHT,
                auto_resize_buffer: false,
                ..default()
            }),
        })
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(SectorRuntimePlugin)
        .run();
}

fn setup_player_system(mut commands: Commands) {
    commands.spawn(Player {
        position: Position3(vec3(0.0, 0.0, 2.0)),
        velocity: Velocity(vec3(0.0, 0.0, 0.0)),
        direction: Direction(0.0),
        current_sector: None,
    });
}

fn queue_map_load_system(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(MapHandle(asset_server.load(DEFAULT_MAP_FILE_PATH)));
}

fn spawn_loaded_map_system(
    mut commands: Commands,
    map_handle: Res<MapHandle>,
    maps: Res<Assets<SectorMap>>,
    mut map_spawn_state: ResMut<MapSpawnState>,
    mut player_query: Query<&mut Player>,
) {
    if map_spawn_state.spawned {
        return;
    }

    let Some(map) = maps.get(&map_handle.0) else {
        return;
    };

    let (initial_sector, sectors) =
        map_to_sectors(map).expect("loaded map asset should be structurally valid");

    if let Ok(mut player) = player_query.single_mut() {
        player.current_sector = Some(initial_sector.0);
    }

    commands.spawn(initial_sector);
    for sector in sectors {
        commands.spawn(sector);
    }

    map_spawn_state.spawned = true;
}

fn update_title_system(
    mut title_timer: ResMut<WindowTitleTimer>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut window_query: Query<&mut Window>,
) {
    if title_timer.0.tick(time.delta()).is_finished() {
        let Ok(mut window) = window_query.single_mut() else {
            return;
        };

        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.value() {
                window.title = format!("sector: {value:.0} fps");
            }
        }
    }
}

fn mouse_capture_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut cursor_options_query: Query<&mut CursorOptions>,
) {
    let Ok(mut cursor_options) = cursor_options_query.single_mut() else {
        return;
    };

    if cursor_options.grab_mode == CursorGrabMode::None {
        if mouse_button.just_pressed(MouseButton::Left) {
            cursor_options.grab_mode = CursorGrabMode::Locked;
            cursor_options.visible = false;
        }
    } else {
        if mouse_button.just_pressed(MouseButton::Right) {
            cursor_options.grab_mode = CursorGrabMode::None;
            cursor_options.visible = true;
        }
    }
}

fn escape_system(
    mut app_exit_events: MessageWriter<AppExit>,
    key: Res<ButtonInput<KeyCode>>,
    mut cursor_options_query: Query<&mut CursorOptions>,
) {
    if key.just_pressed(KeyCode::Escape) {
        let Ok(mut cursor_options) = cursor_options_query.single_mut() else {
            return;
        };

        if cursor_options.grab_mode == CursorGrabMode::None {
            #[cfg(not(target_arch = "wasm32"))]
            app_exit_events.write(AppExit::Success);
        } else {
            cursor_options.grab_mode = CursorGrabMode::None;
            cursor_options.visible = true;
        }
    }
}

fn switch_minimap_system(mut minimap: ResMut<MinimapMode>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Tab) {
        minimap.0 = match minimap.0 {
            Minimap::Off => Minimap::FirstPerson,
            Minimap::FirstPerson => Minimap::Absolute,
            Minimap::Absolute => Minimap::Off,
        }
    }
}

fn player_look_system(
    mut player_query: Query<&mut Player>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    key: Res<ButtonInput<KeyCode>>,
    cursor_options_query: Query<&CursorOptions>,
) {
    let Ok(cursor_options) = cursor_options_query.single() else {
        return;
    };
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };

    if cursor_options.grab_mode == CursorGrabMode::Locked {
        for mouse_motion in mouse_motion_events.read() {
            player.direction.0 += -mouse_motion.delta.x * 0.005;
        }
    }

    if key.pressed(KeyCode::ArrowLeft) || key.pressed(KeyCode::KeyQ) {
        player.direction.0 += 0.0001;
    }
    if key.pressed(KeyCode::ArrowRight) || key.pressed(KeyCode::KeyE) {
        player.direction.0 -= 0.0001;
    }
}

fn player_translation_input_system(
    mut player_query: Query<&mut Player>,
    key: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };

    player.velocity.0 = Vec3::ZERO;

    if key.pressed(KeyCode::ArrowUp) || key.pressed(KeyCode::KeyW) {
        player.velocity.0.x -= player.direction.0.sin();
        player.velocity.0.y += player.direction.0.cos();
    }
    if key.pressed(KeyCode::ArrowDown) || key.pressed(KeyCode::KeyS) {
        player.velocity.0.x += player.direction.0.sin();
        player.velocity.0.y -= player.direction.0.cos();
    }
    if key.pressed(KeyCode::KeyA) {
        player.velocity.0.x -= player.direction.0.cos();
        player.velocity.0.y -= player.direction.0.sin();
    }
    if key.pressed(KeyCode::KeyD) {
        player.velocity.0.x += player.direction.0.cos();
        player.velocity.0.y += player.direction.0.sin();
    }
    if key.pressed(KeyCode::Space) {
        player.velocity.0.z += 1.0;
    }
    if key.pressed(KeyCode::ControlLeft) {
        player.velocity.0.z -= 1.0;
    }
}

fn move_player_system(mut player_query: Query<&mut Player>) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };

    let velocity = player.velocity.0;
    player.position.0 += 0.05 * velocity;
}

fn update_current_sector_system(
    mut player_query: Query<&mut Player>,
    sector_query: Query<&Sector>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };

    if let Some(sector_id) = sector_query
        .iter()
        .find(|sector| sector_contains_position(sector, player.position))
        .map(|sector| sector.id)
    {
        player.current_sector = Some(sector_id);
    }
}

fn sector_contains_position(sector: &Sector, position: Position3) -> bool {
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

fn draw_background_system(mut wrapper_query: Query<&mut PixelsWrapper>) {
    let Ok(mut wrapper) = wrapper_query.single_mut() else {
        return;
    };
    let frame = wrapper.pixels.frame_mut();

    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_wall_system(
    player_query: Query<&Player>,
    mut wrapper_query: Query<&mut PixelsWrapper>,
    sector_query: Query<&Sector>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };
    let sectors_by_id: HashMap<_, _> = sector_query
        .iter()
        .map(|sector| (sector.id, sector))
        .collect();
    let Some(current_sector) = player
        .current_sector
        .and_then(|id| sectors_by_id.get(&id).copied())
    else {
        return;
    };

    let Ok(mut wrapper) = wrapper_query.single_mut() else {
        return;
    };
    let frame = wrapper.pixels.frame_mut();
    let view_matrix = Mat3::from_rotation_z(-player.direction.0)
        * Mat3::from_translation(-vec2(player.position.0.x, player.position.0.y));

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
        let view_floor = Length(sector.floor.0 - player.position.0.z);
        let view_ceil = Length(sector.ceil.0 - player.position.0.z);

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
                let left_top: Pixel = norm_left_top.into();
                let left_bottom: Pixel = norm_left_bottom.into();
                let right_top: Pixel = norm_right_top.into();
                let right_bottom: Pixel = norm_right_bottom.into();

                let dx = right_top.x - left_top.x;

                // Skip drawing wall if looking at backside
                if dx <= 0 {
                    continue 'walls;
                }

                // TODO: Use `view_y_middle` in `distance` calculation below
                // let view_y_middle = view_left_bottom.y + (view_y_top - view_left_bottom.y) / 2.0;

                // Clip x by portal sides
                let x_left = left_top.x.clamp(self_portal.x_min, self_portal.x_max);
                let x_right = right_top.x.clamp(self_portal.x_min, self_portal.x_max);

                // Fetch adjacent portal sector
                let portal_sector = wall
                    .portal_sector
                    .and_then(|id| sectors_by_id.get(&id).copied());

                // Process adjacent portal sector
                let (y_portal_top, y_portal_bottom) = if let Some(portal_sector) = portal_sector {
                    // Push adjacent sector on portal queue to render later
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

                // Iterate through pixel columns
                '_columns: for x in x_left..x_right {
                    let skip_floor_ceil = x >= self_portal.x_max as isize - GAP;
                    let skip_wall = x >= x_right - GAP;

                    let x_t = (x - left_top.x) as f32 / dx as f32;

                    // Interpolate z for distance
                    let view_z = lerp(view_left.0.y, view_right.0.y, x_t);
                    let distance = view_z.abs();

                    // Brightness for distance
                    let brightness = if distance > FAR {
                        BRIGHTNESS_FAR
                    } else if distance < NEAR {
                        BRIGHTNESS_NEAR
                    } else {
                        // Interpolate brightness
                        let distance_t = (distance - NEAR) / (FAR - NEAR);
                        lerp(BRIGHTNESS_NEAR, BRIGHTNESS_FAR, distance_t)
                    };
                    let brightness_rounded = (brightness * 100.0).round() / 100.0;

                    // Color for brightness
                    let color: RawColor =
                        Hsv::new(wall.color.hue, wall.color.saturation, brightness_rounded).into();

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
    minimap: Res<MinimapMode>,
    player_query: Query<&Player>,
    mut wrapper_query: Query<&mut PixelsWrapper>,
    sector_query: Query<&Sector>,
) {
    if minimap.0 == Minimap::Off {
        return;
    }

    let Ok(player) = player_query.single() else {
        return;
    };

    let Ok(mut wrapper) = wrapper_query.single_mut() else {
        return;
    };
    let frame = wrapper.pixels.frame_mut();
    let view_matrix = Mat3::from_rotation_z(-player.direction.0)
        * Mat3::from_translation(-vec2(player.position.0.x, player.position.0.y));
    let reverse_view_matrix =
        Mat3::from_translation(vec2(player.position.0.x, player.position.0.y))
            * Mat3::from_rotation_z(player.direction.0);

    // Draw walls
    for sector in &sector_query {
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

            if let Some((left, right, left_after_clip, right_after_clip)) = match minimap.0 {
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

    // Draw frustum and player
    let view_player = Position2(vec2(0.0, 0.0));
    let view_near_left = Position2(*LEFT_CLIP_1);
    let view_near_right = Position2(*RIGHT_CLIP_2);
    let view_far_left = Position2(*LEFT_CLIP_2);
    let view_far_right = Position2(*RIGHT_CLIP_1);

    if let Some((player_pixel, near_left, near_right, far_left, far_right)) = match minimap.0 {
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
