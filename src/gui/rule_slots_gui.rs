use eframe::egui;
use crate::glance_view::{enter_saved_rules_view, Screen};
use crate::rule_meta::draw_rule_meta_params;
use crate::simulation::{params_to_json, persist_saved_rules};
use super::CellularApp;

struct SlotChange {
    slot: usize,
    kind: SlotChangeKind,
}

enum SlotChangeKind {
    NumStates(usize),
    HalfWidth(usize),
    Noise(f64),
    NewRandom,
    LoadFromSaved,
    SaveRule,
    EditThis,
    RemoveSlot,
}

pub fn draw_rule_slots(app: &mut CellularApp, ui: &mut egui::Ui) {
    let num_rule_slots = app.setup.rules.len();
    let multi = num_rule_slots > 1;

    let mut pending: Option<SlotChange> = None;

    for slot in 0..num_rule_slots {
        if slot >= app.slot_texts.len() {
            app.slot_texts.push(params_to_json(&app.setup.rules[slot]));
        }

        let label = if multi {
            app.setup.mode.slot_label(slot)
        } else {
            String::new()
        };

        let draw_slot_contents = |ui: &mut egui::Ui, app: &mut CellularApp, pending: &mut Option<SlotChange>| {
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
                if ui.button("New random").clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::NewRandom });
                }
                let edit_label = if app.editor_active_rule == slot && app.show_rule_editor {
                    "Editing ✓"
                } else {
                    "Edit this rule"
                };
                if ui.button(edit_label).clicked() && pending.is_none() {
                    *pending = Some(SlotChange { slot, kind: SlotChangeKind::EditThis });
                }
                if app.setup.mode.supports_variable_rules() && num_rule_slots > 1 {
                    if ui.button("- Remove Rule").clicked() && pending.is_none() {
                        *pending = Some(SlotChange { slot, kind: SlotChangeKind::RemoveSlot });
                    }
                }
            });
        };

        if multi {
            egui::CollapsingHeader::new(&label)
                .default_open(true)
                .id_salt(format!("rule_slot_{slot}"))
                .show(ui, |ui| {
                    draw_slot_contents(ui, app, &mut pending);
                });
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
            SlotChangeKind::NewRandom => app.new_rule_for_slot(slot),
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
                    enter_saved_rules_view(&mut app.saved_rules_state, &app.saved_rules);
                    app.current_screen = Screen::SavedRules;
                }
            }
            SlotChangeKind::EditThis => {
                app.editor_active_rule = slot;
                app.show_rule_editor = true;
            }
        }
    }
}
