#[cfg(not(target_arch = "wasm32"))]
use bevy::{app::AppExit, ecs::message::MessageWriter};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::message::MessageReader,
    input::mouse::MouseMotion,
    input::ButtonInput,
    prelude::*,
    window::{
        CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResizeConstraints,
        WindowResolution,
    },
};
use bevy_pixels::prelude::*;
use ron::ser::PrettyConfig;
use sector::{
    game::{
        apply_player_look, player_render_view, resolve_player_sector, sector_contains_player,
        setup_player_system, simulate_player, Player, PlayerInput,
    },
    map::{load_map_from_path, map_to_sectors, shipped_map_path},
    render::{
        render_frame_with_metrics, render_frame_with_metrics_and_timings, Automap, RenderMetrics,
        RenderTimings, HEIGHT, WIDTH, WINDOW_SCALE,
    },
    *,
};
use serde::Serialize;
#[cfg(not(target_arch = "wasm32"))]
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

const FIXED_SIMULATION_HZ: f64 = 120.0;
const FLY_MODE_DOUBLE_TAP_WINDOW_SECONDS: f64 = 0.3;

#[derive(Resource, Debug, PartialEq)]
struct AutomapMode(Automap);

#[derive(Resource, Debug)]
struct WindowTitleTimer(Timer);

#[derive(Resource, Debug)]
struct RendererTimingState {
    enabled: bool,
    sampled_frames: u32,
    accumulated: RenderTimings,
}

impl Default for RendererTimingState {
    fn default() -> Self {
        Self {
            enabled: render_timings_enabled_by_default(),
            sampled_frames: 0,
            accumulated: RenderTimings::default(),
        }
    }
}

impl RendererTimingState {
    fn reset(&mut self) {
        self.sampled_frames = 0;
        self.accumulated = RenderTimings::default();
    }

    fn record(&mut self, timings: &RenderTimings, metrics: &RenderMetrics) {
        self.accumulated.accumulate(timings);
        self.sampled_frames += 1;
        if self.sampled_frames < 60 {
            return;
        }

        let average = |duration: std::time::Duration| -> f64 {
            duration.as_secs_f64() * 1000.0 / self.sampled_frames as f64
        };
        let total_ms = average(self.accumulated.total);
        let fps = if total_ms <= f64::EPSILON {
            0.0
        } else {
            1000.0 / total_ms
        };
        println!(
            "sector: render timings [{}x{}] {:.2} ms ({:.0} fps) | clear {:.2} | world setup {:.2}, roots {:.2}, tree {:.2}, outlines {:.2} | automap {:.2}",
            metrics.width,
            metrics.height,
            total_ms,
            fps,
            average(self.accumulated.clear),
            average(self.accumulated.world.setup),
            average(self.accumulated.world.root_sectors),
            average(self.accumulated.world.sector_tree),
            average(self.accumulated.world.outlines),
            average(self.accumulated.automap),
        );
        self.reset();
    }
}

#[derive(Resource, Debug, Default)]
struct FlyToggleTracker {
    last_space_tap_seconds: Option<f64>,
}

#[derive(Resource, Debug, Clone)]
struct RuntimeMapPath(PathBuf);

#[derive(Resource, Debug, Clone)]
struct RuntimeSectors(Vec<Sector>);

impl RuntimeSectors {
    fn as_slice(&self) -> &[Sector] {
        &self.0
    }
}

#[derive(States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum CursorCaptureState {
    #[default]
    Released,
    Captured,
}

struct SectorRuntimePlugin;

