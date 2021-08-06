use bevy::{
    app::AppExit,
    input::mouse::MouseMotion,
    prelude::*,
    window::{WindowMode, WindowResizeConstraints},
};
use bevy_pixels::prelude::*;
use glam::{vec3, Affine3A, Mat4, Vec3};
use image::{io::Reader as ImageReader, RgbaImage};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;

#[derive(Bundle, Debug)]
struct Wall {
    a_position: Position,
    b_position: Position,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
struct Pixel(isize, isize);

// Position
// +y.---> +x
//   |
//   v
//   +z
#[derive(Debug, Copy, Clone)]
struct Position(Vec3);

#[derive(Debug, Copy, Clone)]
struct Velocity(Vec3);

// Direction
//   ^   ^
//    \+θ|
//     \ |
//       .
#[derive(Debug, Copy, Clone)]
struct Direction(f32);

#[derive(Debug, Copy, Clone)]
struct Color(u8, u8, u8, u8);

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
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
enum AppStage {
    DrawBackground,
    DrawObjects,
}

fn main() {
    let brick = ImageReader::open("brick.png")
        .unwrap()
        .decode()
        .unwrap()
        .into_rgba8();

    App::build()
        .insert_resource(WindowDescriptor {
            title: "prender".to_string(),
            width: (4 * WIDTH) as f32,
            height: (4 * HEIGHT) as f32,
            vsync: true,
            mode: WindowMode::Windowed,
            resize_constraints: WindowResizeConstraints {
                min_width: WIDTH as f32,
                min_height: HEIGHT as f32,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert_resource(PixelsOptions {
            width: WIDTH,
            height: HEIGHT,
        })
        .insert_resource(AppState {
            view: View::FirstPerson2d,
            position: Position(vec3(0.0, 2.0, 0.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            brick: brick,
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PixelsPlugin)
        .add_startup_system(setup_system.system())
        .add_system(mouse_capture_system.system())
        .add_system(exit_on_escape_system.system())
        .add_system(switch_view_system.system())
        .add_system(player_movement_system.system())
        .add_stage_after(
            PixelsStage::Draw,
            AppStage::DrawBackground,
            SystemStage::parallel(),
        )
        .add_stage_after(
            AppStage::DrawBackground,
            AppStage::DrawObjects,
            SystemStage::parallel(),
        )
        .add_system_to_stage(PixelsStage::Draw, draw_background_system.system())
        .add_system_to_stage(AppStage::DrawObjects, draw_player_system.system())
        .add_system_to_stage(AppStage::DrawObjects, draw_wall_system.system())
        .run();
}

fn setup_system(mut commands: Commands) {
    commands.spawn().insert(Wall {
        a_position: Position(vec3(-40.0, 0.0, -100.0)),
        b_position: Position(vec3(40.0, 0.0, -50.0)),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    // commands.spawn().insert(Wall {
    //     a_position: Position(vec3(40.0, 0.0, 30.0)),
    //     b_position: Position(vec3(40.0, 0.0, 80.0)),
    //     color: Color(0x00, 0xff, 0x00, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     a_position: Position(vec3(40.0, 0.0, 80.0)),
    //     b_position: Position(vec3(-110.0, 0.0, 80.0)),
    //     color: Color(0x00, 0x00, 0xff, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     a_position: Position(vec3(-110.0, 0.0, 80.0)),
    //     b_position: Position(vec3(-40.0, 0.0, -70.0)),
    //     color: Color(0xff, 0x00, 0xff, 0xff),
    // });
}

fn mouse_capture_system(mut windows: ResMut<Windows>, mouse_button: Res<Input<MouseButton>>) {
    let window = windows.get_primary_mut().unwrap();

    if mouse_button.just_pressed(MouseButton::Left) {
        window.set_cursor_lock_mode(true);
        window.set_cursor_visibility(false);
    }

    if mouse_button.just_pressed(MouseButton::Right) {
        window.set_cursor_lock_mode(false);
        window.set_cursor_visibility(true);
    }
}

fn exit_on_escape_system(key: Res<Input<KeyCode>>, mut app_exit_events: EventWriter<AppExit>) {
    if key.just_pressed(KeyCode::Escape) {
        app_exit_events.send(AppExit);
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
    } else if key.pressed(KeyCode::Right) || key.pressed(KeyCode::E) {
        state.direction.0 -= 0.05;
    }

    if key.pressed(KeyCode::Up) || key.pressed(KeyCode::W) {
        state.velocity.0.x = -state.direction.0.sin();
        state.velocity.0.z = -state.direction.0.cos();
    } else if key.pressed(KeyCode::Down) || key.pressed(KeyCode::S) {
        state.velocity.0.x = state.direction.0.sin();
        state.velocity.0.z = state.direction.0.cos();
    } else if key.pressed(KeyCode::A) {
        state.velocity.0.x = -state.direction.0.cos();
        state.velocity.0.z = state.direction.0.sin();
    } else if key.pressed(KeyCode::D) {
        state.velocity.0.x = state.direction.0.cos();
        state.velocity.0.z = -state.direction.0.sin();
    } else {
        state.velocity.0.x = 0.0;
        state.velocity.0.z = 0.0;
    }

    state.position.0.x += state.velocity.0.x;
    state.position.0.z += state.velocity.0.z;
}

fn draw_background_system(mut pixels_resource: ResMut<PixelsResource>) {
    let frame = pixels_resource.pixels.get_frame();
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_player_system(mut pixels_resource: ResMut<PixelsResource>, state: Res<AppState>) {
    let frame = pixels_resource.pixels.get_frame();
    match state.view {
        View::Absolute2d => {
            let pixel = absolute_to_pixel(state.position.0);
            let end = Pixel(
                (pixel.0 as f32 - 5.0 * state.direction.0.sin()).round() as isize,
                (pixel.1 as f32 - 5.0 * state.direction.0.cos()).round() as isize,
            );
            draw_line(frame, pixel, end, Color(0x88, 0x88, 0x88, 0xff));
            draw_pixel(frame, pixel, Color(0xff, 0x00, 0x00, 0xff));
        }
        View::FirstPerson2d => {
            draw_line(
                frame,
                Pixel(159, 119),
                Pixel(159, 114),
                Color(0x88, 0x88, 0x88, 0xff),
            );
            draw_pixel(frame, Pixel(159, 119), Color(0xff, 0x00, 0x00, 0xff));
        }
        _ => {}
    }
}

fn draw_wall_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<&Wall>,
    state: Res<AppState>,
) {
    let frame = pixels_resource.pixels.get_frame();
    let view = Affine3A::from_rotation_y(-state.direction.0)
        * Affine3A::from_translation(-state.position.0);

    let perspective = Mat4::perspective_infinite_rh(std::f32::consts::FRAC_PI_2, ASPECT_RATIO, 1.0);

    for wall in query.iter() {
        let w_a_b = vec3(wall.a_position.0.x, 0.0, wall.a_position.0.z);
        let w_a_t = vec3(wall.a_position.0.x, 4.0, wall.a_position.0.z);
        let w_b_b = vec3(wall.b_position.0.x, 0.0, wall.b_position.0.z);
        let w_b_t = vec3(wall.b_position.0.x, 4.0, wall.b_position.0.z);

        let mut v_a_b = view.transform_point3(w_a_b);
        let mut v_b_b = view.transform_point3(w_b_b);

        println!("---------------------------------------------");
        dbg!(state.position.0);
        // dbg!(v_a_b);
        // dbg!(v_b_b);

        // if v_a_b.z >= -1.0 && v_b_b.z >= -1.0 {
        //     // Wall entirely behind view plane, skip drawing
        //     continue;
        // } else if !(v_a_b.z < -1.0 && v_b_b.z < -1.0) {
        //     // Wall intersects view plane
        //     if v_a_b.z < -1.0 {
        //         let z_ratio = (v_b_b.z / (v_a_b.z.abs() + v_b_b.z.abs())).abs();
        //         dbg!(z_ratio);
        //         v_b_b.x = v_b_b.x - z_ratio * (v_a_b.x - v_b_b.x);
        //         v_b_b.z = -1.0;
        //     } else {
        //         let z_ratio = (v_a_b.z / (v_a_b.z.abs() + v_b_b.z.abs())).abs();
        //         dbg!(z_ratio);
        //         v_a_b.x = v_a_b.x - z_ratio * (v_b_b.x - v_a_b.x);
        //         v_a_b.z = -1.0;
        //     }
        //     dbg!(v_a_b);
        //     dbg!(v_b_b);
        // }

        let v_a_t = view.transform_point3(w_a_t);
        let v_b_t = view.transform_point3(w_b_t);

        let p_a_b = perspective.project_point3(v_a_b);
        let p_a_t = perspective.project_point3(v_a_t);
        let p_b_b = perspective.project_point3(v_b_b);
        let p_b_t = perspective.project_point3(v_b_t);

        // dbg!(p_a_b);
        // dbg!(p_b_b);

        draw_line(
            frame,
            normalized_to_pixel(p_a_t),
            normalized_to_pixel(p_b_t),
            wall.color,
        );
        draw_line(
            frame,
            normalized_to_pixel(p_a_b),
            normalized_to_pixel(p_b_b),
            wall.color,
        );
        draw_line(
            frame,
            normalized_to_pixel(p_a_t),
            normalized_to_pixel(p_a_b),
            wall.color,
        );
        draw_line(
            frame,
            normalized_to_pixel(p_b_t),
            normalized_to_pixel(p_b_b),
            wall.color,
        );

        match state.view {
            View::Absolute2d => {
                let a_pixel = absolute_to_pixel(wall.a_position.0);
                let b_pixel = absolute_to_pixel(wall.b_position.0);
                draw_line(frame, a_pixel, b_pixel, wall.color);
            }
            View::FirstPerson2d => {
                let a = view.transform_point3(wall.a_position.0);
                let b = view.transform_point3(wall.b_position.0);

                draw_line(
                    frame,
                    absolute_to_pixel(a),
                    absolute_to_pixel(b),
                    wall.color,
                );
            }
            View::FirstPerson3d => {}
        }

        draw_image(frame, Pixel(10, 10), &state.brick);
    }
}

fn draw_wall(frame: &mut [u8], a_t: Pixel, a_b: Pixel, b_t: Pixel, b_b: Pixel, state: AppState) {}

fn draw_image(frame: &mut [u8], location: Pixel, image: &RgbaImage) {
    let frame_offset = pixel_to_offset(location).unwrap();
    for (row_index, row) in image
        .as_raw()
        .chunks(image.dimensions().1 as usize * 4)
        .enumerate()
    {
        frame[frame_offset + row_index * WIDTH as usize * 4
            ..frame_offset + row_index * WIDTH as usize * 4 + image.dimensions().1 as usize * 4]
            .copy_from_slice(row);
    }
}

fn draw_line(frame: &mut [u8], a: Pixel, b: Pixel, color: Color) {
    for (x, y) in line_drawing::Bresenham::new((a.0, a.1), (b.0, b.1)) {
        draw_pixel(frame, Pixel(x, y), color);
    }
}

fn absolute_to_pixel(v: Vec3) -> Pixel {
    Pixel(
        v.x.round() as isize + FRAC_WIDTH_2 as isize - 1,
        v.z.round() as isize + FRAC_HEIGHT_2 as isize - 1,
    )
}

fn normalized_to_pixel(v: Vec3) -> Pixel {
    Pixel(
        FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * v.x).round() as isize,
        FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * v.y).round() as isize,
    )
}

fn pixel_to_offset(pixel: Pixel) -> Option<usize> {
    if pixel.0 >= 0 && pixel.0 < WIDTH as isize && pixel.1 >= 0 && pixel.1 < HEIGHT as isize {
        Some((pixel.1 as u32 * WIDTH * 4 + pixel.0 as u32 * 4) as usize)
    } else {
        None
    }
}

fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: Color) {
    if let Some(offset) = pixel_to_offset(pixel) {
        frame[offset..offset + 4].copy_from_slice(&[color.0, color.1, color.2, color.3]);
    }
}
