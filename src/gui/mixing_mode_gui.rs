use eframe::egui;
use crate::glance_view::{Screen, enter_mode_explore_view};
use crate::simulation::MixingMode;
use super::CellularApp;

pub fn draw_mixing_mode(app: &mut CellularApp, ui: &mut egui::Ui) {
    ui.label("Mixing Mode:");
    ui.horizontal_wrapped(|ui| {
        let is_single = matches!(app.setup.mode, MixingMode::Single);
        let is_divided = matches!(app.setup.mode, MixingMode::Divided { .. });
        let is_alt = matches!(app.setup.mode, MixingMode::Alternating { .. });
        let is_checkerboard = matches!(app.setup.mode, MixingMode::Checkerboard { .. });
        let is_circle = matches!(app.setup.mode, MixingMode::Circle { .. });
        let is_masked = matches!(app.setup.mode, MixingMode::Masked { .. });

        if ui.selectable_label(is_single, "Single").clicked() && !is_single {
            app.setup.mode = MixingMode::Single;
            app.setup.rules.truncate(1);
            app.editor_active_rule = 0;
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
        if ui.selectable_label(is_divided, "Divided").clicked() && !is_divided {
            if app.setup.rules.len() < 2 { app.push_random_slot(); }
            app.setup.rules.truncate(2);
            app.setup.mode = MixingMode::Divided { fraction: 0.5, angle_degrees: 0.0 };
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
        if ui.selectable_label(is_alt, "Alternating").clicked() && !is_alt {
            if app.setup.rules.len() < 2 { app.push_random_slot(); }
            app.setup.mode = MixingMode::Alternating { stripe_height: 20, angle_degrees: 0.0 };
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
        if ui.selectable_label(is_checkerboard, "Checkerboard").clicked() && !is_checkerboard {
            if app.setup.rules.len() < 2 { app.push_random_slot(); }
            app.setup.rules.truncate(2);
            app.setup.mode = MixingMode::Checkerboard { square_size: 20 };
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
        if ui.selectable_label(is_circle, "Circle").clicked() && !is_circle {
            if app.setup.rules.len() < 2 { app.push_random_slot(); }
            app.setup.rules.truncate(2);
            app.setup.mode = MixingMode::Circle { radius_pct: 0.5 };
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ui.selectable_label(is_masked, "Masked").clicked() && !is_masked {
            if app.setup.rules.len() < 2 { app.push_random_slot(); }
            app.setup.rules.truncate(2);
            app.setup.mode = MixingMode::Masked { mask_data: std::sync::Arc::new(Vec::new()) };
            app.sync_texts();
            app.clear_highlight();
            app.restart_same_rule();
        }
    });

    // Mode-specific controls — clone to avoid holding a borrow on app while calling sync/restart
    let mode_snapshot = app.setup.mode.clone();
    let mut new_mode: Option<MixingMode> = None;
    match mode_snapshot {
        MixingMode::Divided { mut fraction, mut angle_degrees } => {
            ui.horizontal(|ui| {
                ui.label("Fraction:");
                let resp = ui.add(egui::Slider::new(&mut fraction, 0.0f32..=1.0).fixed_decimals(2));
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Divided { fraction, angle_degrees });
                }
            });
            ui.horizontal(|ui| {
                ui.label("Angle:");
                let resp = ui.add(egui::DragValue::new(&mut angle_degrees).suffix("°").fixed_decimals(0));
                if resp.changed() {
                    app.setup.mode = MixingMode::Divided { fraction, angle_degrees };
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Divided { fraction, angle_degrees });
                }
                for preset in [0.0_f32, 90.0, 45.0, 135.0] {
                    if ui.button(format!("{}°", preset as i32)).clicked() {
                        angle_degrees = preset;
                        new_mode = Some(MixingMode::Divided { fraction, angle_degrees });
                    }
                }
            });
            if ui.button("Explore…").clicked() {
                app.mode_explore_state.selected_palette = app.selected_palette;
                app.mode_explore_state.set_palette(app.state_palette.clone());
                enter_mode_explore_view(&mut app.mode_explore_state, &app.setup);
                app.current_screen = Screen::ModeExplore;
            }
        }
        MixingMode::Alternating { mut stripe_height, mut angle_degrees } => {
            ui.horizontal(|ui| {
                ui.label("Stripe size:");
                let resp = ui.add(egui::DragValue::new(&mut stripe_height).suffix(" rows").range(1u32..=u32::MAX));
                if resp.changed() {
                    app.setup.mode = MixingMode::Alternating { stripe_height, angle_degrees };
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Alternating { stripe_height, angle_degrees });
                }
            });
            ui.horizontal(|ui| {
                ui.label("Angle:");
                let resp = ui.add(egui::DragValue::new(&mut angle_degrees).suffix("°").fixed_decimals(0));
                if resp.changed() {
                    app.setup.mode = MixingMode::Alternating { stripe_height, angle_degrees };
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Alternating { stripe_height, angle_degrees });
                }
                for preset in [0.0_f32, 90.0, 45.0, 135.0] {
                    if ui.button(format!("{}°", preset as i32)).clicked() {
                        angle_degrees = preset;
                        new_mode = Some(MixingMode::Alternating { stripe_height, angle_degrees });
                    }
                }
            });
            if ui.button("Explore…").clicked() {
                app.mode_explore_state.selected_palette = app.selected_palette;
                app.mode_explore_state.set_palette(app.state_palette.clone());
                enter_mode_explore_view(&mut app.mode_explore_state, &app.setup);
                app.current_screen = Screen::ModeExplore;
            }
        }
        MixingMode::Checkerboard { mut square_size } => {
            ui.horizontal(|ui| {
                ui.label("Square size:");
                let resp = ui.add(egui::DragValue::new(&mut square_size).suffix(" cells").range(1u32..=u32::MAX));
                if resp.changed() {
                    app.setup.mode = MixingMode::Checkerboard { square_size };
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Checkerboard { square_size });
                }
            });
            if ui.button("Explore…").clicked() {
                app.mode_explore_state.selected_palette = app.selected_palette;
                app.mode_explore_state.set_palette(app.state_palette.clone());
                enter_mode_explore_view(&mut app.mode_explore_state, &app.setup);
                app.current_screen = Screen::ModeExplore;
            }
        }
        MixingMode::Circle { mut radius_pct } => {
            ui.horizontal(|ui| {
                ui.label("Radius:");
                let resp = ui.add(egui::Slider::new(&mut radius_pct, 0.0f32..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                if resp.drag_stopped() || resp.lost_focus() {
                    new_mode = Some(MixingMode::Circle { radius_pct });
                }
            });
            if ui.button("Explore…").clicked() {
                app.mode_explore_state.selected_palette = app.selected_palette;
                app.mode_explore_state.set_palette(app.state_palette.clone());
                enter_mode_explore_view(&mut app.mode_explore_state, &app.setup);
                app.current_screen = Screen::ModeExplore;
            }
        }
        MixingMode::Single => {}
        MixingMode::Masked { mask_data } => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if ui.button("Upload mask image…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff"])
                        .pick_file()
                    {
                        if let Ok(dyn_img) = image::open(&path) {
                            let gray = dyn_img.into_luma8();
                            let scaled = std::sync::Arc::new(super::scale_mask(&gray, app.sim_width, app.sim_height));
                            app.setup.mode = MixingMode::Masked { mask_data: scaled };
                            app.mask_source = Some(gray);
                            app.restart_same_rule();
                        }
                    }
                }
                if mask_data.is_empty() {
                    ui.label("No mask loaded — all cells use rule A.");
                }
            }
        }
    }
    if let Some(mode) = new_mode {
        app.setup.mode = mode;
        app.sync_texts();
        app.restart_same_rule();
    }
}
