use sector::*;

use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::message::MessageWriter,
    input::ButtonInput,
    prelude::*,
    tasks::IoTaskPool,
    window::WindowResolution,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use egui_plot::{Line as PlotLine, MarkerShape, Plot, PlotPoints, Points, Polygon};
use sector::map::{
    load_map_from_path, map_to_sectors, save_map_to_path, sectors_to_map_with_spawn, MapVertex,
};
use std::path::PathBuf;
use std::time::Duration;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 960;

#[derive(Resource, Debug)]
struct State {
    update_title_timer: Timer,
}

#[derive(Resource, Debug, Clone, Copy)]
struct MapMetadata {
    initial_position: MapVertex,
    initial_direction_degrees: f32,
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
        .insert_resource(State {
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "sector_edit".to_string(),
                resolution: WindowResolution::new(WIDTH, HEIGHT),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, init_scene_system)
        .add_systems(
            Update,
            (save_scene_system, update_title_system, escape_system),
        )
        .add_systems(EguiPrimaryContextPass, egui_system)
        .run();
}

fn init_scene_system(world: &mut World) {
    world.spawn(Camera2d);

    let map_path = PathBuf::from("assets").join(DEFAULT_MAP_FILE_PATH);
    let map = load_map_from_path(&map_path)
        .unwrap_or_else(|error| panic!("failed to load map from {}: {error}", map_path.display()));
    let (initial_sector, sectors) = map_to_sectors(&map).unwrap_or_else(|error| {
        panic!("failed to convert map from {}: {error}", map_path.display())
    });

    world.insert_resource(MapMetadata {
        initial_position: map.initial_position,
        initial_direction_degrees: map.initial_direction_degrees,
    });

    world.spawn(initial_sector);
    for sector in sectors {
        world.spawn(sector);
    }
}

fn save_scene_system(world: &mut World) {
    let mut initial_sector_query = world.query::<Ref<InitialSector>>();
    let Ok(initial_sector) = initial_sector_query.single(world) else {
        return;
    };
    let Some(metadata) = world.get_resource::<MapMetadata>().copied() else {
        return;
    };
    let initial_sector_id = initial_sector.0;
    let initial_sector_changed = initial_sector.is_changed();

    let mut sector_query = world.query::<Ref<Sector>>();
    let sector_refs: Vec<_> = sector_query.iter(world).collect();
    let should_save =
        initial_sector_changed || sector_refs.iter().any(|sector| sector.is_changed());

    if !should_save {
        return;
    }

    let sectors: Vec<_> = sector_refs
        .into_iter()
        .map(|sector| (*sector).clone())
        .collect();
    let map = sectors_to_map_with_spawn(
        initial_sector_id,
        metadata.initial_position,
        metadata.initial_direction_degrees,
        &sectors,
    )
    .expect("editor sectors should save cleanly");
    let map_path = PathBuf::from("assets").join(DEFAULT_MAP_FILE_PATH);

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            save_map_to_path(&map, &map_path).unwrap_or_else(|error| {
                panic!("failed to save map to {}: {error}", map_path.display())
            });
        })
        .detach();
}

fn update_title_system(
    mut state: ResMut<State>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut window_query: Query<&mut Window>,
) {
    if state.update_title_timer.tick(time.delta()).is_finished() {
        let Ok(mut window) = window_query.single_mut() else {
            return;
        };

        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.value() {
                window.title = format!("sector_edit: {value:.0} fps");
            }
        }
    }
}

fn escape_system(mut app_exit_events: MessageWriter<AppExit>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Escape) {
        app_exit_events.write(AppExit::Success);
    }
}

