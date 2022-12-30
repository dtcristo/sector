use sector_core::*;

use bevy::{
    app::AppExit,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    math::vec2,
    prelude::*,
    scene::serde::SceneSerializer,
    tasks::IoTaskPool,
    utils::Duration,
};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use palette::named::*;
use std::fs::File;
use std::io::Write;

const DEFAULT_SCENE_RON_FILE_PATH: &str = "scenes/default.scn.ron";
const DEFAULT_SCENE_MP_FILE_PATH: &str = "scenes/default.scn.mp";

#[derive(Resource, Debug)]
struct AppState {
    update_title_timer: Timer,
}

fn main() {
    App::new()
        .register_type::<SectorId>()
        .register_type::<Option<SectorId>>()
        .register_type::<Sector>()
        .register_type::<InitialSector>()
        .register_type::<Position2>()
        .register_type::<Length>()
        .register_type::<RawColor>()
        .insert_resource(AppState {
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: "sector_edit".to_string(),
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_plugin(EguiPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(init_scene_system)
        .add_startup_system(save_scene_system.after(init_scene_system))
        .add_system(update_title_system)
        .add_system(escape_system)
        .add_system(egui_system)
        .run();
}

fn init_scene_system(world: &mut World) {
    // Vertices
    let v0 = Position2(vec2(2.0, 10.0));
    let v1 = Position2(vec2(4.0, 10.0));
    let v2 = Position2(vec2(11.0, -8.0));
    let v3 = Position2(vec2(-4.0, -8.0));
    let v4 = Position2(vec2(-4.0, 1.0));
    let v5 = Position2(vec2(-2.0, 5.0));
    let v6 = Position2(vec2(-4.0, 15.0));
    let v7 = Position2(vec2(4.0, 15.0));
    let v8 = Position2(vec2(-7.0, -9.0));
    let v9 = Position2(vec2(-10.0, -5.0));

    // Spawn singleton component entity
    world.spawn(InitialSector(SectorId(0)));

    world.spawn(Sector {
        id: SectorId(0),
        vertices: vec![v0, v1, v2, v3, v4, v5],
        portal_sectors: vec![None, None, None, Some(SectorId(2)), None, Some(SectorId(1))],
        colors: vec![
            BLUE.into(),
            GREEN.into(),
            ORANGE.into(),
            FUCHSIA.into(),
            YELLOW.into(),
            RED.into(),
        ],
        floor: Length(0.0),
        ceil: Length(4.0),
    });

    world.spawn(Sector {
        id: SectorId(1),
        vertices: vec![v0, v5, v6, v7],
        portal_sectors: vec![Some(SectorId(0)), None, None, None],
        colors: vec![RED.into(), FUCHSIA.into(), GREEN.into(), YELLOW.into()],
        floor: Length(0.25),
        ceil: Length(3.75),
    });

    world.spawn(Sector {
        id: SectorId(2),
        vertices: vec![v4, v3, v8, v9],
        portal_sectors: vec![Some(SectorId(0)), None, None, None],
        colors: vec![RED.into(), FUCHSIA.into(), GREEN.into(), BLUE.into()],
        floor: Length(-0.5),
        ceil: Length(4.5),
    });
}

fn save_scene_system(world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>();
    let scene = DynamicScene::from_world(&world, type_registry);

    let scene_ron = scene.serialize_ron(type_registry).unwrap();
    info!("{}", scene_ron);

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/{DEFAULT_SCENE_RON_FILE_PATH}"))
                .and_then(|mut file| file.write(scene_ron.as_bytes()))
                .expect("failed to write `scene_ron` to file");
        })
        .detach();

    let scene_serializer = SceneSerializer::new(&scene, type_registry);
    let scene_mp: Vec<u8> = rmp_serde::to_vec(&scene_serializer).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/{DEFAULT_SCENE_MP_FILE_PATH}"))
                .and_then(|mut file| file.write(&scene_mp))
                .expect("failed to write `scene_mp` to file");
        })
        .detach();
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
                window.set_title(format!("sector_edit: {value:.0} fps"));
            }
        }
    }
}

fn escape_system(mut app_exit_events: EventWriter<AppExit>, key: Res<Input<KeyCode>>) {
    if key.just_pressed(KeyCode::Escape) {
        app_exit_events.send(AppExit);
    }
}

fn egui_system(mut egui_ctx: ResMut<EguiContext>) {
    egui_ctx.ctx_mut().set_visuals(egui::Visuals::light());

    // egui::TopBottomPanel::top("top_panel").show(egui_ctx.ctx_mut(), |ui| {
    //     egui::menu::bar(ui, |ui| {
    //         ui.menu_button("File", |ui| {
    //             if ui.button("About...").clicked() {
    //                 ui.close_menu();
    //             }
    //         })
    //     });
    // });

    let polygon_1 = egui::plot::Polygon::new(egui::plot::PlotPoints::new(vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
    ]));

    let polygon_2 = egui::plot::Polygon::new(egui::plot::PlotPoints::new(vec![
        [0.0, 0.0],
        [-1.0, 0.0],
        [-1.0, -1.0],
    ]));

    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(egui_ctx.ctx_mut(), |ui| {
            egui::plot::Plot::new("plot")
                .data_aspect(1.0)
                .show_axes([true, true])
                // .auto_bounds_x()
                .show(ui, |plot_ui| {
                    plot_ui.polygon(polygon_1);
                    plot_ui.polygon(polygon_2);

                    if plot_ui.plot_clicked() {
                        println!("{:?}", plot_ui.pointer_coordinate().unwrap());

                        println!("{:?}", plot_ui.pointer_coordinate_drag_delta());
                    }
                });
        });
}
