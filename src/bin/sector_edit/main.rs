use sector::*;

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

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 960.0;

#[derive(Resource, Debug)]
struct State {
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
        .insert_resource(State {
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: "sector_edit".to_string(),
                width: WIDTH,
                height: HEIGHT,
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_plugin(EguiPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(init_scene_system)
        .add_system(save_scene_system)
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
    mut state: ResMut<State>,
    mut windows: ResMut<Windows>,
    time: Res<Time>,
    diagnostics: Res<Diagnostics>,
) {
    if state.update_title_timer.tick(time.delta()).finished() {
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

fn egui_system(
    mut egui_ctx: ResMut<EguiContext>,
    mut _state: ResMut<State>,
    mut sector_query: Query<&mut Sector>,
) {
    egui_ctx.ctx_mut().set_visuals(egui::Visuals::light());

    let mut highligted_sector: Option<SectorId> = None;
    let mut highligted_wall: Option<Wall> = None;
    let mut highligted_vertex: Option<Position2> = None;

    // egui::TopBottomPanel::top("top_panel").show(egui_ctx.ctx_mut(), |ui| {
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
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("🔷 sector_edit");
            });

            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for mut sector in &mut sector_query {
                        let sector_frame_response = egui::Frame::none()
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
                                            .clamp_range(-10.0..=(10.0 - 0.1))
                                            .prefix("floor: "),
                                    );
                                    let floor = sector.floor.0;
                                    ui.add(
                                        egui::DragValue::new(&mut sector.ceil.0)
                                            .speed(0.1)
                                            .clamp_range((floor + 0.1)..=10.0)
                                            .prefix("ceil: "),
                                    );

                                    egui::CollapsingHeader::new("walls")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for (i, wall) in sector.to_walls().iter().enumerate() {
                                                let wall_response = egui::Frame::none()
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
                                                                        .clamp_range(-100.0..=100.0)
                                                                        .prefix("x: "),
                                                                    );
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut y,
                                                                        )
                                                                        .speed(0.1)
                                                                        .clamp_range(-100.0..=100.0)
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
                                                                        .clamp_range(-100.0..=100.0)
                                                                        .prefix("x: "),
                                                                    );
                                                                    ui.add(
                                                                        egui::DragValue::new(
                                                                            &mut y,
                                                                        )
                                                                        .speed(0.1)
                                                                        .clamp_range(-100.0..=100.0)
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
                                                                    wall.raw_color.0[0],
                                                                    wall.raw_color.0[1],
                                                                    wall.raw_color.0[2],
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
                                                                .clamp_range(-100.0..=100.0)
                                                                .prefix("x: "),
                                                        );
                                                        ui.add(
                                                            egui::DragValue::new(&mut vertex.0.y)
                                                                .speed(0.1)
                                                                .clamp_range(-100.0..=100.0)
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

    let polygons: Vec<egui::plot::Polygon> = sector_query
        .iter()
        .map(|sector| {
            let highlighted =
                highligted_sector.is_some() && highligted_sector.unwrap() == sector.id;

            egui::plot::Polygon::new(egui::plot::PlotPoints::new(
                sector
                    .vertices
                    .iter()
                    .map(|v| [v.0.x as f64, v.0.y as f64])
                    .collect(),
            ))
            .highlight(highlighted)
        })
        .collect();

    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(egui_ctx.ctx_mut(), |ui| {
            egui::plot::Plot::new("plot")
                .data_aspect(1.0)
                .show_axes([true, true])
                .auto_bounds_x()
                .show(ui, |plot_ui| {
                    for polygon in polygons {
                        plot_ui.polygon(polygon);
                    }

                    if highligted_wall.is_some() {
                        let wall = highligted_wall.unwrap();
                        let wall_points = egui::plot::PlotPoints::new(vec![
                            [wall.left.0.x as f64, wall.left.0.y as f64],
                            [wall.right.0.x as f64, wall.right.0.y as f64],
                        ]);
                        let wall_color32 = egui::Color32::from_rgb(
                            wall.raw_color.0[0],
                            wall.raw_color.0[1],
                            wall.raw_color.0[2],
                        );
                        plot_ui.line(
                            egui::plot::Line::new(wall_points)
                                .color(wall_color32)
                                .highlight(true)
                                .width(2.0),
                        );
                    }

                    if highligted_vertex.is_some() {
                        let vertex = highligted_vertex.unwrap();
                        plot_ui.points(
                            egui::plot::Points::new(vec![[vertex.0.x as f64, vertex.0.y as f64]])
                                .color(egui::Color32::BLUE)
                                .filled(true)
                                .radius(6.0)
                                .highlight(true)
                                .shape(egui::widgets::plot::MarkerShape::Diamond),
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
