use bevy::prelude::*;
use bevy_egui::{
    egui::{self, plot},
    EguiContext, EguiPlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: "sector_edit".to_string(),
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_plugin(EguiPlugin)
        .add_system(ui_system)
        .run();
}

fn ui_system(mut egui_ctx: ResMut<EguiContext>) {
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

    let points = egui::plot::PlotPoints::new(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);

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
            // egui::plot::Plot::new("plot")
            //     .data_aspect(1.0)
            //     .show_axes([true, true])
            //     // .auto_bounds_x()
            //     .show(ui, |plot_ui| {
            //         // plot_ui.polygon(polygon_1);
            //         // plot_ui.polygon(polygon_2);
            //         plot_ui.points(egui::plot::Points::new(points));

            //         if plot_ui.plot_clicked() {
            //             println!("{:?}", plot_ui.pointer_coordinate().unwrap());

            //             println!("{:?}", plot_ui.pointer_coordinate_drag_delta());
            //         }
            //     });
        });
}
