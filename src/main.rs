use bevy::{app::AppExit, prelude::*, window::WindowResizeConstraints};
use bevy_pixels::prelude::*;
// use rand::prelude::*;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

#[derive(Bundle, Debug)]
struct PlayerBundle {
    position: Position,
    velocity: Velocity,
    direction: Direction,
}

#[derive(Bundle, Debug)]
struct Wall {
    start_position: Position,
    end_position: Position,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
struct Pixel(isize, isize);

#[derive(Debug, Copy, Clone)]
struct Position(f32, f32);

#[derive(Debug, Copy, Clone)]
struct Velocity(f32, f32);

#[derive(Debug, Copy, Clone)]
struct Direction(f32);

#[derive(Debug, Copy, Clone)]
struct Color(u8, u8, u8, u8);

#[derive(Debug, PartialEq)]
enum View {
    Absolute2d,
    FirstPerson2d,
}

#[derive(Debug)]
pub struct AppState {
    view: View,
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
            width: (2 * WIDTH) as f32,
            height: (2 * HEIGHT) as f32,
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
    commands.spawn().insert_bundle(PlayerBundle {
        position: Position(0.0, 0.0),
        velocity: Velocity(0.0, 0.0),
        direction: Direction(0.0),
    });

    commands.spawn().insert(Wall {
        start_position: Position(-40.0, -70.0),
        end_position: Position(40.0, 30.0),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert(Wall {
        start_position: Position(40.0, 30.0),
        end_position: Position(40.0, 80.0),
        color: Color(0x00, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert(Wall {
        start_position: Position(40.0, 80.0),
        end_position: Position(-110.0, 80.0),
        color: Color(0x00, 0x00, 0xff, 0xff),
    });

    commands.spawn().insert(Wall {
        start_position: Position(-110.0, 80.0),
        end_position: Position(-40.0, -70.0),
        color: Color(0xff, 0x00, 0xff, 0xff),
    });
}

fn exit_on_escape_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut app_exit_events: EventWriter<AppExit>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        app_exit_events.send(AppExit);
    }
}

fn switch_view_system(keyboard_input: Res<Input<KeyCode>>, mut resource: ResMut<AppState>) {
    if keyboard_input.just_pressed(KeyCode::Key1) {
        if resource.view != View::Absolute2d {
            resource.view = View::Absolute2d;
            println!("Absolute2d");
        }
    } else if keyboard_input.just_pressed(KeyCode::Key2) {
        if resource.view != View::FirstPerson2d {
            resource.view = View::FirstPerson2d;
            println!("FirstPerson2d");
        }
    }
}

fn player_movement_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut Velocity, &mut Direction, &mut Position)>,
) {
    if let Ok((mut velocity, mut direction, mut position)) = query.single_mut() {
        if keyboard_input.pressed(KeyCode::Left) {
            direction.0 -= 5.0;
        } else if keyboard_input.pressed(KeyCode::Right) {
            direction.0 += 5.0;
        }

        if keyboard_input.pressed(KeyCode::Up) {
            velocity.0 = direction.0.to_radians().sin();
            velocity.1 = -direction.0.to_radians().cos();
        } else if keyboard_input.pressed(KeyCode::Down) {
            velocity.0 = -direction.0.to_radians().sin();
            velocity.1 = direction.0.to_radians().cos();
        } else {
            velocity.0 = 0.0;
            velocity.1 = 0.0;
        }

        position.0 += velocity.0;
        position.1 += velocity.1;
    }
}

fn draw_background_system(mut pixels_resource: ResMut<PixelsResource>) {
    let frame = pixels_resource.pixels.get_frame();
    frame.copy_from_slice(&[0x00, 0x00, 0x00, 0xff].repeat(frame.len() / 4));
}

fn draw_player_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<(&Position, &Direction)>,
    resource: Res<AppState>,
) {
    for (position, direction) in query.iter() {
        let frame = pixels_resource.pixels.get_frame();
        match resource.view {
            View::Absolute2d => {
                let pixel = position_to_pixel(position);
                let end = Pixel(
                    (pixel.0 as f32 + 5.0 * direction.0.to_radians().sin()).round() as isize,
                    (pixel.1 as f32 - 5.0 * direction.0.to_radians().cos()).round() as isize,
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
        }
    }
}

fn draw_wall_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<&Wall>,
    resource: Res<AppState>,
) {
    for wall in query.iter() {
        match resource.view {
            View::Absolute2d => {
                let start_pixel = position_to_pixel(&wall.start_position);
                let end_pixel = position_to_pixel(&wall.end_position);
                let frame = pixels_resource.pixels.get_frame();
                draw_line(frame, start_pixel, end_pixel, wall.color);
            }
            View::FirstPerson2d => {}
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
        position.0.round() as isize + (WIDTH / 2) as isize,
        position.1.round() as isize + (HEIGHT / 2) as isize,
    )
}

fn pixel_to_frame_offset(pixel: Pixel) -> Option<usize> {
    if pixel.0 >= 0 && pixel.0 < WIDTH as isize && pixel.0 >= 0 && pixel.0 < HEIGHT as isize {
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
