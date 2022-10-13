use bevy::{
    app::AppExit, input::mouse::MouseMotion, math::vec3, prelude::*, utils::Duration,
    window::WindowResizeConstraints,
};
use bevy_pixels::prelude::*;
use image::{io::Reader as ImageReader, RgbaImage};
use rust_bresenham::Bresenham;

const WIDTH: u32 = 320;
const WIDTH_MINUS_1: isize = WIDTH as isize - 1;
const HEIGHT: u32 = 240;
const HEIGHT_MINUS_1: isize = HEIGHT as isize - 1;
const FRAC_WIDTH_2: u32 = WIDTH / 2;
const FRAC_HEIGHT_2: u32 = HEIGHT / 2;
const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;
const Z_NEAR: f32 = 0.1;

#[derive(Component, Bundle, Debug)]
struct Wall {
    a: Position,
    b: Position,
    height: Length,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
struct Pixel(isize, isize);

impl Pixel {
    fn to_tuple(self) -> (isize, isize) {
        (self.0, self.1)
    }
}

#[derive(Component, Debug, Copy, Clone)]
struct Length(f32);

// Position (https://bevy-cheatbook.github.io/features/coords.html)
// +y.---> +x
//   |
//   v
//   +z
#[derive(Component, Debug, Copy, Clone)]
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

#[derive(Component, Debug, Copy, Clone)]
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

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    #[cfg(not(target_arch = "wasm32"))]
    let brick = ImageReader::open("brick.png")
        .unwrap()
        .decode()
        .unwrap()
        .into_rgba8();
    #[cfg(target_arch = "wasm32")]
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
            view: View::FirstPerson2d,
            position: Position(vec3(0.0, 2.0, 0.0)),
            velocity: Velocity(vec3(0.0, 0.0, 0.0)),
            direction: Direction(0.0),
            brick: brick,
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PixelsPlugin)
        .add_startup_system(setup_system)
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
        a: Position(vec3(-5.0, 0.0, -5.0)),
        b: Position(vec3(5.0, 0.0, -5.0)),
        height: Length(4.0),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    // commands.spawn().insert(Wall {
    //     a: Position(vec3(-40.0, 0.0, -100.0)),
    //     b: Position(vec3(40.0, 0.0, -50.0)),
    //     height: Length(4.0),
    //     color: Color(0xff, 0xff, 0x00, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     a: Position(vec3(40.0, 0.0, 30.0)),
    //     b: Position(vec3(40.0, 0.0, 80.0)),
    //     height: Length(4.0),
    //     color: Color(0x00, 0xff, 0x00, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     a: Position(vec3(40.0, 0.0, 80.0)),
    //     b: Position(vec3(-110.0, 0.0, 80.0)),
    //     height: Length(4.0),
    //     color: Color(0x00, 0x00, 0xff, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     a: Position(vec3(-110.0, 0.0, 80.0)),
    //     b: Position(vec3(-40.0, 0.0, -70.0)),
    //     height: Length(4.0),
    //     color: Color(0xff, 0x00, 0xff, 0xff),
    // });
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
    } else if key.pressed(KeyCode::Space) {
        state.velocity.0.y = 1.0;
    } else if key.pressed(KeyCode::LControl) {
        state.velocity.0.y = -1.0;
    } else {
        state.velocity.0.x = 0.0;
        state.velocity.0.y = 0.0;
        state.velocity.0.z = 0.0;
    }

    state.position.0.x += 0.05 * state.velocity.0.x;
    state.position.0.y += 0.05 * state.velocity.0.y;
    state.position.0.z += 0.05 * state.velocity.0.z;
}

