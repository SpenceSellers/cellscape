use eframe::egui;
use crate::simulation::{CellSource, setup_to_json};
use crate::gui::CellularApp;

pub struct RandomEditor {
    pub rule_idx: usize,
    pub state_idx: usize,
    pub weights: Vec<f32>,
}

impl RandomEditor {
    pub fn from_source(source: &CellSource, num_states: usize, rule_idx: usize, state_idx: usize) -> Self {
        let mut weights = vec![0.0f32; num_states];
        match source {
            CellSource::Static(v) => {
                if (*v as usize) < num_states { weights[*v as usize] = 1.0; }
            }
            CellSource::Random { cumulative, values } => {
                let mut prev = 0.0f32;
                for (&cum, &val) in cumulative.iter().zip(values.iter()) {
                    if (val as usize) < num_states { weights[val as usize] += cum - prev; }
                    prev = cum;
                }
            }
        }
        RandomEditor { rule_idx, state_idx, weights }
    }
}

pub fn draw_rule_editor(app: &mut CellularApp, ui: &mut egui::Ui) {
    // Tab strip when multiple rules are active
    let multi_rule = app.setup.rules.len() > 1;
    if multi_rule {
        ui.horizontal(|ui| {
            for i in 0..app.setup.rules.len() {
                let label = if i == 0 { "Rule A".to_string() } else { format!("Rule {}", (b'A' + i as u8) as char) };
                let selected = app.editor_active_rule == i;
                if ui.selectable_label(selected, &label).clicked() {
                    app.editor_active_rule = i;
                }
            }
        });
        ui.separator();
    }

    let rule_idx = app.editor_active_rule;
    let cell_sz = 9.0_f32;
    let cell_gap = 1.0_f32;
    let nbr_cells = 2 * app.setup.rules[rule_idx].rule.half_width + 1;
    let pat_gap = 14.0_f32;
    let out_gap = 4.0_f32;

    let nbr_w = nbr_cells as f32 * cell_sz + (nbr_cells - 1) as f32 * cell_gap;
    let tile_w = nbr_w + pat_gap;
    let tile_h = cell_sz + out_gap + cell_sz;

    let total_patterns = app.setup.rules[rule_idx].rule.lookup.len();

    ui.label(egui::RichText::new("Rule editor — left-click output to cycle state, right-click for weighted random").small());
    ui.separator();

    let mut left_clicked: Option<usize> = None;
    let mut right_clicked: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("rule_editor_scroll")
        .max_height(ui.available_height())
        .show(ui, |ui| {
            let avail_w = ui.available_width();
            let cols = ((avail_w / tile_w) as usize).clamp(1, total_patterns);
            let rows = total_patterns.div_ceil(cols);

            for row in 0..rows {
                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(avail_w, tile_h),
                    egui::Sense::hover(),
                );
                let painter = ui.painter();

                for col in 0..cols {
                    let state = row * cols + col;
                    if state >= total_patterns { break; }

                    let x0 = row_rect.min.x + col as f32 * tile_w;
                    let y0 = row_rect.min.y;

                    let highlighted = app.highlighted_state == Some((rule_idx, state));
                    if highlighted {
                        let tile_rect = egui::Rect::from_min_size(
                            egui::pos2(x0 - 2.0, y0 - 2.0),
                            egui::vec2(nbr_w + 4.0, tile_h + 4.0),
                        );
                        painter.rect_filled(tile_rect, 3.0, egui::Color32::from_rgb(255, 200, 50));
                        painter.rect_stroke(tile_rect, 3.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 200, 50)));
                    }

                    let num_states = app.setup.rules[rule_idx].rule.num_states;
                    for bit_pos in 0..nbr_cells {
                        let digit = (state / num_states.pow((nbr_cells - 1 - bit_pos) as u32)) % num_states;
                        let x = x0 + bit_pos as f32 * (cell_sz + cell_gap);
                        let r = egui::Rect::from_min_size(egui::pos2(x, y0), egui::vec2(cell_sz, cell_sz));
                        painter.rect_filled(r, 1.0, app.state_palette[digit % app.state_palette.len()]);
                        let border = if bit_pos == app.setup.rules[rule_idx].rule.half_width {
                            egui::Color32::from_rgb(80, 130, 220)
                        } else {
                            egui::Color32::from_gray(80)
                        };
                        painter.rect_stroke(r, 1.0, egui::Stroke::new(0.5, border));
                    }

                    let out_x = x0 + (nbr_w - cell_sz) / 2.0;
                    let out_y = y0 + cell_sz + out_gap;
                    let out_rect = egui::Rect::from_min_size(egui::pos2(out_x, out_y), egui::vec2(cell_sz, cell_sz));

                    match &app.setup.rules[rule_idx].rule.lookup[state] {
                        CellSource::Static(v) => {
                            painter.rect_filled(out_rect, 1.0, app.state_palette[*v as usize % app.state_palette.len()]);
                            painter.rect_stroke(out_rect, 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(140)));
                        }
                        CellSource::Random { cumulative, values } => {
                            let mut prev = 0.0f32;
                            for (&cum, &val) in cumulative.iter().zip(values.iter()) {
                                let frac = cum - prev;
                                if frac > 0.01 {
                                    let seg = egui::Rect::from_min_size(
                                        egui::pos2(out_x + prev * cell_sz, out_y),
                                        egui::vec2(frac * cell_sz, cell_sz),
                                    );
                                    painter.rect_filled(seg, 0.0, app.state_palette[val as usize % app.state_palette.len()]);
                                }
                                prev = cum;
                            }
                            let is_editing = app.random_editor.as_ref()
                                .map_or(false, |e| e.rule_idx == rule_idx && e.state_idx == state);
                            let border_color = if is_editing {
                                egui::Color32::from_rgb(220, 100, 255)
                            } else {
                                egui::Color32::from_rgb(160, 80, 200)
                            };
                            painter.rect_stroke(out_rect, 1.0, egui::Stroke::new(1.5, border_color));
                        }
                    }

                    let resp = ui.interact(out_rect, egui::Id::new(("rule_out", rule_idx, state)), egui::Sense::click());
                    if resp.clicked() {
                        left_clicked = Some(state);
                    }
                    if resp.secondary_clicked() {
                        right_clicked = Some(state);
                    }
                    if resp.hovered() {
                        painter.rect_stroke(out_rect, 1.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 180, 255)));
                    }
                }

                ui.add_space(6.0);
            }
        });

    if let Some(state) = left_clicked {
        let num_states = app.setup.rules[rule_idx].rule.num_states;
        let v = app.setup.rules[rule_idx].rule.lookup[state].static_value().unwrap_or(0);
        app.setup.rules[rule_idx].rule.lookup[state] = CellSource::Static(((v as usize + 1) % num_states) as u8);
        app.setup_text = setup_to_json(&app.setup);
        app.sync_slot_texts();
        app.restart_same_rule();
        if app.random_editor.as_ref().map_or(false, |e| e.rule_idx == rule_idx && e.state_idx == state) {
            app.random_editor = None;
        }
    }

    if let Some(state) = right_clicked {
        let num_states = app.setup.rules[rule_idx].rule.num_states;
        let editor = RandomEditor::from_source(&app.setup.rules[rule_idx].rule.lookup[state], num_states, rule_idx, state);
        app.random_editor = Some(editor);
    }
}

