use eframe::egui;
use crate::glance_view::{enter_mixed_adjacent_view, enter_mixed_glance_view, enter_saved_rules_view, Screen};
use crate::rule_meta::draw_rule_meta_params;
use crate::simulation::{compute_sim, params_to_json, persist_saved_rules, rule_string_from_lookup};
use super::{CellularApp, RuleSlot};

const PREVIEW_W: usize = 320;
const PREVIEW_H: usize = 150;
const PREVIEW_PRERUN: usize = 40;
const PREVIEW_DISPLAY_W: f32 = 160.0;
const PREVIEW_DISPLAY_H: f32 = 75.0;


struct SlotChange {
    slot: usize,
    kind: SlotChangeKind,
}

enum SlotChangeKind {
    NumStates(usize),
    HalfWidth(usize),
    Noise(f64),
    ExploreRandom,
    ExploreAdjacent,
    LoadFromSaved,
    SaveRule,
    RemoveSlot,
}

pub fn draw_rule_slots(app: &mut CellularApp, ui: &mut egui::Ui) {
    let num_rule_slots = app.setup.rules.len();
    let multi = num_rule_slots > 1;

    let mut pending: Option<SlotChange> = None;

    for slot in 0..num_rule_slots {
        while app.rule_slots.len() <= slot {
            let i = app.rule_slots.len();
            app.rule_slots.push(RuleSlot { text: params_to_json(&app.setup.rules[i]), preview_texture: None });
        }

        let label = if multi {
            app.setup.mode.slot_label(slot)
        } else {
            String::new()
        };

        let draw_slot_contents = |ui: &mut egui::Ui, app: &mut CellularApp, pending: &mut Option<SlotChange>| {
            if app.rule_slots[slot].preview_texture.is_none() {
                let raw = compute_sim(&app.setup.rules[slot], PREVIEW_W, PREVIEW_H, PREVIEW_PRERUN);
                let name = format!("slot_preview_{}_{}",
                    rule_string_from_lookup(&app.setup.rules[slot].rule),
                    app.setup.rules[slot].seed);
                app.rule_slots[slot].preview_texture = Some(crate::texture::make_sim_texture(
                    ui.ctx(), &name, &raw, PREVIEW_W, PREVIEW_H, &app.state_palette,
                ));
            }
            if let Some(tex) = &app.rule_slots[slot].preview_texture {
                let resp = ui.allocate_response(
                    egui::vec2(PREVIEW_DISPLAY_W, PREVIEW_DISPLAY_H),
                    egui::Sense::hover(),
                );
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter_at(resp.rect).image(tex.id(), resp.rect, uv, egui::Color32::WHITE);
            }

            let mut num_states = app.setup.rules[slot].rule.num_states;
            let mut half_width = app.setup.rules[slot].rule.half_width;
            let mut noise = app.setup.rules[slot].noise;

            let meta_resp = draw_rule_meta_params(ui, &mut num_states, &mut half_width, &mut noise, true);
            if meta_resp.num_states_changed && pending.is_none() {
                *pending = Some(SlotChange { slot, kind: SlotChangeKind::NumStates(num_states) });
            }
            if meta_resp.half_width_changed && pending.is_none() {
                *pending = Some(SlotChange { slot, kind: SlotChangeKind::HalfWidth(half_width) });
            }
            if meta_resp.noise_changed && pending.is_none() {
                *pending = Some(SlotChange { slot, kind: SlotChangeKind::Noise(noise) });
            }

            ui.horizontal(|ui| {
                if ui.button("Save Rule").clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::SaveRule });
                }
                if ui.button("Load from Saved…").clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::LoadFromSaved });
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Explore random").clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::ExploreRandom });
                }
                if ui.button("Explore adjacent").clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::ExploreAdjacent });
                }
            });
        };

        if multi {
            let can_remove = app.setup.mode.supports_variable_rules();
            let mut remove_clicked = false;
            let id = ui.make_persistent_id(format!("rule_slot_{slot}"));
            let state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(), id, true,
            );
            let header = state.show_header(ui, |ui| {
                ui.label(&label);
                if can_remove {
                    remove_clicked = ui.button("🗑").clicked();
                }
            });
            header.body(|ui| {
                draw_slot_contents(ui, app, &mut pending);
            });
            if remove_clicked && pending.is_none() {
                pending = Some(SlotChange { slot, kind: SlotChangeKind::RemoveSlot });
            }
        } else {
            draw_slot_contents(ui, app, &mut pending);
        }
    }

    if app.setup.mode.supports_variable_rules() {
        if ui.button("+ Add Rule").clicked() {
            app.push_random_slot();
            app.sync_texts();
            app.restart_same_rule();
        }
    }

    if let Some(change) = pending {
        let slot = change.slot;
        match change.kind {
            SlotChangeKind::NumStates(k) => app.change_num_states_for_slot(slot, k),
            SlotChangeKind::HalfWidth(hw) => app.change_half_width_for_slot(slot, hw),
            SlotChangeKind::Noise(n) => {
                app.setup.rules[slot].noise = n;
                app.sync_texts();
                app.restart_same_rule();
            }
            SlotChangeKind::ExploreRandom => {
                app.saved_rules_slot = Some(slot);
                app.mixed_glance_state.selected_palette = app.selected_palette;
                app.mixed_glance_state.set_palette(app.state_palette.clone());
                enter_mixed_glance_view(&mut app.mixed_glance_state, &app.setup, slot);
                app.current_screen = Screen::MixedGlance;
            }
            SlotChangeKind::ExploreAdjacent => {
                app.saved_rules_slot = Some(slot);
                app.mixed_adjacent_state.selected_palette = app.selected_palette;
                app.mixed_adjacent_state.set_palette(app.state_palette.clone());
                enter_mixed_adjacent_view(&mut app.mixed_adjacent_state, &app.setup, slot);
                app.current_screen = Screen::MixedAdjacent;
            }
            SlotChangeKind::RemoveSlot => {
                app.setup.rules.remove(slot);
                app.editor_active_rule = app.editor_active_rule.min(app.setup.rules.len() - 1);
                app.sync_texts();
                app.restart_same_rule();
            }
            SlotChangeKind::SaveRule => {
                app.saved_rules.push(app.setup.rules[slot].clone());
                persist_saved_rules(&app.saved_rules);
            }
            SlotChangeKind::LoadFromSaved => {
                if !app.saved_rules.is_empty() {
                    app.saved_rules_slot = Some(slot);
                    app.saved_rules_state.selected_palette = app.selected_palette;
                    app.saved_rules_state.set_palette(app.state_palette.clone());
                    enter_saved_rules_view(&mut app.saved_rules_state, &app.saved_rules, Some((&app.setup, slot)));
                    app.current_screen = Screen::SavedRules;
                }
            }
        }
    }
}