impl Plugin for SectorRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<CursorCaptureState>()
            .insert_resource(AutomapMode(Automap::Off))
            .insert_resource(RendererTimingState::default())
            .insert_resource(FlyToggleTracker::default())
            .insert_resource(WindowTitleTimer(Timer::new(
                Duration::from_millis(500),
                TimerMode::Repeating,
            )))
            .insert_resource(Time::<Fixed>::from_hz(FIXED_SIMULATION_HZ))
            .add_systems(Startup, (setup_player_system, init_runtime_system).chain())
            .add_systems(
                OnEnter(CursorCaptureState::Captured),
                apply_captured_cursor_system,
            )
            .add_systems(
                OnEnter(CursorCaptureState::Released),
                apply_released_cursor_system,
            )
            .add_systems(
                Update,
                (
                    update_title_system,
                    request_cursor_capture_system,
                    release_cursor_with_mouse_system,
                    escape_system,
                    switch_automap_system,
                    toggle_renderer_timings_system,
                    toggle_noclip_system,
                    sync_pixel_buffer_system,
                    player_look_system,
                    dump_runtime_state_system,
                ),
            )
            .add_systems(
                FixedUpdate,
                player_simulation_system.run_if(in_state(CursorCaptureState::Captured)),
            )
            .add_systems(Draw, draw_frame_system);
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let map_path = runtime_map_path_from_args();

    let mut app = App::new();
    app.register_type::<SectorId>()
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
        .insert_resource(RuntimeMapPath(map_path))
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
                        present_mode: PresentMode::AutoNoVsync,
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(FrameTimeDiagnosticsPlugin::default());

    configure_runtime_plugins(&mut app);

    app.run();
}

fn configure_runtime_plugins(app: &mut App) {
    app.add_plugins(runtime_pixels_plugin())
        .add_plugins(SectorRuntimePlugin);
}

