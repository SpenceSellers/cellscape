use eframe::egui;

use crate::simulation::rule_no_from_lookup;
use crate::gui::CellularApp;

pub fn draw_rule_editor(app: &mut CellularApp, ui: &mut egui::Ui) {
    let cell_sz = 9.0_f32;
    let cell_gap = 1.0_f32;
    let nbr_cells = 7usize;
    let pat_gap = 14.0_f32;
    let out_gap = 4.0_f32;

    let nbr_w = nbr_cells as f32 * cell_sz + (nbr_cells - 1) as f32 * cell_gap;
    let tile_w = nbr_w + pat_gap;
    let tile_h = cell_sz + out_gap + cell_sz;

    ui.label(egui::RichText::new("Rule editor — click an output cell to toggle it").small());
    ui.separator();

    let mut clicked: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("rule_editor_scroll")
        .max_height(ui.available_height())
        .show(ui, |ui| {
            let avail_w = ui.available_width();
            let cols = ((avail_w / tile_w) as usize).max(1).min(128);
            let rows = (64 + cols - 1) / cols;

            for row in 0..rows {
                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(avail_w, tile_h),
                    egui::Sense::hover(),
                );
                let painter = ui.painter();

                for col in 0..cols {
                    let state = row * cols + col;
                    if state >= 64 { break; }

                    let x0 = row_rect.min.x + col as f32 * tile_w;
                    let y0 = row_rect.min.y;

                    if app.highlighted_state == Some(state) {
                        let tile_rect = egui::Rect::from_min_size(
                            egui::pos2(x0 - 2.0, y0 - 2.0),
                            egui::vec2(nbr_w + 4.0, tile_h + 4.0),
                        );
                        painter.rect_filled(
                            tile_rect, 3.0,
                            egui::Color32::from_rgb(255, 200, 50),
                        );
                        painter.rect_stroke(
                            tile_rect, 3.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 200, 50)),
                        );
                    }

                    for bit_pos in 0..nbr_cells {
                        let bit_idx = nbr_cells - 1 - bit_pos;
                        let alive = (state >> bit_idx) & 1 == 1;
                        let x = x0 + bit_pos as f32 * (cell_sz + cell_gap);
                        let r = egui::Rect::from_min_size(
                            egui::pos2(x, y0),
                            egui::vec2(cell_sz, cell_sz),
                        );
                        let fill = if alive { egui::Color32::WHITE } else { egui::Color32::BLACK };
                        painter.rect_filled(r, 1.0, fill);
                        let border = if bit_pos == 3 {
                            egui::Color32::from_rgb(80, 130, 220)
                        } else {
                            egui::Color32::from_gray(80)
                        };
                        painter.rect_stroke(r, 1.0, egui::Stroke::new(0.5, border));
                    }

                    let out_x = x0 + (nbr_w - cell_sz) / 2.0;
                    let out_y = y0 + cell_sz + out_gap;
                    let out_rect = egui::Rect::from_min_size(
                        egui::pos2(out_x, out_y),
                        egui::vec2(cell_sz, cell_sz),
                    );
                    let output = app.rule_lookup[state];
                    let fill = if output == 1 { egui::Color32::WHITE } else { egui::Color32::BLACK };
                    painter.rect_filled(out_rect, 1.0, fill);
                    painter.rect_stroke(out_rect, 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(140)));

                    let resp = ui.interact(
                        out_rect,
                        egui::Id::new(("rule_out", state)),
                        egui::Sense::click(),
                    );
                    if resp.clicked() {
                        clicked = Some(state);
                    }
                    if resp.hovered() {
                        painter.rect_stroke(
                            out_rect, 1.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 180, 255)),
                        );
                    }
                }

                ui.add_space(6.0);
            }
        });

    if let Some(state) = clicked {
        app.rule_lookup[state] = 1 - app.rule_lookup[state];
        app.rule_no = rule_no_from_lookup(&app.rule_lookup);
        app.restart_same_rule();
    }
}
