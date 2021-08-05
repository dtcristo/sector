use bevy::{
    app::AppExit,
    prelude::*,
    window::{WindowMode, WindowResizeConstraints},
};
use bevy_pixels::prelude::*;
use glam::{Affine2, Vec2};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

#[derive(Bundle, Debug)]
struct Wall {
    start_position: Position,
    end_position: Position,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
struct Pixel(isize, isize);

// Position
//   .---> +x
//   |
//   v
//   +y
#[derive(Debug, Copy, Clone)]
struct Position(f32, f32);

#[derive(Debug, Copy, Clone)]
struct Velocity(f32, f32);

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
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
enum AppStage {
    DrawBackground,
    DrawObjects,
}

fn main() {
    App::build()
        .insert_resource(WindowDescriptor {
            title: "prender".to_string(),
            width: (4 * WIDTH) as f32,
            height: (4 * HEIGHT) as f32,
            vsync: true,
            mode: WindowMode::BorderlessFullscreen,
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
            view: View::Absolute2d,
            position: Position(0.0, 0.0),
            velocity: Velocity(0.0, 0.0),
            direction: Direction(0.0),
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PixelsPlugin)
        .add_startup_system(setup_system.system())
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
        start_position: Position(-40.0, -70.0),
        end_position: Position(40.0, 30.0),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    // commands.spawn().insert(Wall {
    //     start_position: Position(40.0, 30.0),
    //     end_position: Position(40.0, 80.0),
    //     color: Color(0x00, 0xff, 0x00, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     start_position: Position(40.0, 80.0),
    //     end_position: Position(-110.0, 80.0),
    //     color: Color(0x00, 0x00, 0xff, 0xff),
    // });

    // commands.spawn().insert(Wall {
    //     start_position: Position(-110.0, 80.0),
    //     end_position: Position(-40.0, -70.0),
    //     color: Color(0xff, 0x00, 0xff, 0xff),
    // });
}

fn exit_on_escape_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        app_exit_events.send(AppExit);
    }
}

fn switch_view_system(keyboard_input: Res<Input<KeyCode>>, mut state: ResMut<AppState>) {
    if keyboard_input.just_pressed(KeyCode::Key1) {
        if state.view != View::Absolute2d {
            state.view = View::Absolute2d;
        }
    } else if keyboard_input.just_pressed(KeyCode::Key2) {
        if state.view != View::FirstPerson2d {
            state.view = View::FirstPerson2d;
        }
    } else if keyboard_input.just_pressed(KeyCode::Key3) {
        if state.view != View::FirstPerson3d {
            state.view = View::FirstPerson3d;
        }
    }
}

fn player_movement_system(keyboard_input: Res<Input<KeyCode>>, mut state: ResMut<AppState>) {
    if keyboard_input.pressed(KeyCode::Left) || keyboard_input.pressed(KeyCode::Q) {
        state.direction.0 += 0.06;
    } else if keyboard_input.pressed(KeyCode::Right) || keyboard_input.pressed(KeyCode::E) {
        state.direction.0 -= 0.06;
    }

    if keyboard_input.pressed(KeyCode::Up) || keyboard_input.pressed(KeyCode::W) {
        state.velocity.0 = -state.direction.0.sin();
        state.velocity.1 = -state.direction.0.cos();
    } else if keyboard_input.pressed(KeyCode::Down) || keyboard_input.pressed(KeyCode::S) {
        state.velocity.0 = state.direction.0.sin();
        state.velocity.1 = state.direction.0.cos();
    } else if keyboard_input.pressed(KeyCode::A) {
        state.velocity.0 = -state.direction.0.cos();
        state.velocity.1 = state.direction.0.sin();
    } else if keyboard_input.pressed(KeyCode::D) {
        state.velocity.0 = state.direction.0.cos();
        state.velocity.1 = -state.direction.0.sin();
    } else {
        state.velocity.0 = 0.0;
        state.velocity.1 = 0.0;
    }

    state.position.0 += state.velocity.0;
    state.position.1 += state.velocity.1;
}

