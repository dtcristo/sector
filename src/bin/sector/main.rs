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
        apply_player_look, apply_player_translation_input, move_player, render_frame,
        setup_player_system, update_current_sector, Minimap, Player, PlayerInput, HEIGHT, WIDTH,
        WINDOW_SCALE,
    },
    map::{map_to_sectors, SectorMap, SectorMapLoader},
    *,
};
use std::time::Duration;

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

fn switch_minimap_system(mut minimap: ResMut<MinimapMode>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Tab) {
        minimap.0 = match minimap.0 {
            Minimap::Off => Minimap::FirstPerson,
            Minimap::FirstPerson => Minimap::Absolute,
            Minimap::Absolute => Minimap::Off,
        };
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

    let input = PlayerInput::from_keys(&key).with_mouse_look(mouse_delta_x, cursor_locked);
    apply_player_look(&mut player, input);
}

fn player_translation_input_system(
    mut player_query: Query<&mut Player>,
    key: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    apply_player_translation_input(&mut player, PlayerInput::from_keys(&key));
}

fn move_player_system(mut player_query: Query<&mut Player>) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    move_player(&mut player);
}

fn update_current_sector_system(
    mut player_query: Query<&mut Player>,
    sector_query: Query<&Sector>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    update_current_sector(&mut player, sector_query.iter());
}

fn draw_frame_system(
    minimap: Res<MinimapMode>,
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
        player,
        sector_query.iter(),
        minimap.0,
    );
}
