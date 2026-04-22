use sector::*;

use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::message::MessageWriter,
    input::ButtonInput,
    math::vec2,
    prelude::*,
    window::WindowResolution,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use egui_plot::{
    Line as PlotLine, MarkerShape, Plot, PlotBounds, PlotPoint, PlotPoints, Points, Polygon,
};
use rfd::FileDialog;
use sector::map::{
    load_map_from_path, map_to_sectors, save_map_to_path, sectors_to_map_with_spawn,
    shipped_map_path, MapVertex,
};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    time::Duration,
};

const WIDTH: u32 = 1440;
const HEIGHT: u32 = 960;
const VERTEX_PICK_RADIUS: f32 = 0.35;
const WALL_PICK_RADIUS: f32 = 0.25;
const SPAWN_ARROW_LENGTH: f32 = 0.9;
const GEOMETRY_EPSILON: f32 = 0.001;
const MIN_PLOT_HALF_EXTENT: f32 = 2.0;
const MAX_PLOT_HALF_EXTENT: f32 = 256.0;

#[derive(Resource, Debug, Clone)]
struct EditorDocument {
    sectors: Vec<Sector>,
    initial_sector: SectorId,
    initial_position: MapVertex,
    initial_direction_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapFileFormat {
    Ron,
    Protobuf,
}

impl MapFileFormat {
    fn label(self) -> &'static str {
        match self {
            Self::Ron => "RON",
            Self::Protobuf => "Protobuf",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Ron => ".map.ron",
            Self::Protobuf => ".map.pb",
        }
    }
}

#[derive(Resource, Debug)]
struct EditorState {
    update_title_timer: Timer,
    map_path: PathBuf,
    output_format: MapFileFormat,
    selected_sector: Option<SectorId>,
    selected_wall: Option<usize>,
    selected_vertex: Option<usize>,
    tool: EditorTool,
    draft_room: Vec<Vec2>,
    draft_style: DraftSectorStyle,
    dirty: bool,
    auto_portals_on_save: bool,
    restart_play_on_save: bool,
    last_plot_hover: Option<Vec2>,
    plot_center: Vec2,
    plot_half_extent: f32,
    pending_plot_bounds: Option<PlotBounds>,
    status: StatusMessage,
}

impl EditorState {
    fn new(map_path: PathBuf) -> Self {
        Self {
            update_title_timer: Timer::new(Duration::from_millis(500), TimerMode::Repeating),
            output_format: map_format_for_path(&map_path),
            map_path,
            selected_sector: None,
            selected_wall: None,
            selected_vertex: None,
            tool: EditorTool::Select,
            draft_room: Vec::new(),
            draft_style: DraftSectorStyle::default(),
            dirty: false,
            auto_portals_on_save: true,
            restart_play_on_save: false,
            last_plot_hover: None,
            plot_center: Vec2::ZERO,
            plot_half_extent: 16.0,
            pending_plot_bounds: None,
            status: StatusMessage::info("Loaded default map"),
        }
    }

    fn load_document(&mut self, document: &EditorDocument, path: PathBuf, status: StatusMessage) {
        self.map_path = path;
        self.output_format = map_format_for_path(&self.map_path);
        self.selected_sector = Some(document.initial_sector);
        self.selected_wall = None;
        self.selected_vertex = None;
        self.draft_room.clear();
        self.last_plot_hover = None;
        self.dirty = false;
        self.status = status;
        self.focus_view_on_spawn(document);
    }

    fn focus_view(&mut self, center: Vec2, half_extent: f32) {
        self.plot_center = center;
        self.plot_half_extent = half_extent.clamp(MIN_PLOT_HALF_EXTENT, MAX_PLOT_HALF_EXTENT);
        self.pending_plot_bounds = Some(plot_bounds_for_view(center, self.plot_half_extent));
    }

    fn focus_view_on_spawn(&mut self, document: &EditorDocument) {
        self.focus_view(spawn_position(document), default_plot_half_extent(document));
    }

    fn center_view_on_spawn(&mut self, document: &EditorDocument) {
        self.focus_view(spawn_position(document), self.plot_half_extent);
    }

    fn sync_view_from_bounds(&mut self, bounds: &PlotBounds) {
        if !bounds.is_valid() {
            return;
        }

        let center = bounds.center();
        self.plot_center = vec2(center.x as f32, center.y as f32);
        self.plot_half_extent = ((bounds.width().max(bounds.height()) * 0.5) as f32)
            .clamp(MIN_PLOT_HALF_EXTENT, MAX_PLOT_HALF_EXTENT);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum EditorTool {
    #[default]
    Select,
    DrawRoom,
    SetSpawn,
}

#[derive(Debug, Clone, Copy)]
struct DraftSectorStyle {
    floor: f32,
    ceil: f32,
    floor_color: RawColor,
    ceil_color: RawColor,
    wall_color: RawColor,
    no_ceiling: bool,
    sky_color: Option<RawColor>,
}

impl Default for DraftSectorStyle {
    fn default() -> Self {
        Self {
            floor: 0.0,
            ceil: 3.2,
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            wall_color: *MISSING_WALL_COLOR,
            no_ceiling: false,
            sky_color: None,
        }
    }
}

#[derive(Debug, Clone)]
struct StatusMessage {
    text: String,
    tone: StatusTone,
}

impl StatusMessage {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: StatusTone::Info,
        }
    }

    fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: StatusTone::Success,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: StatusTone::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusTone {
    Info,
    Success,
    Error,
}

#[derive(Debug, Default)]
struct PlaySession {
    child: Option<Child>,
}

#[derive(Debug, Clone, Copy, Default)]
struct PlotSelection {
    sector: Option<SectorId>,
    wall: Option<usize>,
    vertex: Option<usize>,
}

fn main() {
    App::new()
        .register_type::<SectorId>()
        .register_type::<Option<SectorId>>()
        .register_type::<Sector>()
        .register_type::<Position2>()
        .register_type::<Length>()
        .register_type::<RawColor>()
        .insert_resource(EditorState::new(shipped_map_path("default")))
        .insert_non_send_resource(PlaySession::default())
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
        .add_systems(Update, (update_title_system, escape_system))
        .add_systems(EguiPrimaryContextPass, egui_system)
        .run();
}

fn init_scene_system(mut commands: Commands, mut editor: ResMut<EditorState>) {
    commands.spawn(Camera2d);

    let document = load_editor_document(&editor.map_path).unwrap_or_else(|error| {
        panic!(
            "failed to load map from {}: {error}",
            editor.map_path.display()
        )
    });
    let loaded_path = editor.map_path.clone();
    editor.load_document(
        &document,
        loaded_path.clone(),
        StatusMessage::success(format!("Loaded {}", loaded_path.display())),
    );
    commands.insert_resource(document);
}

fn update_title_system(
    mut editor: ResMut<EditorState>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut window_query: Query<&mut Window>,
) {
    if !editor.update_title_timer.tick(time.delta()).is_finished() {
        return;
    }

    let Ok(mut window) = window_query.single_mut() else {
        return;
    };

    let file_name = editor
        .map_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("untitled");
    let dirty_marker = if editor.dirty { "*" } else { "" };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.value())
        .map(|fps| format!("{fps:.0} fps"))
        .unwrap_or_else(|| "fps n/a".to_owned());
    window.title = format!("sector_edit{dirty_marker} - {file_name} - {fps}");
}

fn escape_system(mut app_exit_events: MessageWriter<AppExit>, key: Res<ButtonInput<KeyCode>>) {
    if key.just_pressed(KeyCode::Escape) {
        app_exit_events.write(AppExit::Success);
    }
}

