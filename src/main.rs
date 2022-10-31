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
use bevy_render::color::Color as BevyColor;

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
    static ref BACK_CLIP_1: Vec2 = vec2(*X_NEAR, Z_NEAR);
    static ref BACK_CLIP_2: Vec2 = vec2(-*X_NEAR, Z_NEAR);
    static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_2;
    static ref LEFT_CLIP_2: Vec2 = vec2(-*X_FAR, Z_FAR);
    static ref RIGHT_CLIP_1: Vec2 = vec2(*X_FAR, Z_FAR);
    static ref RIGHT_CLIP_2: Vec2 = *BACK_CLIP_1;
}

#[derive(Component, Bundle, Debug)]
pub struct Wall {
    pub left: Position,
    pub right: Position,
    pub height: Length,
    pub color: Color,
}

#[derive(Component, Debug, Copy, Clone)]
pub struct Length(f32);

// Position (https://bevy-cheatbook.github.io/features/coords.html)
// +y.---> +x
//   |
//   v
//   +z
#[derive(Component, Debug, Copy, Clone)]
pub struct Position(Vec3);

#[derive(Debug, Copy, Clone)]
pub struct Velocity(Vec3);

// Direction
//   ^   ^
//    \+θ|
//     \ |
//       .
#[derive(Debug, Copy, Clone)]
pub struct Direction(f32);

#[derive(Component, Debug, Copy, Clone)]
pub struct Color(u8, u8, u8, u8);

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

