use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::message::{MessageReader, MessageWriter},
    input::mouse::MouseMotion,
    input::ButtonInput,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, WindowResizeConstraints, WindowResolution},
};
use bevy_pixels::prelude::*;
use sector::{
    game::{
        apply_player_look, player_render_view, sector_contains_player, setup_player_system,
        simulate_player, Player, PlayerInput,
    },
    map::{load_map_from_path, map_to_sectors},
    render::{render_frame, Automap, HEIGHT, WIDTH, WINDOW_SCALE},
    *,
};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Resource, Debug, PartialEq)]
struct AutomapMode(Automap);

#[derive(Resource, Debug)]
struct WindowTitleTimer(Timer);

struct SectorRuntimePlugin;

impl Plugin for SectorRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AutomapMode(Automap::Off))
            .insert_resource(WindowTitleTimer(Timer::new(
                Duration::from_millis(500),
                TimerMode::Repeating,
            )))
            .add_systems(Startup, (setup_player_system, init_runtime_system).chain())
            .add_systems(
                Update,
                (
                    update_title_system,
                    mouse_capture_system,
                    escape_system,
                    switch_automap_system,
                    (player_look_system, player_simulation_system).chain(),
                ),
            )
            .add_systems(Draw, draw_frame_system);
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

fn init_runtime_system(world: &mut World) {
    let map_path = PathBuf::from("assets").join(DEFAULT_MAP_FILE_PATH);
    println!("sector: loading map from {}", map_path.display());

    let map = load_map_from_path(&map_path)
        .unwrap_or_else(|error| panic!("failed to load map from {}: {error}", map_path.display()));
    let (initial_sector, sectors) = map_to_sectors(&map).unwrap_or_else(|error| {
        panic!("failed to convert map from {}: {error}", map_path.display())
    });

    let initial_sector_data = sectors
        .iter()
        .find(|sector| sector.id == initial_sector.0)
        .expect("initial sector must exist in converted map");
    let spawn_position = Vec3::new(
        map.initial_position.0,
        map.initial_position.1,
        initial_sector_data.floor.0,
    );

    {
        let mut player_query = world.query::<&mut Player>();
        let mut player = player_query
            .single_mut(world)
            .expect("player should exist before runtime initialization");
        player.position = Position3(spawn_position);
        player.direction.0 = map.initial_direction_radians();
        player.current_sector = Some(initial_sector.0);
        player.grounded = true;

        assert!(
            sector_contains_player(initial_sector_data, player.position),
            "initial spawn must be inside initial sector"
        );
    }

    println!(
        "loaded {} sectors; spawn sector={} position=({:.2}, {:.2}, {:.2}) direction={:.1}deg",
        sectors.len(),
        initial_sector.0 .0,
        spawn_position.x,
        spawn_position.y,
        spawn_position.z,
        map.initial_direction_degrees,
    );

    world.spawn(initial_sector);
    for sector in sectors {
        world.spawn(sector);
    }

    println!("sector: runtime ready");
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
    } else if mouse_button.just_pressed(MouseButton::Right) {
        cursor_options.grab_mode = CursorGrabMode::None;
        cursor_options.visible = true;
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

fn switch_automap_system(mut automap: ResMut<AutomapMode>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Tab) {
        automap.0 = automap.0.next();
    }
}

fn player_look_system(
    mut player_query: Query<&mut Player>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    key: Res<ButtonInput<KeyCode>>,
    cursor_options_query: Query<&CursorOptions>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let mouse_delta_x: f32 = mouse_motion_events
        .read()
        .map(|motion| motion.delta.x)
        .sum();
    let cursor_locked = cursor_options_query
        .single()
        .map(|cursor_options| cursor_options.grab_mode == CursorGrabMode::Locked)
        .unwrap_or(false);

    let input = PlayerInput::from_keys(&key, key.just_pressed(KeyCode::Space))
        .with_mouse_look(mouse_delta_x, cursor_locked);
    apply_player_look(&mut player, input);
}

fn player_simulation_system(
    mut player_query: Query<&mut Player>,
    key: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    sector_query: Query<&Sector>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let input = PlayerInput::from_keys(&key, key.just_pressed(KeyCode::Space));
    simulate_player(&mut player, input, time.delta_secs(), sector_query.iter());
}

fn draw_frame_system(
    automap: Res<AutomapMode>,
    player_query: Query<&Player>,
    sector_query: Query<&Sector>,
    mut wrapper_query: Query<&mut PixelsWrapper>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };
    let Ok(mut wrapper) = wrapper_query.single_mut() else {
        return;
    };
    render_frame(
        wrapper.pixels.frame_mut(),
        &player_render_view(player),
        sector_query.iter(),
        automap.0,
    );
}
