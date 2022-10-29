mod draw;
mod pixel;

use crate::{draw::*, pixel::*};

use bevy::{
    app::AppExit,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
    math::vec3,
    math::Vec3,
    prelude::*,
    utils::Duration,
    window::WindowResizeConstraints,
};
use bevy_pixels::prelude::*;
use bevy_render::color::Color as BevyColor;
use image::RgbaImage;

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

lazy_static! {
    static ref FOV_Y_RADIANS: f32 = 2.0 * ((FOV_X_RADIANS * 0.5).tan() / ASPECT_RATIO).atan();
    static ref PERSPECTIVE_MATRIX: Mat4 =
        Mat4::perspective_infinite_reverse_rh(*FOV_Y_RADIANS, ASPECT_RATIO, Z_NEAR);
    static ref TAN_FAC_FOV_X_2: f32 = (FOV_X_RADIANS / 2.0).tan();
    static ref X_NEAR: f32 = -Z_NEAR * *TAN_FAC_FOV_X_2;
    static ref X_FAR: f32 = -Z_FAR * *TAN_FAC_FOV_X_2;
    static ref BACK_CLIP_1: Vec2 = Vec2::new(-*X_NEAR, Z_NEAR);
    static ref BACK_CLIP_2: Vec2 = Vec2::new(*X_NEAR, Z_NEAR);
    static ref LEFT_CLIP_1: Vec2 = *BACK_CLIP_1;
    static ref LEFT_CLIP_2: Vec2 = Vec2::new(-*X_FAR, Z_FAR);
    static ref RIGHT_CLIP_1: Vec2 = *BACK_CLIP_2;
    static ref RIGHT_CLIP_2: Vec2 = Vec2::new(*X_FAR, Z_FAR);
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
enum View {
    Absolute2d,
    FirstPerson2d,
    FirstPerson3d,
}

#[derive(Debug)]
pub struct AppState {
    view: View,
    position: Position,
    velocity: Velocity,
    direction: Direction,
    brick: RgbaImage,
    update_title_timer: Timer,
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    // #[cfg(not(target_arch = "wasm32"))]
    // let brick = ImageReader::open("brick.png")
    //     .unwrap()
    //     .decode()
    //     .unwrap()
    //     .into_rgba8();
    // #[cfg(target_arch = "wasm32")]
    // let brick = RgbaImage::new(64, 64);

    let brick = RgbaImage::new(64, 64);

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
            view: View::FirstPerson3d,
            position: Position(vec3(0.0, 2.0, 0.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            brick: brick,
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
        .add_system(switch_view_system)
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
        .add_system_to_stage(
            PixelsStage::Draw,
            draw_player_system.after(draw_minimap_system),
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

fn switch_view_system(key: Res<Input<KeyCode>>, mut state: ResMut<AppState>) {
    if key.just_pressed(KeyCode::Key1) {
        if state.view != View::Absolute2d {
            state.view = View::Absolute2d;
        }
    } else if key.just_pressed(KeyCode::Key2) {
        if state.view != View::FirstPerson2d {
            state.view = View::FirstPerson2d;
        }
    } else if key.just_pressed(KeyCode::Key3) {
        if state.view != View::FirstPerson3d {
            state.view = View::FirstPerson3d;
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

        let (view_left_top, view_left_bottom, view_right_top, view_right_bottom, draw) = clip_wall(
            view_left_top,
            view_left_bottom,
            view_right_top,
            view_right_bottom,
        );
        if !draw {
            continue;
        }

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
            &state.brick,
        );
    }
}

fn clip_wall(
    mut view_left_top: Vec3,
    mut view_left_bottom: Vec3,
    mut view_right_top: Vec3,
    mut view_right_bottom: Vec3,
) -> (Vec3, Vec3, Vec3, Vec3, bool) {
    if view_left_top.z > Z_NEAR && view_right_top.z > Z_NEAR {
        // Wall entirely behind view plane, skip drawing
        return (
            view_left_top,
            view_left_bottom,
            view_right_top,
            view_right_bottom,
            false,
        );
    }

    if view_left_top.z > Z_NEAR {
        // Left side behind, clip
        // println!("clip left behind");
        view_left_top = clip_line_xz(view_left_top, view_right_top, *BACK_CLIP_1, *BACK_CLIP_2);
        view_left_bottom.x = view_left_top.x;
        view_left_bottom.z = view_left_top.z;
    } else {
        // Left side in front, clip right edge right side
        // println!("clip right right");
        view_right_top = clip_line_xz(view_right_top, view_left_top, *RIGHT_CLIP_1, *RIGHT_CLIP_2);
        view_right_bottom.x = view_right_top.x;
        view_right_bottom.z = view_right_top.z;
    }

    if view_right_top.z > Z_NEAR {
        // Right side behind, clip
        // println!("clip right behind");
        view_right_top = clip_line_xz(view_right_top, view_left_top, *BACK_CLIP_1, *BACK_CLIP_2);
        view_right_bottom.x = view_right_top.x;
        view_right_bottom.z = view_right_top.z;
    } else {
        // Right side in front, clip left edge left side
        // println!("clip left left");
        view_left_top = clip_line_xz(view_left_top, view_right_top, *LEFT_CLIP_1, *LEFT_CLIP_2);
        view_left_bottom.x = view_left_top.x;
        view_left_bottom.z = view_left_top.z;
    }

    (
        view_left_top,
        view_left_bottom,
        view_right_top,
        view_right_bottom,
        true,
    )
}

fn clip_line_xz(outside: Vec3, inside: Vec3, clip_1: Vec2, clip_2: Vec2) -> Vec3 {
    if let Some(Vec2 { x, y: z }) = intersection(
        Vec2::new(outside.x, outside.z),
        Vec2::new(inside.x, inside.z),
        clip_1,
        clip_2,
    ) {
        if x >= outside.x.min(inside.x) && x <= outside.x.max(inside.x) {
            return Vec3::new(x, outside.y, z);
        }
    }

    outside
}

fn intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let a_perp_dot = a1.perp_dot(a2);
    let b_perp_dot = b1.perp_dot(b2);

    let divisor = Vec2::new(a1.x - a2.x, a1.y - a2.y).perp_dot(Vec2::new(b1.x - b2.x, b1.y - b2.y));
    if divisor == 0.0 {
        return None;
    };

    Some(Vec2::new(
        Vec2::new(a_perp_dot, a1.x - a2.x).perp_dot(Vec2::new(b_perp_dot, b1.x - b2.x)) / divisor,
        Vec2::new(a_perp_dot, a1.y - a2.y).perp_dot(Vec2::new(b_perp_dot, b1.y - b2.y)) / divisor,
    ))
}

fn draw_minimap_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<&Wall>,
    state: Res<AppState>,
) {
    let frame = pixels_resource.pixels.get_frame_mut();
    let view_matrix =
        Mat4::from_rotation_y(-state.direction.0) * Mat4::from_translation(-state.position.0);

    for wall in query.iter() {
        match state.view {
            View::Absolute2d => {
                let a = Pixel::from_absolute(wall.left.0);
                let b = Pixel::from_absolute(wall.right.0);
                draw_line(frame, a, b, wall.color);
            }
            View::FirstPerson2d => {
                let wall_left_top = vec3(wall.left.0.x, wall.height.0, wall.left.0.z);
                let wall_left_bottom = wall.left.0;
                let wall_right_top = vec3(wall.right.0.x, wall.height.0, wall.right.0.z);
                let wall_right_bottom = wall.right.0;

                let view_left_top = view_matrix.transform_point3(wall_left_top);
                let view_left_bottom = view_matrix.transform_point3(wall_left_bottom);
                let view_right_top = view_matrix.transform_point3(wall_right_top);
                let view_right_bottom = view_matrix.transform_point3(wall_right_bottom);

                let (view_left_top_after_clip, _, view_right_top_after_clip, _, draw) = clip_wall(
                    view_left_top,
                    view_left_bottom,
                    view_right_top,
                    view_right_bottom,
                );

                if !draw {
                    draw_line(
                        frame,
                        Pixel::from_absolute(view_left_top),
                        Pixel::from_absolute(view_right_top),
                        Color(0xff, 0xff, 0xff, 0xff),
                    );
                    continue;
                }

                if view_left_top_after_clip != view_left_top {
                    draw_line(
                        frame,
                        Pixel::from_absolute(view_left_top),
                        Pixel::from_absolute(view_left_top_after_clip),
                        Color(0xff, 0xff, 0xff, 0xff),
                    );
                }

                if view_right_top_after_clip != view_right_top {
                    draw_line(
                        frame,
                        Pixel::from_absolute(view_right_top_after_clip),
                        Pixel::from_absolute(view_right_top),
                        Color(0xff, 0xff, 0xff, 0xff),
                    );
                }

                draw_line(
                    frame,
                    Pixel::from_absolute(view_left_top_after_clip),
                    Pixel::from_absolute(view_right_top_after_clip),
                    wall.color,
                );
            }
            View::FirstPerson3d => {}
        }
    }
}

fn draw_player_system(mut pixels_resource: ResMut<PixelsResource>, state: Res<AppState>) {
    let frame = pixels_resource.pixels.get_frame_mut();

    match state.view {
        View::Absolute2d => {
            let pixel = Pixel::from_absolute(state.position.0);
            let end = Pixel::new(
                (pixel.x as f32 - 10.0 * state.direction.0.sin()).floor() as isize,
                (pixel.y as f32 - 10.0 * state.direction.0.cos()).floor() as isize,
            );
            draw_line(frame, pixel, end, Color(0x88, 0x88, 0x88, 0xff));
            draw_pixel(frame, pixel, Color(0xff, 0x00, 0x00, 0xff));
        }
        View::FirstPerson2d => {
            let position = (FRAC_WIDTH_2 as isize - 1, FRAC_HEIGHT_2 as isize - 1);
            draw_line(
                frame,
                Pixel::new(position.0, position.1),
                Pixel::new(position.0 - 80, position.1 - 80),
                Color(0x88, 0x88, 0x88, 0xff),
            );
            draw_line(
                frame,
                Pixel::new(position.0, position.1),
                Pixel::new(position.0 + 80, position.1 - 80),
                Color(0x88, 0x88, 0x88, 0xff),
            );
            draw_pixel(
                frame,
                Pixel::new(position.0, position.1),
                Color(0xff, 0x00, 0x00, 0xff),
            );
        }
        _ => {}
    }
}