fn setup_system(mut commands: Commands) {
    commands.spawn().insert(Wall {
        left: Position(vec3(-4.0, 0.0, -10.0)),
        right: Position(vec3(4.0, 0.0, -5.0)),
        height: Length(4.0),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert(Wall {
        left: Position(vec3(4.0, 0.0, -5.0)),
        right: Position(vec3(4.0, 0.0, 8.0)),
        height: Length(4.0),
        color: Color(0x00, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert(Wall {
        left: Position(vec3(4.0, 0.0, 8.0)),
        right: Position(vec3(-11.0, 0.0, 8.0)),
        height: Length(4.0),
        color: Color(0x00, 0x00, 0xff, 0xff),
    });

    commands.spawn().insert(Wall {
        left: Position(vec3(-11.0, 0.0, 8.0)),
        right: Position(vec3(-4.0, 0.0, -10.0)),
        height: Length(4.0),
        color: Color(0xff, 0x00, 0xff, 0xff),
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

fn switch_minimap_system(key: Res<Input<KeyCode>>, mut state: ResMut<AppState>) {
    if key.just_pressed(KeyCode::Tab) {
        state.minimap = match state.minimap {
            Minimap::Off => Minimap::FirstPerson,
            Minimap::FirstPerson => Minimap::Absolute,
            Minimap::Absolute => Minimap::Off,
        }
    }
}

fn player_movement_system(
    windows: Res<Windows>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    key: Res<Input<KeyCode>>,
    mut state: ResMut<AppState>,
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
    query: Query<&Wall>,
    state: Res<AppState>,
) {
    let frame = pixels_resource.pixels.get_frame_mut();
    let view_matrix =
        Mat4::from_rotation_y(-state.direction.0) * Mat4::from_translation(-state.position.0);

    for wall in query.iter() {
        let wall_left_top = vec3(wall.left.0.x, wall.height.0, wall.left.0.z);
        let wall_left_bottom = wall.left.0;
        let wall_right_top = vec3(wall.right.0.x, wall.height.0, wall.right.0.z);
        let wall_right_bottom = wall.right.0;

        let view_left_top = view_matrix.transform_point3(wall_left_top);
        let view_left_bottom = view_matrix.transform_point3(wall_left_bottom);
        let view_right_top = view_matrix.transform_point3(wall_right_top);
        let view_right_bottom = view_matrix.transform_point3(wall_right_bottom);

        println!("\n...");
        // println!("before clip");
        // dbg!(view_left_top);
        // dbg!(view_left_bottom);
        // dbg!(view_right_top);
        // dbg!(view_right_bottom);

        if let Some((view_left_top, view_left_bottom, view_right_top, view_right_bottom)) =
            clip_wall(
                view_left_top,
                view_left_bottom,
                view_right_top,
                view_right_bottom,
            )
        {
            // println!("after clip");
            // dbg!(view_left_top);
            // dbg!(view_left_bottom);
            // dbg!(view_right_top);
            // dbg!(view_right_bottom);

            draw_wall(
                frame,
                view_left_top,
                view_left_bottom,
                view_right_top,
                view_right_bottom,
                wall.color,
            );
        };
    }
}

fn clip_wall(
    mut view_left_top: Vec3,
    mut view_left_bottom: Vec3,
    mut view_right_top: Vec3,
    mut view_right_bottom: Vec3,
) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
    // Skip entirely behind back
    if view_left_top.z > Z_NEAR && view_right_top.z > Z_NEAR {
        return None;
    }

    // Clip left side
    if let Some(intersection) =
        intersect_xz(view_left_top, view_right_top, *LEFT_CLIP_1, *LEFT_CLIP_2)
    {
        if intersection.x < -*X_NEAR {
            if point_behind_xz(view_left_top, *LEFT_CLIP_1, *LEFT_CLIP_2) {
                view_left_top = intersection;
                view_left_bottom.x = view_left_top.x;
                view_left_bottom.z = view_left_top.z;
            } else {
                view_right_top = intersection;
                view_right_bottom.x = view_right_top.x;
                view_right_bottom.z = view_right_top.z;
            }
        }
    }

    // Clip right side
    if let Some(intersection) =
        intersect_xz(view_left_top, view_right_top, *RIGHT_CLIP_1, *RIGHT_CLIP_2)
    {
        if intersection.x > *X_NEAR {
            if point_behind_xz(view_left_top, *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
                view_left_top = intersection;
                view_left_bottom.x = view_left_top.x;
                view_left_bottom.z = view_left_top.z;
            } else {
                view_right_top = intersection;
                view_right_bottom.x = view_right_top.x;
                view_right_bottom.z = view_right_top.z;
            }
        }
    }

    // Clip behind back
    if view_left_top.z > Z_NEAR || view_right_top.z > Z_NEAR {
        if let Some(intersection) =
            intersect_xz(view_left_top, view_right_top, *BACK_CLIP_1, *BACK_CLIP_2)
        {
            if point_behind_xz(view_left_top, *BACK_CLIP_1, *BACK_CLIP_2) {
                view_left_top = intersection;
                view_left_bottom.x = view_left_top.x;
                view_left_bottom.z = view_left_top.z;
            } else {
                view_right_top = intersection;
                view_right_bottom.x = view_right_top.x;
                view_right_bottom.z = view_right_top.z;
            }
        }
    }

    // Skip entirely behind left side
    if point_behind_xz(view_left_top, *LEFT_CLIP_1, *LEFT_CLIP_2) {
        return None;
    }

    // Skip entirely behind right side
    if point_behind_xz(view_left_top, *RIGHT_CLIP_1, *RIGHT_CLIP_2) {
        return None;
    }

    Some((
        view_left_top,
        view_left_bottom,
        view_right_top,
        view_right_bottom,
    ))
}

fn intersect_xz(a1: Vec3, a2: Vec3, b1: Vec2, b2: Vec2) -> Option<Vec3> {
    if let Some(Vec2 { x, y: z }) = intersect(vec2(a1.x, a1.z), vec2(a2.x, a2.z), b1, b2) {
        Some(vec3(x, a1.y, z))
    } else {
        None
    }
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

fn point_behind_xz(point: Vec3, a: Vec2, b: Vec2) -> bool {
    point_behind(vec2(point.x, point.z), a, b)
}

fn point_behind(point: Vec2, a: Vec2, b: Vec2) -> bool {
    vec2(b.x - a.x, b.y - a.y).perp_dot(vec2(point.x - a.x, point.y - a.y)) < 0.0
}

fn draw_minimap_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<&Wall>,
    state: Res<AppState>,
) {
    if state.minimap == Minimap::Off {
        return;
    }

    let frame = pixels_resource.pixels.get_frame_mut();
    let view_matrix =
        Mat4::from_rotation_y(-state.direction.0) * Mat4::from_translation(-state.position.0);
    let reverse_view_matrix =
        Mat4::from_translation(state.position.0) * Mat4::from_rotation_y(state.direction.0);

    let wall_clipped_color = Color(0xff, 0xff, 0xff, 0xff);
    let frustum_color = Color(0x88, 0x88, 0x88, 0xff);
    let player_color = Color(0xff, 0x00, 0x00, 0xff);

    // Draw walls
    for wall in query.iter() {
        let wall_left_top = vec3(wall.left.0.x, wall.height.0, wall.left.0.z);
        let wall_right_top = vec3(wall.right.0.x, wall.height.0, wall.right.0.z);

        let view_left_top = view_matrix.transform_point3(wall_left_top);
        let view_right_top = view_matrix.transform_point3(wall_right_top);

        let mut view_left_top_after_clip = view_left_top;
        let mut view_right_top_after_clip = view_right_top;

        let clipping = clip_wall(view_left_top, view_left_top, view_right_top, view_right_top);
        if let Some((l, _, r, _)) = clipping {
            view_left_top_after_clip = l;
            view_right_top_after_clip = r;
        }

        if let Some((left_top, right_top, left_top_after_clip, right_top_after_clip)) =
            match state.minimap {
                Minimap::Off => None,
                Minimap::FirstPerson => Some((
                    Pixel::from_absolute(view_left_top),
                    Pixel::from_absolute(view_right_top),
                    Pixel::from_absolute(view_left_top_after_clip),
                    Pixel::from_absolute(view_right_top_after_clip),
                )),
                Minimap::Absolute => {
                    let absolute_left_top = reverse_view_matrix.transform_point3(view_left_top);
                    let absolute_right_top = reverse_view_matrix.transform_point3(view_right_top);
                    let absolute_left_top_after_clip =
                        reverse_view_matrix.transform_point3(view_left_top_after_clip);
                    let absolute_right_top_after_clip =
                        reverse_view_matrix.transform_point3(view_right_top_after_clip);

                    Some((
                        Pixel::from_absolute(absolute_left_top),
                        Pixel::from_absolute(absolute_right_top),
                        Pixel::from_absolute(absolute_left_top_after_clip),
                        Pixel::from_absolute(absolute_right_top_after_clip),
                    ))
                }
            }
        {
            if clipping.is_none() {
                draw_line(frame, left_top, right_top, wall_clipped_color);
                continue;
            }
            if left_top_after_clip != left_top {
                draw_line(frame, left_top, left_top_after_clip, wall_clipped_color);
            }
            if right_top_after_clip != right_top {
                draw_line(frame, right_top_after_clip, right_top, wall_clipped_color);
            }
            draw_line(frame, left_top_after_clip, right_top_after_clip, wall.color);
        }
    }

    // Draw frustum and player
    let view_player = vec3(0.0, 0.0, 0.0);
    let view_near_left = vec3(-*X_NEAR, 0.0, Z_NEAR);
    let view_near_right = vec3(*X_NEAR, 0.0, Z_NEAR);
    let view_far_left = vec3(-*X_FAR, 0.0, Z_FAR);
    let view_far_right = vec3(*X_FAR, 0.0, Z_FAR);

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
            let absolute_player = state.position.0;
            let absolute_near_left = reverse_view_matrix.transform_point3(view_near_left);
            let absolute_near_right = reverse_view_matrix.transform_point3(view_near_right);
            let absolute_far_left = reverse_view_matrix.transform_point3(view_far_left);
            let absolute_far_right = reverse_view_matrix.transform_point3(view_far_right);

            Some((
                Pixel::from_absolute(absolute_player),
                Pixel::from_absolute(absolute_near_left),
                Pixel::from_absolute(absolute_near_right),
                Pixel::from_absolute(absolute_far_left),
                Pixel::from_absolute(absolute_far_right),
            ))
        }
    } {
        draw_line(frame, near_left, far_left, frustum_color);
        draw_line(frame, near_right, far_right, frustum_color);
        draw_line(frame, near_left, near_right, frustum_color);
        draw_pixel(frame, player, player_color);
    }
}