fn runtime_pixels_plugin() -> PixelsPlugin {
    PixelsPlugin {
        primary_window: Some(PixelsOptions {
            width: WIDTH,
            height: HEIGHT,
            auto_resize_buffer: false,
            auto_resize_surface: true,
            ..default()
        }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_timings_enabled_by_default() -> bool {
    env::var_os("SECTOR_RENDER_TIMINGS").is_some()
}

#[cfg(target_arch = "wasm32")]
fn render_timings_enabled_by_default() -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime_map_path_from_args() -> PathBuf {
    let mut args = env::args_os();
    let _ = args.next();
    let map_path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        panic!("usage: cargo run --features sector --bin sector -- [map-path]");
    }

    map_path.unwrap_or_else(|| shipped_map_path("default"))
}

#[cfg(target_arch = "wasm32")]
fn runtime_map_path_from_args() -> PathBuf {
    let window = web_sys::window().expect("browser runtime should have a window");
    let location = window.location();
    let pathname = location.pathname().unwrap_or_default();
    runtime_map_path_from_web_route(&pathname)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn runtime_map_path_from_web_route(pathname: &str) -> PathBuf {
    runtime_map_path_from_name(&map_name_from_route(pathname))
}

#[cfg(any(test, target_arch = "wasm32"))]
fn runtime_map_path_from_name(map_name: &str) -> PathBuf {
    shipped_map_path(map_name)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn map_name_from_route(pathname: &str) -> String {
    map_name_from_route_component(pathname).unwrap_or_else(|| "default".to_owned())
}

#[cfg(any(test, target_arch = "wasm32"))]
fn map_name_from_route_component(route: &str) -> Option<String> {
    let trimmed = route.trim();
    let trimmed = trimmed.trim_matches('/');
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("index.html") {
        return None;
    }

    let map_name = trimmed.rsplit('/').find(|segment| !segment.is_empty())?;
    assert!(
        map_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_')),
        "map routes may only use ASCII letters, digits, '-' and '_'"
    );
    Some(map_name.to_ascii_lowercase())
}

fn init_runtime_system(world: &mut World) {
    let map_path = world.resource::<RuntimeMapPath>().0.clone();
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

    world.insert_resource(RuntimeSectors(sectors));

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

fn sync_pixel_buffer_system(
    mut window_query: Query<(&Window, &mut PixelsOptions, &mut PixelsWrapper), With<PrimaryWindow>>,
) {
    let Ok((window, mut pixels_options, mut wrapper)) = window_query.single_mut() else {
        return;
    };

    let (width, height) = RenderMetrics::buffer_size_for_window(window.width(), window.height());
    if pixels_options.width == width && pixels_options.height == height {
        return;
    }

    pixels_options.width = width;
    pixels_options.height = height;
    wrapper
        .pixels
        .resize_buffer(width, height)
        .expect("failed to resize pixel buffer");
}

fn request_cursor_capture_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    cursor_capture_state: Res<State<CursorCaptureState>>,
    mut next_cursor_capture_state: ResMut<NextState<CursorCaptureState>>,
) {
    if *cursor_capture_state.get() == CursorCaptureState::Released
        && mouse_button.just_pressed(MouseButton::Left)
    {
        next_cursor_capture_state.set(CursorCaptureState::Captured);
    }
}

fn release_cursor_with_mouse_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    cursor_capture_state: Res<State<CursorCaptureState>>,
    mut next_cursor_capture_state: ResMut<NextState<CursorCaptureState>>,
) {
    if *cursor_capture_state.get() == CursorCaptureState::Captured
        && mouse_button.just_pressed(MouseButton::Right)
    {
        next_cursor_capture_state.set(CursorCaptureState::Released);
    }
}

fn apply_captured_cursor_system(mut cursor_options_query: Query<&mut CursorOptions>) {
    let Ok(mut cursor_options) = cursor_options_query.single_mut() else {
        return;
    };

    cursor_options.grab_mode = CursorGrabMode::Locked;
    cursor_options.visible = false;
}

fn apply_released_cursor_system(mut cursor_options_query: Query<&mut CursorOptions>) {
    let Ok(mut cursor_options) = cursor_options_query.single_mut() else {
        return;
    };

    cursor_options.grab_mode = CursorGrabMode::None;
    cursor_options.visible = true;
}

fn escape_system(
    #[cfg(not(target_arch = "wasm32"))] mut app_exit_events: MessageWriter<AppExit>,
    key: Res<ButtonInput<KeyCode>>,
    cursor_capture_state: Res<State<CursorCaptureState>>,
    mut next_cursor_capture_state: ResMut<NextState<CursorCaptureState>>,
) {
    if key.just_pressed(KeyCode::Escape) {
        if *cursor_capture_state.get() == CursorCaptureState::Released {
            #[cfg(not(target_arch = "wasm32"))]
            app_exit_events.write(AppExit::Success);
        } else {
            next_cursor_capture_state.set(CursorCaptureState::Released);
        }
    }
}

fn switch_automap_system(mut automap: ResMut<AutomapMode>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Tab) {
        automap.0 = automap.0.next();
    }
}

fn toggle_renderer_timings_system(
    mut timing_state: ResMut<RendererTimingState>,
    key: Res<ButtonInput<KeyCode>>,
) {
    if !key.just_pressed(KeyCode::F3) {
        return;
    }

    timing_state.enabled = !timing_state.enabled;
    timing_state.reset();
    println!(
        "sector: renderer timings {}",
        if timing_state.enabled {
            "enabled (averages print every 60 frames)"
        } else {
            "disabled"
        }
    );
}

#[derive(Debug, Serialize, PartialEq)]
struct RuntimeStateDump {
    map_path: String,
    sector_count: usize,
    resolved_sector: Option<u32>,
    player: PlayerStateDump,
    current_sector: Option<SectorStateDump>,
}

#[derive(Debug, Serialize, PartialEq)]
struct PlayerStateDump {
    feet_position: [f32; 3],
    eye_position: [f32; 3],
    velocity: [f32; 3],
    horizontal_speed: f32,
    vertical_speed: f32,
    direction_radians: f32,
    direction_degrees: f32,
    grounded: bool,
    crouching: bool,
    noclip: bool,
    fly_mode: bool,
    current_sector: Option<u32>,
}

#[derive(Debug, Serialize, PartialEq)]
struct SectorStateDump {
    id: u32,
    floor: f32,
    ceil: f32,
    headroom: f32,
    no_ceiling: bool,
    sky_color: Option<[u8; 3]>,
    floor_color: [u8; 3],
    ceil_color: [u8; 3],
    vertices: Vec<[f32; 2]>,
    walls: Vec<WallStateDump>,
}

#[derive(Debug, Serialize, PartialEq)]
struct WallStateDump {
    index: usize,
    start: [f32; 2],
    end: [f32; 2],
    color: [u8; 3],
    upper_color: Option<[u8; 3]>,
    lower_color: Option<[u8; 3]>,
    portal: Option<PortalStateDump>,
}

#[derive(Debug, Serialize, PartialEq)]
struct PortalStateDump {
    target_sector: u32,
    walkable: bool,
    target_floor: f32,
    target_ceil: f32,
    target_no_ceiling: bool,
}

fn debug_dump_requested(keys: &ButtonInput<KeyCode>) -> bool {
    keys.just_pressed(KeyCode::Slash)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
}

fn dump_runtime_state_system(
    map_path: Res<RuntimeMapPath>,
    runtime_sectors: Res<RuntimeSectors>,
    key: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Player>,
) {
    if !debug_dump_requested(&key) {
        return;
    }

    let Ok(player) = player_query.single() else {
        return;
    };
    let dump = build_runtime_state_dump(&map_path.0, player, runtime_sectors.as_slice());
    let pretty = PrettyConfig::default()
        .struct_names(true)
        .enumerate_arrays(true)
        .separate_tuple_members(true);
    let ron = ron::ser::to_string_pretty(&dump, pretty)
        .expect("runtime state dump should serialize to RON");
    println!("runtime_state: {ron}");
}

fn toggle_noclip_system(mut player_query: Query<&mut Player>, key: Res<ButtonInput<KeyCode>>) {
    if !key.just_pressed(KeyCode::KeyN) {
        return;
    }

    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    player.noclip = !player.noclip;
    player.velocity.z = 0.0;
    println!(
        "movement: noclip {}",
        if player.noclip { "enabled" } else { "disabled" }
    );
}

fn space_double_tap_detected(tracker: &mut FlyToggleTracker, now_seconds: f64) -> bool {
    let toggled = tracker
        .last_space_tap_seconds
        .is_some_and(|last_space_tap| {
            now_seconds - last_space_tap <= FLY_MODE_DOUBLE_TAP_WINDOW_SECONDS
        });
    tracker.last_space_tap_seconds = if toggled { None } else { Some(now_seconds) };
    toggled
}

fn build_runtime_state_dump(
    map_path: &Path,
    player: &Player,
    sectors: &[Sector],
) -> RuntimeStateDump {
    let resolved_sector = resolve_player_sector(player, sectors).map(|sector_id| sector_id.0);
    let current_sector = player
        .current_sector
        .and_then(|sector_id| sectors.iter().find(|sector| sector.id == sector_id))
        .map(|sector| build_sector_state_dump(sector, sectors));

    RuntimeStateDump {
        map_path: map_path.display().to_string(),
        sector_count: sectors.len(),
        resolved_sector,
        player: PlayerStateDump {
            feet_position: vec3_components(player.position.0),
            eye_position: vec3_components(player.eye_position().0),
            velocity: vec3_components(player.velocity),
            horizontal_speed: player.velocity.truncate().length(),
            vertical_speed: player.velocity.z,
            direction_radians: player.direction.0,
            direction_degrees: player.direction.0.to_degrees(),
            grounded: player.grounded,
            crouching: player.crouching,
            noclip: player.noclip,
            fly_mode: player.fly_mode,
            current_sector: player.current_sector.map(|sector_id| sector_id.0),
        },
        current_sector,
    }
}

fn build_sector_state_dump(sector: &Sector, sectors: &[Sector]) -> SectorStateDump {
    let walls = (0..sector.vertices.len())
        .map(|index| {
            let start = sector.vertices[index].0;
            let end = sector.vertices[(index + 1) % sector.vertices.len()].0;
            let portal = sector.portal_sectors[index].and_then(|sector_id| {
                sectors
                    .iter()
                    .find(|candidate| candidate.id == sector_id)
                    .map(|target| PortalStateDump {
                        target_sector: target.id.0,
                        walkable: sector.portal_walkable[index],
                        target_floor: target.floor.0,
                        target_ceil: target.ceil.0,
                        target_no_ceiling: target.no_ceiling,
                    })
            });

            WallStateDump {
                index,
                start: vec2_components(start),
                end: vec2_components(end),
                color: sector.colors[index].0,
                upper_color: sector.portal_upper_colors[index].map(|color| color.0),
                lower_color: sector.portal_lower_colors[index].map(|color| color.0),
                portal,
            }
        })
        .collect();

    SectorStateDump {
        id: sector.id.0,
        floor: sector.floor.0,
        ceil: sector.ceil.0,
        headroom: sector.ceil.0 - sector.floor.0,
        no_ceiling: sector.no_ceiling,
        sky_color: sector.sky_color.map(|color| color.0),
        floor_color: sector.floor_color.0,
        ceil_color: sector.ceil_color.0,
        vertices: sector
            .vertices
            .iter()
            .map(|vertex| vec2_components(vertex.0))
            .collect(),
        walls,
    }
}

fn vec2_components(vector: Vec2) -> [f32; 2] {
    [vector.x, vector.y]
}

fn vec3_components(vector: Vec3) -> [f32; 3] {
    [vector.x, vector.y, vector.z]
}

fn player_look_system(
    mut player_query: Query<&mut Player>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    key: Res<ButtonInput<KeyCode>>,
    cursor_capture_state: Res<State<CursorCaptureState>>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let mouse_delta_x: f32 = mouse_motion_events
        .read()
        .map(|motion| motion.delta.x)
        .sum();
    let cursor_locked = *cursor_capture_state.get() == CursorCaptureState::Captured;

    let input = PlayerInput::from_keys(&key, key.just_pressed(KeyCode::Space))
        .with_mouse_look(mouse_delta_x, cursor_locked);
    apply_player_look(&mut player, input);
}

fn player_simulation_system(
    mut player_query: Query<&mut Player>,
    key: Res<ButtonInput<KeyCode>>,
    mut fly_toggle_tracker: ResMut<FlyToggleTracker>,
    time: Res<Time<Fixed>>,
    runtime_sectors: Res<RuntimeSectors>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let jump_just_pressed = key.just_pressed(KeyCode::Space);
    let toggled_fly_mode = jump_just_pressed
        && space_double_tap_detected(&mut fly_toggle_tracker, time.elapsed_secs_f64());
    if toggled_fly_mode {
        player.fly_mode = !player.fly_mode;
        player.velocity.z = 0.0;
        println!(
            "movement: fly mode {}",
            if player.fly_mode {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    let input = PlayerInput::from_keys(
        &key,
        jump_just_pressed && !toggled_fly_mode && !player.fly_mode,
    );
    simulate_player(
        &mut player,
        input,
        time.delta_secs(),
        runtime_sectors.as_slice(),
    );
}

fn draw_frame_system(
    automap: Res<AutomapMode>,
    player_query: Query<&Player>,
    runtime_sectors: Res<RuntimeSectors>,
    mut timing_state: ResMut<RendererTimingState>,
    mut wrapper_query: Query<(&mut PixelsWrapper, &PixelsOptions)>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };
    let Ok((mut wrapper, pixels_options)) = wrapper_query.single_mut() else {
        return;
    };
    let metrics = RenderMetrics::new(pixels_options.width, pixels_options.height);
    let view = player_render_view(player);
    if timing_state.enabled {
        let mut frame_timings = RenderTimings::default();
        render_frame_with_metrics_and_timings(
            wrapper.pixels.frame_mut(),
            &metrics,
            &view,
            runtime_sectors.as_slice(),
            automap.0,
            &mut frame_timings,
        );
        timing_state.record(&frame_timings, &metrics);
    } else {
        render_frame_with_metrics(
            wrapper.pixels.frame_mut(),
            &metrics,
            &view,
            runtime_sectors.as_slice(),
            automap.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::{vec2, vec3};

    fn sector(id: u32) -> Sector {
        Sector {
            id: SectorId(id),
            vertices: vec![
                Position2(vec2(-1.0, 1.0)),
                Position2(vec2(1.0, 1.0)),
                Position2(vec2(1.0, -1.0)),
                Position2(vec2(-1.0, -1.0)),
            ],
            portal_sectors: vec![Some(SectorId(id + 1)), None, None, None],
            portal_walkable: vec![false, true, true, true],
            colors: vec![
                RawColor([10, 20, 30]),
                RawColor([40, 50, 60]),
                RawColor([70, 80, 90]),
                RawColor([100, 110, 120]),
            ],
            portal_upper_colors: vec![Some(RawColor([1, 2, 3])), None, None, None],
            portal_lower_colors: vec![Some(RawColor([4, 5, 6])), None, None, None],
            floor: Length(0.0),
            ceil: Length(4.0),
            floor_color: RawColor([11, 22, 33]),
            ceil_color: RawColor([44, 55, 66]),
            no_ceiling: true,
            sky_color: Some(RawColor([77, 88, 99])),
        }
    }

    #[test]
    fn debug_dump_requires_shift_slash() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Slash);
        assert!(!debug_dump_requested(&keys));

        let mut shifted_keys = ButtonInput::<KeyCode>::default();
        shifted_keys.press(KeyCode::ShiftLeft);
        shifted_keys.press(KeyCode::Slash);
        assert!(debug_dump_requested(&shifted_keys));
    }

    #[test]
    fn runtime_state_dump_includes_player_and_sector_details() {
        let current = sector(3);
        let mut target = sector(4);
        target.portal_sectors = vec![Some(SectorId(3)), None, None, None];
        target.portal_walkable = vec![false, true, true, true];
        target.no_ceiling = false;
        target.floor = Length(1.0);
        target.ceil = Length(5.0);

        let player = Player {
            position: Position3(vec3(0.0, 0.0, 0.5)),
            velocity: vec3(1.0, 2.0, 3.0),
            direction: sector::game::Direction(std::f32::consts::FRAC_PI_2),
            current_sector: Some(SectorId(3)),
            grounded: false,
            crouching: true,
            noclip: true,
            fly_mode: true,
        };
        let sectors = [current, target];

        let dump =
            build_runtime_state_dump(Path::new("assets/maps/test.map.ron"), &player, &sectors);

        assert_eq!(dump.map_path, "assets/maps/test.map.ron");
        assert_eq!(dump.sector_count, 2);
        assert_eq!(dump.resolved_sector, Some(3));
        assert_eq!(dump.player.current_sector, Some(3));
        assert_eq!(dump.player.velocity, [1.0, 2.0, 3.0]);
        assert!(dump.player.crouching);
        assert!(dump.player.noclip);
        assert!(dump.player.fly_mode);

        let current_sector = dump
            .current_sector
            .expect("current sector dump should exist");
        assert_eq!(current_sector.id, 3);
        assert!(current_sector.no_ceiling);
        assert_eq!(current_sector.sky_color, Some([77, 88, 99]));
        assert_eq!(current_sector.floor_color, [11, 22, 33]);
        assert_eq!(
            current_sector.walls[0].portal,
            Some(PortalStateDump {
                target_sector: 4,
                walkable: false,
                target_floor: 1.0,
                target_ceil: 5.0,
                target_no_ceiling: false,
            })
        );
    }

    #[test]
    fn web_route_defaults_to_default_map() {
        assert_eq!(map_name_from_route("/"), "default");
        assert_eq!(
            runtime_map_path_from_web_route("/"),
            shipped_map_path("default")
        );
    }

    #[test]
    fn web_route_uses_path_segment() {
        assert_eq!(map_name_from_route("/e1m1"), "e1m1");
        assert_eq!(
            runtime_map_path_from_web_route("/e1m1"),
            shipped_map_path("e1m1")
        );
    }

    #[test]
    fn web_route_ignores_hash_fallback() {
        assert_eq!(map_name_from_route("/index.html"), "default");
    }

    #[test]
    fn runtime_plugin_setup_preserves_draw_schedule_systems() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);

        configure_runtime_plugins(&mut app);

        assert_eq!(
            app.get_schedule(Draw)
                .expect("pixels plugin should install the draw schedule")
                .systems_len(),
            1,
            "runtime draw schedule should keep the frame blit system"
        );
    }

    #[test]
    fn fly_mode_toggle_requires_two_space_taps_within_window() {
        let mut tracker = FlyToggleTracker::default();

        assert!(!space_double_tap_detected(&mut tracker, 1.0));
        assert!(space_double_tap_detected(&mut tracker, 1.2));
        assert!(!space_double_tap_detected(&mut tracker, 2.0));
        assert!(!space_double_tap_detected(
            &mut tracker,
            2.0 + FLY_MODE_DOUBLE_TAP_WINDOW_SECONDS + 0.05
        ));
    }
}