fn draw_background_system(mut pixels_resource: ResMut<PixelsResource>) {
    let frame = pixels_resource.pixels.get_frame_mut();
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_player_system(mut pixels_resource: ResMut<PixelsResource>, state: Res<AppState>) {
    let frame = pixels_resource.pixels.get_frame_mut();

    // Debug lines and dots
    // draw_line(
    //     frame,
    //     Pixel(0, 0),
    //     Pixel(WIDTH as isize - 1, HEIGHT as isize - 1),
    //     Color(0xff, 0x00, 0x00, 0xff),
    // );
    // draw_line(
    //     frame,
    //     Pixel(0, HEIGHT as isize - 1),
    //     Pixel(WIDTH as isize - 1, 0),
    //     Color(0xff, 0x00, 0x00, 0xff),
    // );

    // draw_pixel(frame, Pixel(0, 0), Color(0x00, 0xff, 0x00, 0xff));
    // draw_pixel(
    //     frame,
    //     Pixel(WIDTH as isize - 1, 0),
    //     Color(0x00, 0xff, 0x00, 0xff),
    // );
    // draw_pixel(
    //     frame,
    //     Pixel(0, HEIGHT as isize - 1),
    //     Color(0x00, 0xff, 0x00, 0xff),
    // );
    // draw_pixel(
    //     frame,
    //     Pixel(WIDTH as isize - 1, HEIGHT as isize - 1),
    //     Color(0x00, 0xff, 0x00, 0xff),
    // );

    match state.view {
        View::Absolute2d => {
            let pixel = absolute_to_pixel(state.position.0);
            let end = Pixel(
                (pixel.0 as f32 - 5.0 * state.direction.0.sin()).floor() as isize,
                (pixel.1 as f32 - 5.0 * state.direction.0.cos()).floor() as isize,
            );
            draw_line(frame, pixel, end, Color(0x88, 0x88, 0x88, 0xff));
            draw_pixel(frame, pixel, Color(0xff, 0x00, 0x00, 0xff));
        }
        View::FirstPerson2d => {
            draw_line(
                frame,
                Pixel(159, 119),
                Pixel(149, 109),
                Color(0x88, 0x88, 0x88, 0xff),
            );
            draw_line(
                frame,
                Pixel(159, 119),
                Pixel(169, 109),
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
    time: Res<Time>,
    state: Res<AppState>,
) {
    let frame = pixels_resource.pixels.get_frame_mut();
    let time_since_startup = time.time_since_startup();
    let view_matrix =
        Mat4::from_rotation_y(-state.direction.0) * Mat4::from_translation(-state.position.0);
    let perspective_matrix =
        Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, ASPECT_RATIO, Z_NEAR);

    for wall in query.iter() {
        println!("\n\n\n\n......");

        let wall_a_top = vec3(wall.a.0.x, wall.height.0, wall.a.0.z);
        let wall_a_bottom = wall.a.0;
        let wall_b_top = vec3(wall.b.0.x, wall.height.0, wall.b.0.z);
        let wall_b_bottom = wall.b.0;

        let mut view_a_top = view_matrix.transform_point3(wall_a_top);
        let mut view_b_top = view_matrix.transform_point3(wall_b_top);
        let mut view_a_bottom = view_matrix.transform_point3(wall_a_bottom);
        let mut view_b_bottom = view_matrix.transform_point3(wall_b_bottom);

        println!("before clip");
        dbg!(view_a_top);
        dbg!(view_a_bottom);
        // dbg!(view_b_top);
        // dbg!(view_b_bottom);

        if view_a_top.z > -Z_NEAR && view_b_top.z > -Z_NEAR {
            // Wall entirely behind view plane, skip drawing
            continue;
        } else if view_a_top.z > -Z_NEAR {
            // Left side behind player
            println!("clipping left");
            clip_line_behind(&mut view_a_top, view_b_top);
            clip_line_behind(&mut view_a_bottom, view_b_bottom);
        } else if view_b_top.z > -Z_NEAR {
            // Right side behind player
            println!("clipping right");
            clip_line_behind(&mut view_b_top, view_a_top);
            clip_line_behind(&mut view_b_bottom, view_a_bottom);
        }

        println!("after clip");
        dbg!(view_a_top);
        dbg!(view_a_bottom);
        // dbg!(view_b_top);
        // dbg!(view_b_bottom);

        // draw_image(frame, Pixel(10, 10), &state.brick);

        let normalized_a_top = perspective_matrix.project_point3(view_a_top);
        let normalized_a_bottom = perspective_matrix.project_point3(view_a_bottom);
        let normalized_b_top = perspective_matrix.project_point3(view_b_top);
        let normalized_b_bottom = perspective_matrix.project_point3(view_b_bottom);

        println!("\n......");
        dbg!(normalized_a_top);
        dbg!(normalized_a_bottom);
        // dbg!(normalized_b_top);
        // dbg!(normalized_b_bottom);

        let a_top = normalized_to_pixel(normalized_a_top);
        let a_bottom = normalized_to_pixel(normalized_a_bottom);
        let b_top = normalized_to_pixel(normalized_b_top);
        let b_bottom = normalized_to_pixel(normalized_b_bottom);

        println!("\n......");
        dbg!(a_top);
        dbg!(a_bottom);
        // dbg!(b_top);
        // dbg!(b_bottom);

        draw_wall(
            frame,
            a_top,
            a_bottom,
            b_top,
            b_bottom,
            time_since_startup,
            &state.brick,
        );
    }
}

fn clip_line_behind(back: &mut Vec3, front: Vec3) {
    let dx1 = front.x - back.x;
    let mut dz1 = front.z - back.z;
    if dz1 == 0.0 {
        dz1 = 1.0
    };
    let dz2 = -Z_NEAR - back.z;
    let dx2 = dz2 * dx1 / dz1;

    back.x = back.x + dx2;
    back.z = -Z_NEAR;
}

fn draw_wall(
    frame: &mut [u8],
    a_top: Pixel,
    a_bottom: Pixel,
    b_top: Pixel,
    b_bottom: Pixel,
    time_since_startup: Duration,
    _texture: &RgbaImage,
) {
    if a_top.0 != a_bottom.0 || b_top.0 != b_bottom.0 {
        panic!("top of wall is not directly above bottom of wall");
    }

    let dy_top = b_top.1 - a_top.1;
    let dy_bottom = b_bottom.1 - a_bottom.1;
    let mut dx = b_top.0 - a_top.0;
    if dx == 0 {
        dx = 1;
    }
    let xs = a_top.0;

    // Clip x
    let x1 = if a_top.0 > 1 { a_top.0 } else { 1 };
    let x2 = if b_top.0 < WIDTH_MINUS_1 - 1 {
        b_top.0
    } else {
        WIDTH_MINUS_1 - 1
    };

    for x in x1..=x2 {
        let y_top = dy_top * (x - xs) / dx + a_top.1;
        let y_bottom = dy_bottom * (x - xs) / dx + a_bottom.1;

        // Clip y
        let y1 = if y_top > 1 { y_top } else { 1 };
        let y2 = if y_bottom < HEIGHT_MINUS_1 - 1 {
            y_bottom
        } else {
            HEIGHT_MINUS_1 - 1
        };

        for y in y1..=y2 {
            draw_pixel(frame, Pixel(x, y), Color(0xff, 0x00, 0xff, 0xff));
        }
    }

    // Draw wall outline (blinking)
    if time_since_startup.as_secs() & 1 == 1 {
        draw_line(
            frame,
            Pixel(a_top.0, a_top.1),
            Pixel(b_top.0, b_top.1),
            Color(0xff, 0x00, 0x00, 0xff),
        );
        draw_line(
            frame,
            Pixel(a_bottom.0, a_bottom.1),
            Pixel(b_bottom.0, b_bottom.1),
            Color(0xff, 0x00, 0x00, 0xff),
        );

        draw_pixel(frame, a_top, Color(0x00, 0xff, 0x00, 0xff));
        draw_pixel(frame, a_bottom, Color(0x00, 0xff, 0x00, 0xff));
        draw_pixel(frame, b_top, Color(0x00, 0xff, 0x00, 0xff));
        draw_pixel(frame, b_bottom, Color(0x00, 0xff, 0x00, 0xff));
    }
}

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
                let a_pixel = absolute_to_pixel(wall.a.0);
                let b_pixel = absolute_to_pixel(wall.b.0);
                draw_line(frame, a_pixel, b_pixel, wall.color);
            }
            View::FirstPerson2d => {
                let a = view_matrix.transform_point3(wall.a.0);
                let b = view_matrix.transform_point3(wall.b.0);

                draw_line(
                    frame,
                    absolute_to_pixel(a),
                    absolute_to_pixel(b),
                    wall.color,
                );
            }
            View::FirstPerson3d => {}
        }
    }
}

fn draw_line(frame: &mut [u8], a: Pixel, b: Pixel, color: Color) {
    for (x, y) in Bresenham::new((a.0, a.1), (b.0, b.1)) {
        draw_pixel(frame, Pixel(x, y), color);
    }
}

fn absolute_to_pixel(v: Vec3) -> Pixel {
    Pixel(
        v.x.floor() as isize + FRAC_WIDTH_2 as isize,
        v.z.floor() as isize + FRAC_HEIGHT_2 as isize,
    )
}

fn normalized_to_pixel(v: Vec3) -> Pixel {
    Pixel(
        FRAC_WIDTH_2 as isize + (FRAC_WIDTH_2 as f32 * v.x).floor() as isize,
        FRAC_HEIGHT_2 as isize - (FRAC_HEIGHT_2 as f32 * v.y).floor() as isize,
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