pub fn draw_random_editor(app: &mut CellularApp, ctx: &egui::Context) {
    if app.random_editor.is_none() { return; }

    let rule_idx = app.random_editor.as_ref().unwrap().rule_idx;
    let num_states = app.setup.rules[rule_idx].rule.num_states;
    let palette = app.state_palette.clone();
    let state_idx = app.random_editor.as_ref().unwrap().state_idx;
    let rule_label = if app.setup.rules.len() > 1 {
        format!(" (Rule {})", (b'A' + rule_idx as u8) as char)
    } else {
        String::new()
    };

    let mut open = true;
    let mut should_apply = false;
    let mut make_static = false;

    egui::Window::new(format!("Pattern #{state_idx}{rule_label}"))
        .id(egui::Id::new("random_editor_window"))
        .open(&mut open)
        .resizable(false)
        .default_width(220.0)
        .show(ctx, |ui| {
            let editor = app.random_editor.as_mut().unwrap();

            let bar_w = ui.available_width();
            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 14.0), egui::Sense::hover());
            let total: f32 = editor.weights.iter().sum();
            if total > 0.0 {
                let mut x = bar_rect.min.x;
                for (s, &w) in editor.weights.iter().enumerate() {
                    let seg_w = (w / total) * bar_w;
                    if seg_w >= 0.5 {
                        let seg = egui::Rect::from_min_size(
                            egui::pos2(x, bar_rect.min.y),
                            egui::vec2(seg_w, 14.0),
                        );
                        ui.painter().rect_filled(seg, 0.0, palette[s % palette.len()]);
                        x += seg_w;
                    }
                }
            } else {
                ui.painter().rect_filled(bar_rect, 2.0, egui::Color32::from_gray(50));
                ui.painter().text(
                    bar_rect.center(), egui::Align2::CENTER_CENTER,
                    "all zero — set at least one weight",
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(140),
                );
            }

            ui.add_space(4.0);

            for s in 0..num_states {
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(swatch, 2.0, palette[s % palette.len()]);
                    let resp = ui.add(
                        egui::Slider::new(&mut editor.weights[s], 0.0f32..=1.0)
                            .text(format!("{s}"))
                    );
                    if resp.drag_stopped() {
                        should_apply = true;
                    }
                });
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    should_apply = true;
                }
                if ui.button("→ Static").clicked() {
                    make_static = true;
                }
            });
        });

    if !open {
        app.random_editor = None;
        return;
    }

    if should_apply {
        let weights = app.random_editor.as_ref().unwrap().weights.clone();
        let weighted: Vec<(f32, u8)> = weights.iter().enumerate()
            .filter(|(_, &w)| w > 0.0)
            .map(|(i, &w)| (w, i as u8))
            .collect();
        if weighted.len() == 1 {
            app.setup.rules[rule_idx].rule.lookup[state_idx] = CellSource::Static(weighted[0].1);
        } else if weighted.len() > 1 {
            app.setup.rules[rule_idx].rule.lookup[state_idx] = CellSource::random(weighted);
        }
        app.setup_text = setup_to_json(&app.setup);
        app.sync_slot_texts();
        app.restart_same_rule();
    }

    if make_static {
        let weights = &app.random_editor.as_ref().unwrap().weights;
        let v = weights.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as u8)
            .unwrap_or(0);
        app.setup.rules[rule_idx].rule.lookup[state_idx] = CellSource::Static(v);
        app.setup_text = setup_to_json(&app.setup);
        app.sync_slot_texts();
        app.restart_same_rule();
        app.random_editor = None;
    }
}