fn egui_system(
    mut contexts: EguiContexts,
    mut _state: ResMut<State>,
    mut sector_query: Query<&mut Sector>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    ctx.set_visuals(egui::Visuals::light());

    let mut highligted_sector: Option<SectorId> = None;
    let mut highligted_wall: Option<WallSegment> = None;
    let mut highligted_vertex: Option<Position2> = None;

    // egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
    //     egui::menu::bar(ui, |ui| {
    //         ui.menu_button("File", |ui| {
    //             if ui.button("About...").clicked() {
    //                 ui.close_menu();
    //             }
    //         })
    //     });
    // });

    egui::SidePanel::left("left_panel")
        .default_width(250.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("🔷 sector_edit");
            });

            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for mut sector in &mut sector_query {
                        let sector_frame_response = egui::Frame::NONE
                            .show(ui, |ui| {
                                egui::collapsing_header::CollapsingState::load_with_default_open(
                                    ui.ctx(),
                                    ui.make_persistent_id(format!("sector: {}", sector.id.0)),
                                    false,
                                )
                                .show_header(ui, |ui| {
                                    ui.checkbox(&mut true, format!("sector: {}", sector.id.0));
                                })
                                .body(|ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut sector.floor.0)
                                            .speed(0.1)
                                            .range(-10.0..=(10.0 - 0.1))
                                            .prefix("floor: "),
                                    );
                                    let floor = sector.floor.0;
                                    ui.add(
                                        egui::DragValue::new(&mut sector.ceil.0)
                                            .speed(0.1)
                                            .range((floor + 0.1)..=10.0)
                                            .prefix("ceil: "),
                                    );

                                    egui::CollapsingHeader::new("walls")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for (i, wall) in
                                                sector.wall_segments().iter().enumerate()
                                            {
                                                let wall_response = egui::Frame::NONE
                                                    .show(ui, |ui| {
                                                        egui::CollapsingHeader::new(format!(
                                                            "wall {}",
                                                            i
                                                        ))
                                                        .default_open(true)
                                                        .show(ui, |ui| {
                                                            let vertex_response = ui
                                                                .horizontal(|ui| {
                                                                    ui.label(format!("left:"));
                                                                    let mut x = wall.left.0.x;
                                                                    let mut y = wall.left.0.y;
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut x,
                                                                        )
                                                                        .speed(0.1)
                                                                        .range(-100.0..=100.0)
                                                                        .prefix("x: "),
                                                                    );
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut y,
                                                                        )
                                                                        .speed(0.1)
                                                                        .range(-100.0..=100.0)
                                                                        .prefix("y: "),
                                                                    );
                                                                })
                                                                .response;

                                                            if vertex_response.hovered() {
                                                                highligted_vertex = Some(wall.left);
                                                            }

                                                            let vertex_response = ui
                                                                .horizontal(|ui| {
                                                                    ui.label(format!("right:"));
                                                                    let mut x = wall.right.0.x;
                                                                    let mut y = wall.right.0.y;
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut x,
                                                                        )
                                                                        .speed(0.1)
                                                                        .range(-100.0..=100.0)
                                                                        .prefix("x: "),
                                                                    );
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut y,
                                                                        )
                                                                        .speed(0.1)
                                                                        .range(-100.0..=100.0)
                                                                        .prefix("y: "),
                                                                    );
                                                                })
                                                                .response;

                                                            if vertex_response.hovered() {
                                                                highligted_vertex =
                                                                    Some(wall.right);
                                                            }

                                                            let mut color32 =
                                                                egui::Color32::from_rgb(
                                                                    wall.color.0[0],
                                                                    wall.color.0[1],
                                                                    wall.color.0[2],
                                                                );
                                                            ui.horizontal(|ui| {
                                                                ui.label("color:");
                                                                ui.color_edit_button_srgba(
                                                                    &mut color32,
                                                                );
                                                            })
                                                        });
                                                    })
                                                    .response;

                                                if wall_response.hovered() {
                                                    highligted_wall = Some(*wall);
                                                }
                                            }
                                        });

                                    egui::CollapsingHeader::new("vertices")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for vertex in &mut sector.vertices {
                                                let vertex_response = ui
                                                    .horizontal(|ui| {
                                                        ui.add(
                                                            egui::DragValue::new(&mut vertex.0.x)
                                                                .speed(0.1)
                                                                .range(-100.0..=100.0)
                                                                .prefix("x: "),
                                                        );
                                                        ui.add(
                                                            egui::DragValue::new(&mut vertex.0.y)
                                                                .speed(0.1)
                                                                .range(-100.0..=100.0)
                                                                .prefix("y: "),
                                                        );
                                                    })
                                                    .response;

                                                if vertex_response.hovered() {
                                                    highligted_vertex = Some(*vertex);
                                                }
                                            }
                                        });
                                });
                            })
                            .response;

                        if sector_frame_response.hovered() {
                            highligted_sector = Some(sector.id);
                        }
                    }
                });
        });

    let polygons: Vec<Polygon<'static>> = sector_query
        .iter()
        .map(|sector| {
            let highlighted =
                highligted_sector.is_some() && highligted_sector.unwrap() == sector.id;

            Polygon::new(
                format!("sector {}", sector.id.0),
                PlotPoints::new(
                    sector
                        .vertices
                        .iter()
                        .map(|v| [v.0.x as f64, v.0.y as f64])
                        .collect(),
                ),
            )
            .highlight(highlighted)
        })
        .collect();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            Plot::new("plot")
                .data_aspect(1.0)
                .show_axes([true, true])
                .auto_bounds([true, false])
                .show(ui, |plot_ui| {
                    for polygon in polygons {
                        plot_ui.polygon(polygon);
                    }

                    if highligted_wall.is_some() {
                        let wall = highligted_wall.unwrap();
                        let wall_points = PlotPoints::new(vec![
                            [wall.left.0.x as f64, wall.left.0.y as f64],
                            [wall.right.0.x as f64, wall.right.0.y as f64],
                        ]);
                        let wall_color32 = egui::Color32::from_rgb(
                            wall.color.0[0],
                            wall.color.0[1],
                            wall.color.0[2],
                        );
                        plot_ui.line(
                            PlotLine::new("highlighted wall", wall_points)
                                .color(wall_color32)
                                .highlight(true)
                                .width(2.0),
                        );
                    }

                    if highligted_vertex.is_some() {
                        let vertex = highligted_vertex.unwrap();
                        plot_ui.points(
                            Points::new(
                                "highlighted vertex",
                                vec![[vertex.0.x as f64, vertex.0.y as f64]],
                            )
                            .color(egui::Color32::BLUE)
                            .filled(true)
                            .radius(6.0)
                            .highlight(true)
                            .shape(MarkerShape::Diamond),
                        );
                    }

                    // if plot_ui.plot_clicked() {
                    //     println!("Clicked {:?}", plot_ui.pointer_coordinate().unwrap());
                    // }

                    // if plot_ui.plot_hovered() {
                    //     println!("Bounds {:?}", plot_ui.plot_bounds());
                    //     println!("Drag delta {:?}", plot_ui.pointer_coordinate_drag_delta());
                    // }
                });
        });
}