fn egui_system(
    mut contexts: EguiContexts,
    mut editor: ResMut<EditorState>,
    mut document: ResMut<EditorDocument>,
    mut play_session: NonSendMut<PlaySession>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    ctx.set_visuals(egui::Visuals::dark());

    poll_play_session(&mut play_session, &mut editor);
    sanitize_selection(&document, &mut editor);

    let keyboard_new =
        ctx.input(|input| input.key_pressed(egui::Key::N) && input.modifiers.command);
    let keyboard_open =
        ctx.input(|input| input.key_pressed(egui::Key::O) && input.modifiers.command);
    let keyboard_reload =
        ctx.input(|input| input.key_pressed(egui::Key::R) && input.modifiers.command);
    let keyboard_save =
        ctx.input(|input| input.key_pressed(egui::Key::S) && input.modifiers.command);

    let mut request_new = keyboard_new;
    let mut request_open = keyboard_open;
    let mut request_reload = keyboard_reload;
    let mut request_save = keyboard_save;
    let mut request_save_as = false;
    let mut request_validate = false;
    let mut request_play = false;
    let mut request_restart_play = false;
    let mut request_stop_play = false;
    let mut request_rebuild_portals = false;
    let mut request_create_room = false;
    let mut request_delete_sector = false;
    let mut request_insert_vertex = false;
    let mut request_remove_vertex = false;
    let mut request_clear_wall_portal = false;
    let mut selected_sector_for_spawn = false;
    let mut selected_sector_for_initial = false;
    let mut add_hovered_draft_point = false;
    let mut use_hovered_spawn = false;
    let mut request_center_view_on_spawn = false;
    let mut request_frame_map = false;
    let mut request_zoom_in = false;
    let mut request_zoom_out = false;
    let mut request_apply_view = false;
    let mut selected_output_format = editor.output_format;

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                request_new = true;
            }
            if ui.button("Open").clicked() {
                request_open = true;
            }
            if ui.button("Reload").clicked() {
                request_reload = true;
            }
            if ui.button("Save").clicked() {
                request_save = true;
            }
            if ui.button("Save As").clicked() {
                request_save_as = true;
            }
            if ui.button("Validate").clicked() {
                request_validate = true;
            }

            ui.separator();

            if ui.button("Play").clicked() {
                request_play = true;
            }
            if ui.button("Restart Play").clicked() {
                request_restart_play = true;
            }
            if ui.button("Stop Play").clicked() {
                request_stop_play = true;
            }

            ui.separator();

            ui.label("Tool:");
            ui.selectable_value(&mut editor.tool, EditorTool::Select, "Select");
            ui.selectable_value(&mut editor.tool, EditorTool::DrawRoom, "Draw room");
            ui.selectable_value(&mut editor.tool, EditorTool::SetSpawn, "Set spawn");
        });
    });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Path:");
            ui.monospace(editor.map_path.display().to_string());
            ui.separator();
            ui.strong("Status:");
            ui.colored_label(status_color(editor.status.tone), &editor.status.text);
            if let Some(hover) = editor.last_plot_hover {
                ui.separator();
                ui.label(format!("Cursor: {:.2}, {:.2}", hover.x, hover.y));
            }
            if play_session.child.is_some() {
                ui.separator();
                ui.colored_label(egui::Color32::LIGHT_GREEN, "Play window running");
            }
        });
    });

    egui::SidePanel::left("inspector")
        .default_width(340.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.heading("Document");
                    ui.label(format!("Sectors: {}", document.sectors.len()));
                    ui.label(format!("Current file: {}", editor.map_path.display()));
                    egui::ComboBox::from_label("Save format")
                        .selected_text(selected_output_format.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut selected_output_format,
                                MapFileFormat::Ron,
                                MapFileFormat::Ron.label(),
                            );
                            ui.selectable_value(
                                &mut selected_output_format,
                                MapFileFormat::Protobuf,
                                MapFileFormat::Protobuf.label(),
                            );
                        });

                    ui.separator();
                    ui.heading("Viewport");
                    ui.horizontal(|ui| {
                        if ui.button("Center on spawn").clicked() {
                            request_center_view_on_spawn = true;
                        }
                        if ui.button("Frame map").clicked() {
                            request_frame_map = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("-").clicked() {
                            request_zoom_out = true;
                        }
                        if ui.small_button("+").clicked() {
                            request_zoom_in = true;
                        }
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut editor.plot_half_extent,
                                    MIN_PLOT_HALF_EXTENT..=MAX_PLOT_HALF_EXTENT,
                                )
                                .logarithmic(true)
                                .text("View radius"),
                            )
                            .changed()
                        {
                            request_apply_view = true;
                        }
                    });
                    ui.small("Drag the plot to pan. Scroll or use +/- to zoom.");

                    ui.separator();
                    ui.heading("Spawn");
                    let spawn_changed = ui
                        .horizontal(|ui| {
                            ui.label("Spawn:");
                            let mut changed = false;
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut document.initial_position.0)
                                        .speed(0.1)
                                        .prefix("x "),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut document.initial_position.1)
                                        .speed(0.1)
                                        .prefix("y "),
                                )
                                .changed();
                            changed
                        })
                        .inner;
                    let direction_changed = ui
                        .add(
                            egui::DragValue::new(&mut document.initial_direction_degrees)
                                .speed(1.0)
                                .prefix("Direction "),
                        )
                        .changed();
                    if spawn_changed || direction_changed {
                        editor.dirty = true;
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Set spawn to selected sector center").clicked() {
                            selected_sector_for_spawn = true;
                        }
                        if ui.button("Use selected sector as initial").clicked() {
                            selected_sector_for_initial = true;
                        }
                    });

                    ui.separator();
                    egui::CollapsingHeader::new("Sectors")
                        .default_open(true)
                        .show(ui, |ui| {
                            for sector in &document.sectors {
                                let selected = editor.selected_sector == Some(sector.id);
                                let label =
                                    format!("Sector {} ({} walls)", sector.id.0, sector.vertices.len());
                                if ui.selectable_label(selected, label).clicked() {
                                    editor.selected_sector = Some(sector.id);
                                    editor.selected_wall = None;
                                    editor.selected_vertex = None;
                                }
                            }
                        });

                    ui.separator();
                    egui::CollapsingHeader::new("Selected sector")
                        .default_open(true)
                        .show(ui, |ui| {
                            if let Some(sector_index) =
                                editor.selected_sector.and_then(|id| sector_index(&document, id))
                            {
                                let mut sector_changed = false;
                                let mut keep_sky_tint =
                                    document.sectors[sector_index].sky_color.is_some();
                                let mut new_selected_vertex = editor.selected_vertex;
                                let mut new_selected_wall = editor.selected_wall;
                                let wall_segments = document.sectors[sector_index].wall_segments();

                                {
                                    let sector = &mut document.sectors[sector_index];
                                    ui.label(format!("Editing sector {}", sector.id.0));
                                    ui.horizontal(|ui| {
                                        if ui.button("Delete sector").clicked() {
                                            request_delete_sector = true;
                                        }
                                        if ui.button("Center spawn here").clicked() {
                                            selected_sector_for_spawn = true;
                                        }
                                    });

                                    sector_changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut sector.floor.0)
                                                .speed(0.1)
                                                .prefix("Floor "),
                                        )
                                        .changed();
                                    let min_ceil = sector.floor.0 + 0.1;
                                    sector_changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut sector.ceil.0)
                                                .speed(0.1)
                                                .range(min_ceil..=64.0)
                                                .prefix("Ceil "),
                                        )
                                        .changed();
                                    sector.ceil.0 = sector.ceil.0.max(min_ceil);

                                    sector_changed |=
                                        ui.checkbox(&mut sector.no_ceiling, "Open ceiling").changed();
                                    if sector.no_ceiling {
                                        sector_changed |= ui
                                            .checkbox(&mut keep_sky_tint, "Use flat sky tint")
                                            .changed();
                                        if keep_sky_tint && sector.sky_color.is_none() {
                                            sector.sky_color = Some(RawColor([80, 110, 160]));
                                            sector_changed = true;
                                        }
                                        if !keep_sky_tint {
                                            sector.sky_color = None;
                                        }
                                    } else {
                                        sector.sky_color = None;
                                    }

                                    sector_changed |=
                                        color_edit(ui, "Floor color", &mut sector.floor_color.0);
                                    sector_changed |=
                                        color_edit(ui, "Ceiling color", &mut sector.ceil_color.0);
                                    if let Some(sky_color) = sector.sky_color.as_mut() {
                                        sector_changed |=
                                            color_edit(ui, "Sky tint", &mut sky_color.0);
                                    }

                                    ui.separator();
                                    ui.label("Vertices");
                                    for (vertex_index, vertex) in sector.vertices.iter_mut().enumerate() {
                                        let selected = new_selected_vertex == Some(vertex_index);
                                        ui.horizontal(|ui| {
                                            if ui
                                                .selectable_label(selected, format!("#{}", vertex_index))
                                                .clicked()
                                            {
                                                new_selected_vertex = Some(vertex_index);
                                                new_selected_wall = None;
                                            }
                                            sector_changed |= ui
                                                .add(
                                                    egui::DragValue::new(&mut vertex.0.x)
                                                        .speed(0.1)
                                                        .prefix("x "),
                                                )
                                                .changed();
                                            sector_changed |= ui
                                                .add(
                                                    egui::DragValue::new(&mut vertex.0.y)
                                                        .speed(0.1)
                                                        .prefix("y "),
                                                )
                                                .changed();
                                        });
                                    }
                                    ui.horizontal(|ui| {
                                        if ui.button("Insert vertex after selected").clicked() {
                                            request_insert_vertex = true;
                                        }
                                        if ui.button("Remove selected vertex").clicked() {
                                            request_remove_vertex = true;
                                        }
                                    });

                                    ui.separator();
                                    ui.label("Walls");
                                    for (wall_index, wall) in wall_segments.iter().enumerate() {
                                        let label = match sector.portal_sectors[wall_index] {
                                            Some(target) if sector.portal_walkable[wall_index] => {
                                                format!("Wall {} -> sector {}", wall_index, target.0)
                                            }
                                            Some(target) => {
                                                format!("Wall {} -> window {}", wall_index, target.0)
                                            }
                                            None => format!("Wall {} (solid)", wall_index),
                                        };
                                        if ui
                                            .selectable_label(new_selected_wall == Some(wall_index), label)
                                            .clicked()
                                        {
                                            new_selected_wall = Some(wall_index);
                                            new_selected_vertex = None;
                                        }
                                        ui.small(format!(
                                            "  ({:.2}, {:.2}) -> ({:.2}, {:.2})",
                                            wall.left.0.x, wall.left.0.y, wall.right.0.x, wall.right.0.y
                                        ));
                                    }

                                    if let Some(wall_index) = new_selected_wall
                                        .filter(|index| *index < sector.vertices.len())
                                    {
                                        ui.separator();
                                        ui.label(format!("Selected wall {}", wall_index));
                                        sector_changed |= color_edit(
                                            ui,
                                            "Wall color",
                                            &mut sector.colors[wall_index].0,
                                        );

                                        let mut upper_enabled =
                                            sector.portal_upper_colors[wall_index].is_some();
                                        sector_changed |=
                                            ui.checkbox(&mut upper_enabled, "Upper trim").changed();
                                        if upper_enabled
                                            && sector.portal_upper_colors[wall_index].is_none()
                                        {
                                            sector.portal_upper_colors[wall_index] =
                                                Some(sector.colors[wall_index]);
                                            sector_changed = true;
                                        }
                                        if !upper_enabled {
                                            sector.portal_upper_colors[wall_index] = None;
                                        }
                                        if let Some(color) =
                                            sector.portal_upper_colors[wall_index].as_mut()
                                        {
                                            sector_changed |=
                                                color_edit(ui, "Upper color", &mut color.0);
                                        }

                                        let mut lower_enabled =
                                            sector.portal_lower_colors[wall_index].is_some();
                                        sector_changed |=
                                            ui.checkbox(&mut lower_enabled, "Lower trim").changed();
                                        if lower_enabled
                                            && sector.portal_lower_colors[wall_index].is_none()
                                        {
                                            sector.portal_lower_colors[wall_index] =
                                                Some(sector.colors[wall_index]);
                                            sector_changed = true;
                                        }
                                        if !lower_enabled {
                                            sector.portal_lower_colors[wall_index] = None;
                                        }
                                        if let Some(color) =
                                            sector.portal_lower_colors[wall_index].as_mut()
                                        {
                                            sector_changed |=
                                                color_edit(ui, "Lower color", &mut color.0);
                                        }

                                        if sector.portal_sectors[wall_index].is_some() {
                                            sector_changed |= ui
                                                .checkbox(
                                                    &mut sector.portal_walkable[wall_index],
                                                    "Walkable portal",
                                                )
                                                .changed();
                                            if ui.button("Clear portal").clicked() {
                                                request_clear_wall_portal = true;
                                            }
                                        } else {
                                            ui.small(
                                                "No portal on this wall. Use matching geometry and rebuild portals.",
                                            );
                                        }
                                    }
                                }

                                editor.selected_vertex = new_selected_vertex;
                                editor.selected_wall = new_selected_wall;

                                if sector_changed {
                                    editor.dirty = true;
                                }
                            } else {
                                ui.small("Select a sector from the list or map view.");
                            }
                        });

                    ui.separator();
                    egui::CollapsingHeader::new("Room draft")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(match editor.tool {
                                EditorTool::Select => "Select a sector, wall, or vertex from the map.",
                                EditorTool::DrawRoom => {
                                    "Click the map to add polygon points, then create a room. Concave polygons split into convex sectors automatically."
                                }
                                EditorTool::SetSpawn => {
                                    "Click the map to place the player spawn and update the initial sector."
                                }
                            });

                            ui.horizontal(|ui| {
                                if ui.button("Add hovered point").clicked() {
                                    add_hovered_draft_point = true;
                                }
                                if ui.button("Undo point").clicked() {
                                    editor.draft_room.pop();
                                }
                                if ui.button("Clear draft").clicked() {
                                    editor.draft_room.clear();
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui.button("Create room").clicked() {
                                    request_create_room = true;
                                }
                                if ui.button("Set spawn from hover").clicked() {
                                    use_hovered_spawn = true;
                                }
                            });

                            let mut draft_changed = false;
                            draft_changed |= ui
                                .add(
                                    egui::DragValue::new(&mut editor.draft_style.floor)
                                        .speed(0.1)
                                        .prefix("Draft floor "),
                                )
                                .changed();
                            let min_draft_ceil = editor.draft_style.floor + 0.1;
                            draft_changed |= ui
                                .add(
                                    egui::DragValue::new(&mut editor.draft_style.ceil)
                                        .speed(0.1)
                                        .range(min_draft_ceil..=64.0)
                                        .prefix("Draft ceil "),
                                )
                                .changed();
                            editor.draft_style.ceil = editor.draft_style.ceil.max(min_draft_ceil);
                            draft_changed |= color_edit(
                                ui,
                                "Draft floor color",
                                &mut editor.draft_style.floor_color.0,
                            );
                            draft_changed |= color_edit(
                                ui,
                                "Draft ceiling color",
                                &mut editor.draft_style.ceil_color.0,
                            );
                            draft_changed |=
                                color_edit(ui, "Draft wall color", &mut editor.draft_style.wall_color.0);
                            draft_changed |= ui
                                .checkbox(&mut editor.draft_style.no_ceiling, "Draft open ceiling")
                                .changed();
                            let mut draft_sky_enabled = editor.draft_style.sky_color.is_some();
                            if editor.draft_style.no_ceiling {
                                draft_changed |= ui
                                    .checkbox(&mut draft_sky_enabled, "Draft sky tint")
                                    .changed();
                                if draft_sky_enabled && editor.draft_style.sky_color.is_none() {
                                    editor.draft_style.sky_color = Some(RawColor([80, 110, 160]));
                                }
                                if !draft_sky_enabled {
                                    editor.draft_style.sky_color = None;
                                }
                                if let Some(color) = editor.draft_style.sky_color.as_mut() {
                                    draft_changed |= color_edit(ui, "Draft sky color", &mut color.0);
                                }
                            } else {
                                editor.draft_style.sky_color = None;
                            }
                            if draft_changed {
                                editor.dirty = true;
                            }

                            ui.small(format!("Draft points: {}", editor.draft_room.len()));
                            for (index, point) in editor.draft_room.iter().enumerate() {
                                ui.small(format!("#{index}: {:.2}, {:.2}", point.x, point.y));
                            }
                        });

                    ui.separator();
                    egui::CollapsingHeader::new("Automation")
                        .default_open(false)
                        .show(ui, |ui| {
                            if ui.button("Rebuild portals").clicked() {
                                request_rebuild_portals = true;
                            }
                            ui.checkbox(
                                &mut editor.auto_portals_on_save,
                                "Rebuild matching portals on save",
                            );
                            ui.checkbox(&mut editor.restart_play_on_save, "Restart play on save");
                        });
                });
        });

    let mut hovered_plot_point = None;
    let mut hovered_plot_selection = PlotSelection::default();
    let default_plot_bounds = plot_bounds_for_view(editor.plot_center, editor.plot_half_extent);
    let pending_plot_bounds = editor.pending_plot_bounds.take();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let plot_response = Plot::new("map_plot")
                .data_aspect(1.0)
                .allow_zoom(true)
                .allow_drag(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true)
                .auto_bounds(false)
                .default_x_bounds(default_plot_bounds.min()[0], default_plot_bounds.max()[0])
                .default_y_bounds(default_plot_bounds.min()[1], default_plot_bounds.max()[1])
                .show_axes([true, true])
                .show(ui, |plot_ui| {
                    if let Some(bounds) = pending_plot_bounds {
                        plot_ui.set_plot_bounds(bounds);
                    }
                    hovered_plot_point = plot_ui.pointer_coordinate().map(plot_point_to_vec2);
                    if let Some(pointer) = hovered_plot_point {
                        hovered_plot_selection = pick_plot_selection(&document, pointer);
                    }

                    for sector in &document.sectors {
                        let polygon = Polygon::new(
                            format!("sector {}", sector.id.0),
                            PlotPoints::new(
                                sector
                                    .vertices
                                    .iter()
                                    .map(|vertex| [vertex.0.x as f64, vertex.0.y as f64])
                                    .collect::<Vec<_>>(),
                            ),
                        )
                        .highlight(
                            editor.selected_sector == Some(sector.id)
                                || hovered_plot_selection.sector == Some(sector.id),
                        );
                        plot_ui.polygon(polygon);
                    }

                    if let Some(selected_sector_id) = editor.selected_sector {
                        if let Some(sector_index) = sector_index(&document, selected_sector_id) {
                            if let Some(wall_index) = editor.selected_wall.filter(|index| {
                                *index < document.sectors[sector_index].vertices.len()
                            }) {
                                let wall =
                                    document.sectors[sector_index].wall_segments()[wall_index];
                                plot_ui.line(
                                    PlotLine::new(
                                        "selected wall",
                                        PlotPoints::new(vec![
                                            [wall.left.0.x as f64, wall.left.0.y as f64],
                                            [wall.right.0.x as f64, wall.right.0.y as f64],
                                        ]),
                                    )
                                    .color(egui::Color32::YELLOW)
                                    .width(3.0),
                                );
                            }

                            if let Some(vertex_index) = editor.selected_vertex.filter(|index| {
                                *index < document.sectors[sector_index].vertices.len()
                            }) {
                                let vertex =
                                    document.sectors[sector_index].vertices[vertex_index].0;
                                plot_ui.points(
                                    Points::new(
                                        "selected vertex",
                                        vec![[vertex.x as f64, vertex.y as f64]],
                                    )
                                    .color(egui::Color32::YELLOW)
                                    .filled(true)
                                    .radius(7.0)
                                    .shape(MarkerShape::Diamond),
                                );
                            }
                        }
                    }

                    let spawn = vec2(document.initial_position.0, document.initial_position.1);
                    let spawn_forward =
                        Vec2::from_angle((document.initial_direction_degrees + 90.0).to_radians())
                            * SPAWN_ARROW_LENGTH;
                    plot_ui.points(
                        Points::new("spawn", vec![[spawn.x as f64, spawn.y as f64]])
                            .color(egui::Color32::from_rgb(255, 215, 0))
                            .filled(true)
                            .radius(6.0)
                            .shape(MarkerShape::Circle),
                    );
                    plot_ui.line(
                        PlotLine::new(
                            "spawn direction",
                            PlotPoints::new(vec![
                                [spawn.x as f64, spawn.y as f64],
                                [
                                    (spawn.x + spawn_forward.x) as f64,
                                    (spawn.y + spawn_forward.y) as f64,
                                ],
                            ]),
                        )
                        .color(egui::Color32::from_rgb(255, 215, 0))
                        .width(2.0),
                    );

                    if editor.draft_room.len() >= 2 {
                        let mut draft_points = editor
                            .draft_room
                            .iter()
                            .map(|point| [point.x as f64, point.y as f64])
                            .collect::<Vec<_>>();
                        if editor.draft_room.len() >= 3 {
                            draft_points.push([
                                editor.draft_room[0].x as f64,
                                editor.draft_room[0].y as f64,
                            ]);
                        }
                        plot_ui.line(
                            PlotLine::new("draft room", PlotPoints::new(draft_points))
                                .color(egui::Color32::from_rgb(80, 180, 255))
                                .width(2.0),
                        );
                    }
                    if !editor.draft_room.is_empty() {
                        plot_ui.points(
                            Points::new(
                                "draft vertices",
                                editor
                                    .draft_room
                                    .iter()
                                    .map(|point| [point.x as f64, point.y as f64])
                                    .collect::<Vec<_>>(),
                            )
                            .color(egui::Color32::from_rgb(80, 180, 255))
                            .filled(true)
                            .radius(5.0)
                            .shape(MarkerShape::Circle),
                        );
                    }
                });

            editor.last_plot_hover = hovered_plot_point;
            editor.sync_view_from_bounds(plot_response.transform.bounds());

            if plot_response.response.clicked() {
                if let Some(pointer) = hovered_plot_point {
                    match editor.tool {
                        EditorTool::Select => {
                            editor.selected_sector = hovered_plot_selection.sector;
                            editor.selected_wall = hovered_plot_selection.wall;
                            editor.selected_vertex = hovered_plot_selection.vertex;
                        }
                        EditorTool::DrawRoom => {
                            editor.draft_room.push(pointer);
                            editor.status = StatusMessage::info(format!(
                                "Added draft point {:.2}, {:.2}",
                                pointer.x, pointer.y
                            ));
                        }
                        EditorTool::SetSpawn => {
                            document.initial_position = MapVertex(pointer.x, pointer.y);
                            if let Some(sector_id) = hovered_plot_selection.sector {
                                document.initial_sector = sector_id;
                            }
                            editor.dirty = true;
                            editor.status = StatusMessage::success(format!(
                                "Moved spawn to {:.2}, {:.2}",
                                pointer.x, pointer.y
                            ));
                        }
                    }
                }
            }
        });

    if add_hovered_draft_point {
        if let Some(point) = editor.last_plot_hover {
            editor.draft_room.push(point);
            editor.status =
                StatusMessage::info(format!("Added draft point {:.2}, {:.2}", point.x, point.y));
        } else {
            editor.status = StatusMessage::error("Hover the plot before adding a point");
        }
    }

    if use_hovered_spawn {
        if let Some(point) = editor.last_plot_hover {
            document.initial_position = MapVertex(point.x, point.y);
            if let Some(sector_id) = hovered_plot_selection.sector {
                document.initial_sector = sector_id;
            }
            editor.dirty = true;
            editor.status =
                StatusMessage::success(format!("Moved spawn to {:.2}, {:.2}", point.x, point.y));
        } else {
            editor.status = StatusMessage::error("Hover the plot before setting the spawn");
        }
    }

    if selected_output_format != editor.output_format {
        editor.output_format = selected_output_format;
        editor.dirty = true;
        editor.status = StatusMessage::info(format!(
            "Next save will use {} format",
            editor.output_format.label()
        ));
    }

    if request_center_view_on_spawn {
        editor.center_view_on_spawn(&document);
    }
    if request_frame_map {
        editor.focus_view_on_spawn(&document);
    }
    if request_zoom_in {
        let plot_center = editor.plot_center;
        let plot_half_extent = editor.plot_half_extent;
        editor.focus_view(plot_center, plot_half_extent * 0.8);
    }
    if request_zoom_out {
        let plot_center = editor.plot_center;
        let plot_half_extent = editor.plot_half_extent;
        editor.focus_view(plot_center, plot_half_extent * 1.25);
    }
    if request_apply_view {
        let plot_center = editor.plot_center;
        let plot_half_extent = editor.plot_half_extent;
        editor.focus_view(plot_center, plot_half_extent);
    }

    if selected_sector_for_initial {
        if let Some(sector_id) = editor.selected_sector {
            document.initial_sector = sector_id;
            ensure_valid_spawn(&mut document);
            editor.dirty = true;
            editor.status =
                StatusMessage::success(format!("Set initial sector to {}", sector_id.0));
        }
    }

    if selected_sector_for_spawn {
        if let Some(sector_id) = editor.selected_sector {
            set_spawn_to_sector_center(&mut document, sector_id);
            editor.dirty = true;
            editor.status = StatusMessage::success(format!(
                "Moved spawn to the center of sector {}",
                sector_id.0
            ));
        }
    }

    if request_insert_vertex {
        match insert_vertex_after_selection(&mut document, &mut editor) {
            Ok(message) => {
                editor.dirty = true;
                editor.status = StatusMessage::success(message);
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    }

    if request_remove_vertex {
        match remove_selected_vertex(&mut document, &mut editor) {
            Ok(message) => {
                editor.dirty = true;
                editor.status = StatusMessage::success(message);
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    }

    if request_clear_wall_portal {
        match clear_selected_wall_portal(&mut document, &editor) {
            Ok(message) => {
                editor.dirty = true;
                editor.status = StatusMessage::success(message);
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    } else if let Some((sector_id, wall_index, walkable)) =
        selected_wall_walkability(&document, &editor)
    {
        sync_reciprocal_walkability(&mut document, sector_id, wall_index, walkable);
    }

    if request_delete_sector {
        match delete_selected_sector(&mut document, &mut editor) {
            Ok(message) => {
                editor.dirty = true;
                editor.status = StatusMessage::success(message);
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    }

    if request_rebuild_portals {
        let linked = rebuild_portals(&mut document);
        editor.dirty = true;
        editor.status = StatusMessage::success(format!("Linked {linked} matching wall pairs"));
    }

    if request_create_room {
        match create_room_from_draft(&mut document, &mut editor) {
            Ok(message) => {
                editor.dirty = true;
                editor.status = StatusMessage::success(message);
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    }

    if request_validate {
        editor.status = match validate_document(&document) {
            Ok(()) => StatusMessage::success("Map validates cleanly"),
            Err(error) => StatusMessage::error(error),
        };
    }

    if request_new {
        let new_document = new_editor_document();
        let new_path = default_new_map_path(editor.output_format);
        let output_format = editor.output_format;
        *document = new_document;
        editor.load_document(
            &document,
            new_path.clone(),
            StatusMessage::success(format!("Created a new {}", output_format.label())),
        );
        editor.dirty = true;
    }

    if request_open {
        if let Some(path) = FileDialog::new()
            .add_filter("Sector maps", &["ron", "pb"])
            .set_directory("assets/maps")
            .pick_file()
        {
            match load_editor_document(&path) {
                Ok(new_document) => {
                    *document = new_document;
                    editor.load_document(
                        &document,
                        path.clone(),
                        StatusMessage::success(format!("Loaded {}", path.display())),
                    );
                }
                Err(error) => editor.status = StatusMessage::error(error),
            }
        }
    }

    if request_reload {
        match load_editor_document(&editor.map_path) {
            Ok(new_document) => {
                let reload_path = editor.map_path.clone();
                *document = new_document;
                editor.load_document(
                    &document,
                    reload_path.clone(),
                    StatusMessage::success(format!("Reloaded {}", reload_path.display())),
                );
            }
            Err(error) => editor.status = StatusMessage::error(error),
        }
    }

    if request_save_as {
        let current_name = map_output_path(&editor.map_path, editor.output_format)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled.map.ron")
            .to_owned();
        if let Some(path) = FileDialog::new()
            .add_filter("RON maps", &["ron"])
            .add_filter("Protobuf maps", &["pb"])
            .set_directory("assets/maps")
            .set_file_name(&current_name)
            .save_file()
        {
            let output_format = editor.output_format;
            save_document(
                &mut document,
                &mut editor,
                &mut play_session,
                Some(normalize_map_output_path(path, output_format)),
            );
        }
    } else if request_save {
        save_document(&mut document, &mut editor, &mut play_session, None);
    }

    if request_stop_play {
        stop_play_process(&mut play_session, &mut editor);
    }

    if request_restart_play {
        if save_document(&mut document, &mut editor, &mut play_session, None) {
            let play_path = editor.map_path.clone();
            launch_play_process(&play_path, &mut play_session, &mut editor);
        }
    } else if request_play {
        if save_document(&mut document, &mut editor, &mut play_session, None) {
            let play_path = editor.map_path.clone();
            launch_play_process(&play_path, &mut play_session, &mut editor);
        }
    }
}

fn load_editor_document(path: &Path) -> Result<EditorDocument, String> {
    let map = load_map_from_path(path)
        .map_err(|error| format!("failed to load {}: {error}", path.display()))?;
    let (initial_sector, sectors) = map_to_sectors(&map)
        .map_err(|error| format!("failed to convert {}: {error}", path.display()))?;
    Ok(EditorDocument {
        sectors,
        initial_sector: initial_sector.0,
        initial_position: map.initial_position,
        initial_direction_degrees: map.initial_direction_degrees,
    })
}

fn new_editor_document() -> EditorDocument {
    EditorDocument {
        sectors: vec![sector_from_points(
            0,
            &[
                vec2(-4.0, 4.0),
                vec2(4.0, 4.0),
                vec2(4.0, -4.0),
                vec2(-4.0, -4.0),
            ],
            DraftSectorStyle::default(),
        )],
        initial_sector: SectorId(0),
        initial_position: MapVertex(0.0, 0.0),
        initial_direction_degrees: 0.0,
    }
}

fn spawn_position(document: &EditorDocument) -> Vec2 {
    vec2(document.initial_position.0, document.initial_position.1)
}

fn default_plot_half_extent(document: &EditorDocument) -> f32 {
    let spawn = spawn_position(document);
    let mut half_extent = 8.0_f32;
    for sector in &document.sectors {
        for vertex in &sector.vertices {
            half_extent = half_extent
                .max((vertex.0.x - spawn.x).abs())
                .max((vertex.0.y - spawn.y).abs());
        }
    }
    (half_extent + 2.0).clamp(MIN_PLOT_HALF_EXTENT, MAX_PLOT_HALF_EXTENT)
}

fn plot_bounds_for_view(center: Vec2, half_extent: f32) -> PlotBounds {
    let half_extent = half_extent.clamp(MIN_PLOT_HALF_EXTENT, MAX_PLOT_HALF_EXTENT) as f64;
    PlotBounds::from_min_max(
        [center.x as f64 - half_extent, center.y as f64 - half_extent],
        [center.x as f64 + half_extent, center.y as f64 + half_extent],
    )
}

fn save_document(
    document: &mut EditorDocument,
    editor: &mut EditorState,
    play_session: &mut PlaySession,
    path_override: Option<PathBuf>,
) -> bool {
    let path =
        path_override.unwrap_or_else(|| map_output_path(&editor.map_path, editor.output_format));
    let linked = if editor.auto_portals_on_save {
        Some(rebuild_portals(document))
    } else {
        None
    };
    ensure_valid_spawn(document);

    let map = match sectors_to_map_with_spawn(
        document.initial_sector,
        document.initial_position,
        document.initial_direction_degrees,
        &document.sectors,
    ) {
        Ok(map) => map,
        Err(error) => {
            editor.status = StatusMessage::error(format!("validation failed: {error}"));
            return false;
        }
    };

    if let Err(error) = save_map_to_path(&map, &path) {
        editor.status = StatusMessage::error(format!("failed to save {}: {error}", path.display()));
        return false;
    }

    editor.map_path = path;
    editor.output_format = map_format_for_path(&editor.map_path);
    editor.dirty = false;
    editor.status = StatusMessage::success(match linked {
        Some(count) => format!(
            "Saved {} after rebuilding {count} matching portal pairs",
            editor.map_path.display()
        ),
        None => format!("Saved {}", editor.map_path.display()),
    });

    if editor.restart_play_on_save && play_session.child.is_some() {
        let play_path = editor.map_path.clone();
        launch_play_process(&play_path, play_session, editor);
    }

    true
}

fn validate_document(document: &EditorDocument) -> Result<(), String> {
    sectors_to_map_with_spawn(
        document.initial_sector,
        document.initial_position,
        document.initial_direction_degrees,
        &document.sectors,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn launch_play_process(path: &Path, play_session: &mut PlaySession, editor: &mut EditorState) {
    stop_play_process(play_session, editor);

    match Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("sector")
        .arg("--features")
        .arg("sector")
        .arg("--")
        .arg(path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .spawn()
    {
        Ok(child) => {
            play_session.child = Some(child);
            editor.status =
                StatusMessage::success(format!("Launched play window for {}", path.display()));
        }
        Err(error) => {
            editor.status = StatusMessage::error(format!("failed to launch play window: {error}"));
        }
    }
}

fn stop_play_process(play_session: &mut PlaySession, editor: &mut EditorState) {
    let Some(mut child) = play_session.child.take() else {
        return;
    };

    let _ = child.kill();
    let _ = child.wait();
    editor.status = StatusMessage::info("Stopped play window");
}

fn poll_play_session(play_session: &mut PlaySession, editor: &mut EditorState) {
    let Some(child) = play_session.child.as_mut() else {
        return;
    };

    match child.try_wait() {
        Ok(Some(status)) => {
            play_session.child = None;
            editor.status = StatusMessage::info(format!("Play window exited with {status}"));
        }
        Ok(None) => {}
        Err(error) => {
            play_session.child = None;
            editor.status = StatusMessage::error(format!("failed to poll play window: {error}"));
        }
    }
}

fn create_room_from_draft(
    document: &mut EditorDocument,
    editor: &mut EditorState,
) -> Result<String, String> {
    let next_id = document
        .sectors
        .iter()
        .map(|sector| sector.id.0)
        .max()
        .map(|id| id + 1)
        .unwrap_or(0);
    let new_sectors =
        build_sectors_from_room_draft(&editor.draft_room, editor.draft_style, next_id)?;
    let created_count = new_sectors.len();
    document.sectors.extend(new_sectors);
    if editor.auto_portals_on_save {
        rebuild_portals(document);
    }
    editor.selected_sector = Some(SectorId(next_id));
    editor.selected_wall = None;
    editor.selected_vertex = None;
    editor.draft_room.clear();
    ensure_valid_spawn(document);

    Ok(format!(
        "Created {created_count} sector(s) from the draft room"
    ))
}

fn build_sectors_from_room_draft(
    draft_room: &[Vec2],
    style: DraftSectorStyle,
    next_id: u32,
) -> Result<Vec<Sector>, String> {
    let vertices = normalize_draft_vertices(draft_room);
    if vertices.len() < 3 {
        return Err("room drafts need at least three distinct points".into());
    }
    if polygon_signed_area(&vertices).abs() <= GEOMETRY_EPSILON {
        return Err("room draft area is too small".into());
    }

    let flat_vertices = vertices
        .iter()
        .flat_map(|vertex| [vertex.x as f64, vertex.y as f64])
        .collect::<Vec<_>>();
    let indices = earcutr::earcut(&flat_vertices, &[], 2)
        .map_err(|error| format!("failed to split room draft: {error}"))?;
    if indices.len() < 3 {
        return Err("room draft did not produce any sectors".into());
    }

    let mut sectors = Vec::new();
    for triangle in indices.chunks_exact(3) {
        let mut points = vec![
            vertices[triangle[0]],
            vertices[triangle[1]],
            vertices[triangle[2]],
        ];
        if polygon_signed_area(&points) > 0.0 {
            points.reverse();
        }
        sectors.push(sector_from_points(
            next_id + sectors.len() as u32,
            &points,
            style,
        ));
    }

    Ok(sectors)
}

fn sector_from_points(id: u32, points: &[Vec2], style: DraftSectorStyle) -> Sector {
    Sector {
        id: SectorId(id),
        vertices: points.iter().copied().map(Position2).collect(),
        portal_sectors: vec![None; points.len()],
        portal_walkable: vec![true; points.len()],
        colors: vec![style.wall_color; points.len()],
        portal_upper_colors: vec![None; points.len()],
        portal_lower_colors: vec![None; points.len()],
        floor: Length(style.floor),
        ceil: Length(style.ceil.max(style.floor + 0.1)),
        floor_color: style.floor_color,
        ceil_color: style.ceil_color,
        no_ceiling: style.no_ceiling,
        sky_color: if style.no_ceiling {
            style.sky_color
        } else {
            None
        },
    }
}

fn rebuild_portals(document: &mut EditorDocument) -> usize {
    for sector in &mut document.sectors {
        for portal in &mut sector.portal_sectors {
            *portal = None;
        }
    }

    let mut matches = Vec::new();
    for left_sector_index in 0..document.sectors.len() {
        for right_sector_index in (left_sector_index + 1)..document.sectors.len() {
            let left_sector = &document.sectors[left_sector_index];
            let right_sector = &document.sectors[right_sector_index];
            let left_walls = left_sector.wall_segments();
            let right_walls = right_sector.wall_segments();

            for (left_wall_index, left_wall) in left_walls.iter().enumerate() {
                for (right_wall_index, right_wall) in right_walls.iter().enumerate() {
                    if points_match(left_wall.left.0, right_wall.right.0)
                        && points_match(left_wall.right.0, right_wall.left.0)
                    {
                        matches.push((
                            left_sector_index,
                            left_wall_index,
                            right_sector_index,
                            right_wall_index,
                        ));
                    }
                }
            }
        }
    }

    for (left_sector_index, left_wall_index, right_sector_index, right_wall_index) in &matches {
        let left_id = document.sectors[*left_sector_index].id;
        let right_id = document.sectors[*right_sector_index].id;
        let walkable = document.sectors[*left_sector_index].portal_walkable[*left_wall_index]
            && document.sectors[*right_sector_index].portal_walkable[*right_wall_index];
        document.sectors[*left_sector_index].portal_sectors[*left_wall_index] = Some(right_id);
        document.sectors[*right_sector_index].portal_sectors[*right_wall_index] = Some(left_id);
        document.sectors[*left_sector_index].portal_walkable[*left_wall_index] = walkable;
        document.sectors[*right_sector_index].portal_walkable[*right_wall_index] = walkable;
    }

    matches.len()
}

fn delete_selected_sector(
    document: &mut EditorDocument,
    editor: &mut EditorState,
) -> Result<String, String> {
    let Some(selected_sector_id) = editor.selected_sector else {
        return Err("Select a sector to delete".into());
    };
    if document.sectors.len() <= 1 {
        return Err("The editor keeps at least one sector in the document".into());
    }

    let Some(remove_index) = sector_index(document, selected_sector_id) else {
        return Err("Selected sector no longer exists".into());
    };
    document.sectors.remove(remove_index);
    renumber_sectors(document);
    ensure_valid_spawn(document);
    editor.selected_sector = Some(document.initial_sector);
    editor.selected_wall = None;
    editor.selected_vertex = None;

    Ok(format!("Deleted sector {}", selected_sector_id.0))
}

fn insert_vertex_after_selection(
    document: &mut EditorDocument,
    editor: &mut EditorState,
) -> Result<String, String> {
    let Some(selected_sector_id) = editor.selected_sector else {
        return Err("Select a sector first".into());
    };
    let Some(sector_index) = sector_index(document, selected_sector_id) else {
        return Err("Selected sector no longer exists".into());
    };
    let insert_after = editor
        .selected_vertex
        .or(editor.selected_wall)
        .ok_or_else(|| "Select a vertex or wall first".to_owned())?;

    let sector = &mut document.sectors[sector_index];
    if insert_after >= sector.vertices.len() {
        return Err("Selected vertex is out of range".into());
    }
    let next_index = (insert_after + 1) % sector.vertices.len();
    let previous = sector.vertices[insert_after].0;
    let next = sector.vertices[next_index].0;
    let midpoint = previous + (next - previous) * 0.5;

    sector.vertices.insert(next_index, Position2(midpoint));
    sector.portal_sectors.insert(next_index, None);
    sector.portal_walkable.insert(next_index, true);
    sector
        .colors
        .insert(next_index, sector.colors[insert_after]);
    sector.portal_upper_colors.insert(next_index, None);
    sector.portal_lower_colors.insert(next_index, None);

    editor.selected_vertex = Some(next_index);
    editor.selected_wall = None;

    Ok(format!(
        "Inserted a vertex into sector {}",
        selected_sector_id.0
    ))
}

fn remove_selected_vertex(
    document: &mut EditorDocument,
    editor: &mut EditorState,
) -> Result<String, String> {
    let Some(selected_sector_id) = editor.selected_sector else {
        return Err("Select a sector first".into());
    };
    let Some(vertex_index) = editor.selected_vertex else {
        return Err("Select a vertex to remove".into());
    };
    let Some(sector_index) = sector_index(document, selected_sector_id) else {
        return Err("Selected sector no longer exists".into());
    };
    let sector = &mut document.sectors[sector_index];
    if sector.vertices.len() <= 3 {
        return Err("Sectors need at least three vertices".into());
    }
    if vertex_index >= sector.vertices.len() {
        return Err("Selected vertex is out of range".into());
    }

    sector.vertices.remove(vertex_index);
    sector.portal_sectors.remove(vertex_index);
    sector.portal_walkable.remove(vertex_index);
    sector.colors.remove(vertex_index);
    sector.portal_upper_colors.remove(vertex_index);
    sector.portal_lower_colors.remove(vertex_index);
    editor.selected_vertex = None;

    Ok(format!(
        "Removed a vertex from sector {}",
        selected_sector_id.0
    ))
}

fn clear_selected_wall_portal(
    document: &mut EditorDocument,
    editor: &EditorState,
) -> Result<String, String> {
    let Some(selected_sector_id) = editor.selected_sector else {
        return Err("Select a wall first".into());
    };
    let Some(wall_index) = editor.selected_wall else {
        return Err("Select a wall first".into());
    };
    let Some(selected_sector_index) = sector_index(document, selected_sector_id) else {
        return Err("Selected sector no longer exists".into());
    };
    let Some(target_sector_id) = document.sectors[selected_sector_index].portal_sectors[wall_index]
    else {
        return Err("Selected wall does not have a portal".into());
    };

    if let Some(target_sector_index) = sector_index(document, target_sector_id) {
        if let Some(target_wall_index) =
            reverse_matching_wall_index(document, selected_sector_id, wall_index, target_sector_id)
        {
            document.sectors[target_sector_index].portal_sectors[target_wall_index] = None;
        }
    }
    document.sectors[selected_sector_index].portal_sectors[wall_index] = None;

    Ok(format!(
        "Cleared the portal on wall {} of sector {}",
        wall_index, selected_sector_id.0
    ))
}

fn selected_wall_walkability(
    document: &EditorDocument,
    editor: &EditorState,
) -> Option<(SectorId, usize, bool)> {
    let sector_id = editor.selected_sector?;
    let wall_index = editor.selected_wall?;
    let sector_index = sector_index(document, sector_id)?;
    let sector = &document.sectors[sector_index];
    sector
        .portal_sectors
        .get(wall_index)
        .copied()
        .flatten()
        .map(|_| (sector_id, wall_index, sector.portal_walkable[wall_index]))
}

fn sync_reciprocal_walkability(
    document: &mut EditorDocument,
    sector_id: SectorId,
    wall_index: usize,
    walkable: bool,
) {
    let Some(target_sector_id) = sector_index(document, sector_id)
        .and_then(|sector_index| document.sectors[sector_index].portal_sectors[wall_index])
    else {
        return;
    };
    let Some(target_sector_index) = sector_index(document, target_sector_id) else {
        return;
    };
    if let Some(target_wall_index) =
        reverse_matching_wall_index(document, sector_id, wall_index, target_sector_id)
    {
        document.sectors[target_sector_index].portal_walkable[target_wall_index] = walkable;
    }
}

fn reverse_matching_wall_index(
    document: &EditorDocument,
    sector_id: SectorId,
    wall_index: usize,
    target_sector_id: SectorId,
) -> Option<usize> {
    let source_sector_index = sector_index(document, sector_id)?;
    let target_sector_index = sector_index(document, target_sector_id)?;
    let source_wall = document.sectors[source_sector_index].wall_segments()[wall_index];
    document.sectors[target_sector_index]
        .wall_segments()
        .iter()
        .position(|target_wall| {
            points_match(target_wall.left.0, source_wall.right.0)
                && points_match(target_wall.right.0, source_wall.left.0)
        })
}

fn renumber_sectors(document: &mut EditorDocument) {
    document.sectors.sort_by_key(|sector| sector.id.0);
    let id_map = document
        .sectors
        .iter()
        .enumerate()
        .map(|(new_id, sector)| (sector.id.0, new_id as u32))
        .collect::<std::collections::HashMap<_, _>>();

    for (new_id, sector) in document.sectors.iter_mut().enumerate() {
        sector.id = SectorId(new_id as u32);
    }

    for sector in &mut document.sectors {
        for portal in &mut sector.portal_sectors {
            *portal = portal
                .and_then(|target| id_map.get(&target.0).copied())
                .map(SectorId);
        }
    }

    document.initial_sector = id_map
        .get(&document.initial_sector.0)
        .copied()
        .map(SectorId)
        .unwrap_or_else(|| document.sectors[0].id);
}

fn ensure_valid_spawn(document: &mut EditorDocument) {
    let Some(initial_sector_index) = sector_index(document, document.initial_sector) else {
        document.initial_sector = document.sectors[0].id;
        let center = sector_center(&document.sectors[0]);
        document.initial_position = MapVertex(center.x, center.y);
        return;
    };

    let point = vec2(document.initial_position.0, document.initial_position.1);
    if !sector_contains_2d(&document.sectors[initial_sector_index], point) {
        let center = sector_center(&document.sectors[initial_sector_index]);
        document.initial_position = MapVertex(center.x, center.y);
    }
}

fn set_spawn_to_sector_center(document: &mut EditorDocument, sector_id: SectorId) {
    if let Some(sector_index) = sector_index(document, sector_id) {
        let center = sector_center(&document.sectors[sector_index]);
        document.initial_sector = sector_id;
        document.initial_position = MapVertex(center.x, center.y);
    }
}

fn pick_plot_selection(document: &EditorDocument, point: Vec2) -> PlotSelection {
    let mut containing_sector = None;
    let mut nearest_vertex = (f32::INFINITY, PlotSelection::default());
    let mut nearest_wall = (f32::INFINITY, PlotSelection::default());

    for sector in &document.sectors {
        if sector_contains_2d(sector, point) && containing_sector.is_none() {
            containing_sector = Some(sector.id);
        }

        for (vertex_index, vertex) in sector.vertices.iter().enumerate() {
            let distance = point.distance(vertex.0);
            if distance < nearest_vertex.0 {
                nearest_vertex = (
                    distance,
                    PlotSelection {
                        sector: Some(sector.id),
                        wall: None,
                        vertex: Some(vertex_index),
                    },
                );
            }
        }

        for (wall_index, wall) in sector.wall_segments().iter().enumerate() {
            let distance = point_to_segment_distance(point, wall.left.0, wall.right.0);
            if distance < nearest_wall.0 {
                nearest_wall = (
                    distance,
                    PlotSelection {
                        sector: Some(sector.id),
                        wall: Some(wall_index),
                        vertex: None,
                    },
                );
            }
        }
    }

    if nearest_vertex.0 <= VERTEX_PICK_RADIUS {
        nearest_vertex.1
    } else if nearest_wall.0 <= WALL_PICK_RADIUS {
        nearest_wall.1
    } else {
        PlotSelection {
            sector: containing_sector,
            wall: None,
            vertex: None,
        }
    }
}

fn sanitize_selection(document: &EditorDocument, editor: &mut EditorState) {
    let Some(selected_sector_id) = editor.selected_sector else {
        return;
    };
    let Some(sector_index) = sector_index(document, selected_sector_id) else {
        editor.selected_sector = Some(document.initial_sector);
        editor.selected_wall = None;
        editor.selected_vertex = None;
        return;
    };
    let sector = &document.sectors[sector_index];
    if editor
        .selected_wall
        .is_some_and(|index| index >= sector.vertices.len())
    {
        editor.selected_wall = None;
    }
    if editor
        .selected_vertex
        .is_some_and(|index| index >= sector.vertices.len())
    {
        editor.selected_vertex = None;
    }
}

fn sector_index(document: &EditorDocument, sector_id: SectorId) -> Option<usize> {
    document
        .sectors
        .iter()
        .position(|sector| sector.id == sector_id)
}

fn normalize_draft_vertices(draft_room: &[Vec2]) -> Vec<Vec2> {
    let mut vertices = Vec::with_capacity(draft_room.len());
    for point in draft_room.iter().copied() {
        if vertices
            .last()
            .is_some_and(|last: &Vec2| last.distance_squared(point) <= GEOMETRY_EPSILON)
        {
            continue;
        }
        vertices.push(point);
    }
    if vertices.len() > 1
        && vertices[0].distance_squared(*vertices.last().unwrap()) <= GEOMETRY_EPSILON
    {
        vertices.pop();
    }
    vertices
}

fn sector_center(sector: &Sector) -> Vec2 {
    let sum = sector
        .vertices
        .iter()
        .fold(Vec2::ZERO, |sum, vertex| sum + vertex.0);
    sum / sector.vertices.len() as f32
}

fn sector_contains_2d(sector: &Sector, point: Vec2) -> bool {
    let vertices = sector
        .vertices
        .iter()
        .map(|vertex| vertex.0)
        .collect::<Vec<_>>();
    point_in_polygon(point, &vertices)
}

fn point_in_polygon(point: Vec2, vertices: &[Vec2]) -> bool {
    let mut inside = false;
    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        if point_on_segment(point, current, next) {
            return true;
        }

        let crosses = (current.y > point.y) != (next.y > point.y);
        if !crosses {
            continue;
        }

        let edge_fraction = (point.y - current.y) / (next.y - current.y);
        let x_intersection = current.x + (next.x - current.x) * edge_fraction;
        if point.x < x_intersection {
            inside = !inside;
        }
    }
    inside
}

fn point_on_segment(point: Vec2, start: Vec2, end: Vec2) -> bool {
    let segment = end - start;
    let to_point = point - start;
    segment.perp_dot(to_point).abs() <= GEOMETRY_EPSILON
        && to_point.dot(segment) >= -GEOMETRY_EPSILON
        && to_point.dot(segment) <= segment.length_squared() + GEOMETRY_EPSILON
}

fn point_to_segment_distance(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let projection = if segment.length_squared() <= GEOMETRY_EPSILON {
        0.0
    } else {
        ((point - start).dot(segment) / segment.length_squared()).clamp(0.0, 1.0)
    };
    (start + segment * projection).distance(point)
}

fn points_match(left: Vec2, right: Vec2) -> bool {
    left.distance_squared(right) <= GEOMETRY_EPSILON * GEOMETRY_EPSILON
}

fn polygon_signed_area(vertices: &[Vec2]) -> f32 {
    let mut area = 0.0;
    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        area += current.x * next.y - next.x * current.y;
    }
    area * 0.5
}

fn plot_point_to_vec2(point: PlotPoint) -> Vec2 {
    vec2(point.x as f32, point.y as f32)
}

fn color_edit(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.color_edit_button_srgb(color).changed()
    })
    .inner
}

fn status_color(tone: StatusTone) -> egui::Color32 {
    match tone {
        StatusTone::Info => egui::Color32::LIGHT_BLUE,
        StatusTone::Success => egui::Color32::LIGHT_GREEN,
        StatusTone::Error => egui::Color32::LIGHT_RED,
    }
}

fn map_format_for_path(path: &Path) -> MapFileFormat {
    if path.extension().is_some_and(|extension| extension == "pb") {
        MapFileFormat::Protobuf
    } else {
        MapFileFormat::Ron
    }
}

fn map_output_path(path: &Path, format: MapFileFormat) -> PathBuf {
    let path_string = path.display().to_string();
    for suffix in [".map.ron", ".map.pb"] {
        if let Some(stem) = path_string.strip_suffix(suffix) {
            return PathBuf::from(format!("{}{}", stem, format.suffix()));
        }
    }

    if path.extension().is_some() {
        let extension = match format {
            MapFileFormat::Ron => "ron",
            MapFileFormat::Protobuf => "pb",
        };
        return path.with_extension(extension);
    }

    PathBuf::from(format!("{}{}", path.display(), format.suffix()))
}

fn default_new_map_path(format: MapFileFormat) -> PathBuf {
    PathBuf::from(format!("assets/maps/untitled{}", format.suffix()))
}

fn normalize_map_output_path(path: PathBuf, format: MapFileFormat) -> PathBuf {
    if path.extension().is_some() {
        return path;
    }

    PathBuf::from(format!("{}{}", path.display(), format.suffix()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sector_with_vertices(id: u32, vertices: &[(f32, f32)]) -> Sector {
        Sector {
            id: SectorId(id),
            vertices: vertices
                .iter()
                .map(|(x, y)| Position2(vec2(*x, *y)))
                .collect(),
            portal_sectors: vec![None; vertices.len()],
            portal_walkable: vec![true; vertices.len()],
            colors: vec![*MISSING_WALL_COLOR; vertices.len()],
            portal_upper_colors: vec![None; vertices.len()],
            portal_lower_colors: vec![None; vertices.len()],
            floor: Length(0.0),
            ceil: Length(3.2),
            floor_color: *FLOOR_COLOR,
            ceil_color: *CEILING_COLOR,
            no_ceiling: false,
            sky_color: None,
        }
    }

    #[test]
    fn rebuild_portals_links_reversed_matching_walls() {
        let mut document = EditorDocument {
            sectors: vec![
                sector_with_vertices(0, &[(-2.0, 2.0), (0.0, 2.0), (0.0, -2.0), (-2.0, -2.0)]),
                sector_with_vertices(1, &[(0.0, 2.0), (2.0, 2.0), (2.0, -2.0), (0.0, -2.0)]),
            ],
            initial_sector: SectorId(0),
            initial_position: MapVertex(-1.0, 0.0),
            initial_direction_degrees: 0.0,
        };

        let linked = rebuild_portals(&mut document);

        assert_eq!(linked, 1);
        assert_eq!(document.sectors[0].portal_sectors[1], Some(SectorId(1)));
        assert_eq!(document.sectors[1].portal_sectors[3], Some(SectorId(0)));
    }

    #[test]
    fn room_draft_splits_concave_polygon_into_clockwise_sectors() {
        let sectors = build_sectors_from_room_draft(
            &[
                vec2(-2.0, 2.0),
                vec2(0.0, 0.5),
                vec2(2.0, 2.0),
                vec2(2.0, -2.0),
                vec2(-2.0, -2.0),
            ],
            DraftSectorStyle::default(),
            10,
        )
        .unwrap();

        assert!(sectors.len() >= 2);
        for sector in sectors {
            let vertices = sector
                .vertices
                .iter()
                .map(|vertex| vertex.0)
                .collect::<Vec<_>>();
            assert!(polygon_signed_area(&vertices) < 0.0);
        }
    }

    #[test]
    fn renumbering_updates_portals_and_initial_sector() {
        let mut document = EditorDocument {
            sectors: vec![
                sector_with_vertices(2, &[(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]),
                sector_with_vertices(7, &[(1.0, 1.0), (3.0, 1.0), (3.0, -1.0), (1.0, -1.0)]),
            ],
            initial_sector: SectorId(7),
            initial_position: MapVertex(2.0, 0.0),
            initial_direction_degrees: 0.0,
        };
        document.sectors[0].portal_sectors[1] = Some(SectorId(7));
        document.sectors[1].portal_sectors[3] = Some(SectorId(2));

        renumber_sectors(&mut document);

        assert_eq!(document.sectors[0].id, SectorId(0));
        assert_eq!(document.sectors[1].id, SectorId(1));
        assert_eq!(document.initial_sector, SectorId(1));
        assert_eq!(document.sectors[0].portal_sectors[1], Some(SectorId(1)));
        assert_eq!(document.sectors[1].portal_sectors[3], Some(SectorId(0)));
    }

    #[test]
    fn new_editor_document_starts_with_valid_room() {
        let document = new_editor_document();

        assert_eq!(document.sectors.len(), 1);
        assert_eq!(document.initial_sector, SectorId(0));
        assert!(validate_document(&document).is_ok());
    }

    #[test]
    fn map_output_path_swaps_between_ron_and_protobuf() {
        assert_eq!(
            map_output_path(
                Path::new("assets/maps/test.map.ron"),
                MapFileFormat::Protobuf
            ),
            PathBuf::from("assets/maps/test.map.pb")
        );
        assert_eq!(
            map_output_path(Path::new("assets/maps/test.map.pb"), MapFileFormat::Ron),
            PathBuf::from("assets/maps/test.map.ron")
        );
    }

    #[test]
    fn normalize_map_output_path_uses_selected_format_when_missing_extension() {
        assert_eq!(
            normalize_map_output_path(PathBuf::from("assets/maps/test"), MapFileFormat::Ron),
            PathBuf::from("assets/maps/test.map.ron")
        );
        assert_eq!(
            normalize_map_output_path(PathBuf::from("assets/maps/test"), MapFileFormat::Protobuf,),
            PathBuf::from("assets/maps/test.map.pb")
        );
    }
}
