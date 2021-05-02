use bevy::app::AppExit;
use bevy::window::WindowResizeConstraints;
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
struct WallBundle {
    start_position: StartPosition,
    end_position: EndPosition,
    color: Color,
}

#[derive(Debug, Copy, Clone)]
struct Pixel(u32, u32);

#[derive(Debug, Copy, Clone)]
struct Position(f32, f32);

#[derive(Debug, Copy, Clone)]
struct StartPosition(f32, f32);
impl StartPosition {
    fn as_position(&self) -> Position {
        Position(self.0, self.1)
    }
}

#[derive(Debug, Copy, Clone)]
struct EndPosition(f32, f32);
impl EndPosition {
    fn as_position(&self) -> Position {
        Position(self.0, self.1)
    }
}

#[derive(Debug, Copy, Clone)]
struct Velocity(f32, f32);

#[derive(Debug, Copy, Clone)]
struct Direction(f32);

#[derive(Debug, Copy, Clone)]
struct Color(u8, u8, u8, u8);

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
        .add_plugins(DefaultPlugins)
        .add_plugin(PixelsPlugin)
        .add_startup_system(setup_system.system())
        .add_system(exit_on_escape_system.system())
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
        position: Position(160.0, 120.0),
        velocity: Velocity(0.0, 0.0),
        direction: Direction(0.0),
    });

    commands.spawn().insert_bundle(WallBundle {
        start_position: StartPosition(120.0, 50.0),
        end_position: EndPosition(200.0, 150.0),
        color: Color(0xff, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert_bundle(WallBundle {
        start_position: StartPosition(200.0, 150.0),
        end_position: EndPosition(200.0, 200.0),
        color: Color(0x00, 0xff, 0x00, 0xff),
    });

    commands.spawn().insert_bundle(WallBundle {
        start_position: StartPosition(200.0, 200.0),
        end_position: EndPosition(50.0, 200.0),
        color: Color(0x00, 0x00, 0xff, 0xff),
    });

    commands.spawn().insert_bundle(WallBundle {
        start_position: StartPosition(50.0, 200.0),
        end_position: EndPosition(120.0, 50.0),
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
) {
    for (position, direction) in query.iter() {
        if let Some(pixel) = position_to_pixel(position) {
            let frame = pixels_resource.pixels.get_frame();
            let end = Pixel(
                (pixel.0 as f32 + 5.0 * direction.0.to_radians().sin()).round() as u32,
                (pixel.1 as f32 - 5.0 * direction.0.to_radians().cos()).round() as u32,
            );
            draw_line(frame, pixel, end, Color(0x88, 0x88, 0x88, 0xff));
            draw_pixel(frame, pixel, Color(0xff, 0x00, 0x00, 0xff));
        }
    }
}

fn draw_wall_system(
    mut pixels_resource: ResMut<PixelsResource>,
    query: Query<(&StartPosition, &EndPosition, &Color)>,
) {
    for (start_position, end_position, color) in query.iter() {
        if let Some(start_pixel) = position_to_pixel(&start_position.as_position()) {
            if let Some(end_pixel) = position_to_pixel(&end_position.as_position()) {
                let frame = pixels_resource.pixels.get_frame();
                draw_line(frame, start_pixel, end_pixel, color.clone());
            }
        }
    }
}

fn draw_line(frame: &mut [u8], start: Pixel, end: Pixel, color: Color) {
    for (x, y) in line_drawing::Bresenham::new(
        (start.0 as isize, start.1 as isize),
        (end.0 as isize, end.1 as isize),
    ) {
        if x >= 0 && x < WIDTH as isize && y >= 0 && y < HEIGHT as isize {
            draw_pixel(frame, Pixel(x as u32, y as u32), color);
        }
    }
}

fn position_to_pixel(position: &Position) -> Option<Pixel> {
    let x = position.0;
    let y = position.1;
    if x >= 0.0 && x < WIDTH as f32 && y >= 0.0 && y < HEIGHT as f32 {
        Some(Pixel(x.floor() as u32, y.floor() as u32))
    } else {
        None
    }
}

fn pixel_to_frame_offset(pixel: Pixel) -> usize {
    (pixel.1 * WIDTH * 4 + pixel.0 * 4) as usize
}

fn draw_pixel(frame: &mut [u8], pixel: Pixel, color: Color) {
    let offset = pixel_to_frame_offset(pixel);
    frame[offset..offset + 4].copy_from_slice(&[color.0, color.1, color.2, color.3]);
}