fn draw_background_system(mut pixels_resource: ResMut<PixelsResource>) {
    let frame = pixels_resource.pixels.get_frame();
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_player_system(mut pixels_resource: ResMut<PixelsResource>, state: Res<AppState>) {
    let frame = pixels_resource.pixels.get_frame();
    match state.view {
        View::Absolute2d => {
            let pixel = position_to_pixel(&state.position);
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
    let position = Vec2::new(-state.position.0, -state.position.1);
    let affine = Affine2::from_angle(state.direction.0) * Affine2::from_translation(position);

    for wall in query.iter() {
        match state.view {
            View::Absolute2d => {
                let start_pixel = position_to_pixel(&wall.start_position);
                let end_pixel = position_to_pixel(&wall.end_position);
                draw_line(frame, start_pixel, end_pixel, wall.color);
            }
            View::FirstPerson2d => {
                let start = affine
                    .transform_point2(Vec2::new(wall.start_position.0, wall.start_position.1));
                let end =
                    affine.transform_point2(Vec2::new(wall.end_position.0, wall.end_position.1));

                draw_line(
                    frame,
                    position_to_pixel(&Position(start.x, start.y)),
                    position_to_pixel(&Position(end.x, end.y)),
                    wall.color,
                );
            }
            View::FirstPerson3d => {
                // tx1, tz1
                let start = affine
                    .transform_point2(Vec2::new(wall.start_position.0, wall.start_position.1));

                // tx2, tz2
                let end =
                    affine.transform_point2(Vec2::new(wall.end_position.0, wall.end_position.1));

                let x1 = -start.x * 64.0 / start.y;
                let x2 = -end.x * 64.0 / end.y;

                let y1a = -120.0 / start.y;
                let y2a = -120.0 / end.y;

                let y1b = 120.0 / start.y;
                let y2b = 120.0 / end.y;

                // Top (1-2 b)
                draw_line(
                    frame,
                    Pixel(160 + x1 as isize, 120 + y1a as isize),
                    Pixel(160 + x2 as isize, 120 + y2a as isize),
                    wall.color,
                );
                // Bottom (1-2 b)
                draw_line(
                    frame,
                    Pixel(160 + x1 as isize, 120 + y1b as isize),
                    Pixel(160 + x2 as isize, 120 + y2b as isize),
                    wall.color,
                );
                // Left
                draw_line(
                    frame,
                    Pixel(160 + x1 as isize, 120 + y1a as isize),
                    Pixel(160 + x1 as isize, 120 + y1b as isize),
                    wall.color,
                );
                // Right
                draw_line(
                    frame,
                    Pixel(160 + x2 as isize, 120 + y2a as isize),
                    Pixel(160 + x2 as isize, 120 + y2b as isize),
                    wall.color,
                );
            }
        }
    }
}

fn draw_line(frame: &mut [u8], start: Pixel, end: Pixel, color: Color) {
    for (x, y) in line_drawing::Bresenham::new((start.0, start.1), (end.0, end.1)) {
        draw_pixel(frame, Pixel(x, y), color);
    }
}

fn position_to_pixel(position: &Position) -> Pixel {
    Pixel(
        position.0.round() as isize + (WIDTH / 2) as isize - 1,
        position.1.round() as isize + (HEIGHT / 2) as isize - 1,
    )
}

fn pixel_to_frame_offset(pixel: Pixel) -> Option<usize> {
    if pixel.0 >= 0 && pixel.0 < WIDTH as isize && pixel.1 >= 0 && pixel.1 < HEIGHT as isize {
        Some((pixel.1 as u32 * WIDTH * 4 + pixel.0 as u32 * 4) as usize)
    } else {
        None
    }
}

fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: Color) {
    if let Some(offset) = pixel_to_frame_offset(pixel) {
        frame[offset..offset + 4].copy_from_slice(&[color.0, color.1, color.2, color.3]);
    }
}
