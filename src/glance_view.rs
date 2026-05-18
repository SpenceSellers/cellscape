use eframe::egui;
use rand::Rng;

use crate::palette::{ColorPalette, build_palette, draw_palette_params};
use crate::rule_meta::draw_rule_meta_params;
use crate::simulation::{compute_sim_setup, random_rule, rule_string_from_lookup, CellSource, SimParameters, SimSetup, DEFAULT_NOISE};

#[derive(PartialEq)]
pub enum Screen {
    Main,
    Glance,
    Adjacent,
    MixedAdjacent,
    MixedGlance,
    ModeExplore,
    SavedRules,
}

pub enum GlanceAction {
    None,
    SelectSetup(SimSetup),
    DeleteRule(usize),
    Back,
}

struct GlanceEntry {
    setup: SimSetup,
    pixels: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    name: String,
}

pub struct GalleryState {
    entries: Vec<GlanceEntry>,
    sim_size: usize,
    prerun_size: usize,
    render_scale: u32,
    cols: usize,
    title: &'static str,
    allow_reroll: bool,
    show_delete: bool,
    delete_confirm_idx: Option<usize>,
    num_states: usize,
    half_width: usize,
    pub noise: f64,
    pub selected_palette: ColorPalette,
    pub palette: Vec<egui::Color32>,
    reroll_setup: Option<(SimSetup, usize)>,
}

impl GalleryState {
    pub fn new_glance() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 8,
            title: "Glance View",
            allow_reroll: true,
            show_delete: false,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn new_adjacent() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 8,
            title: "Adjacent Rules",
            allow_reroll: false,
            show_delete: false,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn new_saved() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 8,
            title: "Saved Rules",
            allow_reroll: false,
            show_delete: true,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn new_mixed_adjacent() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 8,
            title: "Adjacent (Mixed Mode)",
            allow_reroll: false,
            show_delete: false,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn new_mixed_glance() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 8,
            title: "Random (Mixed Mode)",
            allow_reroll: true,
            show_delete: false,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn new_mode_explore() -> Self {
        GalleryState {
            entries: Vec::new(),
            sim_size: 80,
            prerun_size: 80,
            render_scale: 2,
            cols: 6,
            title: "Explore Mode Parameters",
            allow_reroll: false,
            show_delete: false,
            delete_confirm_idx: None,
            num_states: 2,
            half_width: 3,
            noise: DEFAULT_NOISE,
            selected_palette: ColorPalette::Classic,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
            reroll_setup: None,
        }
    }

    pub fn set_num_states(&mut self, num_states: usize) {
        self.num_states = num_states;
    }

    pub fn set_palette(&mut self, palette: Vec<egui::Color32>) {
        self.palette = palette;
        for entry in &mut self.entries {
            entry.texture = None;
        }
    }
}

fn push_entry(state: &mut GalleryState, setup: SimSetup, name: String) {
    let size = state.sim_size * state.render_scale as usize;
    let pixels = compute_sim_setup(&setup, size, size, state.prerun_size);
    state.entries.push(GlanceEntry { setup, pixels, texture: None, dirty: false, name });
}

pub fn enter_glance_view(state: &mut GalleryState, num_states: usize, half_width: usize) {
    state.num_states = num_states;
    state.half_width = half_width;
    state.reroll_setup = None;
    state.entries.clear();
    for i in 0..50 {
        let rule = random_rule(num_states, half_width, &mut rand::rng());
        let params = SimParameters { rule, noise: state.noise, seed: rand::rng().random::<u64>() };
        push_entry(state, SimSetup::single(params), i.to_string());
    }
}

pub fn enter_adjacent_view(state: &mut GalleryState, base: &SimParameters) {
    state.num_states = base.rule.num_states;
    state.half_width = base.rule.half_width;
    state.noise = base.noise;
    state.reroll_setup = None;
    state.entries.clear();
    for entry_idx in 0..base.rule.lookup.len() {
        let mut params = base.clone();
        params.rule.lookup[entry_idx] = CellSource::Static(
            (params.rule.lookup[entry_idx].static_value().unwrap_or(0) + 1) % base.rule.num_states as u8,
        );
        push_entry(state, SimSetup::single(params), entry_idx.to_string());
    }
}

pub fn enter_saved_rules_view(
    state: &mut GalleryState,
    saved: &[SimParameters],
    slot_context: Option<(&SimSetup, usize)>,
) {
    state.delete_confirm_idx = None;
    state.reroll_setup = None;
    state.entries.clear();
    for (i, params) in saved.iter().enumerate() {
        let setup = match slot_context {
            Some((base, slot)) => {
                let mut s = base.clone();
                s.rules[slot] = params.clone();
                s
            }
            None => SimSetup::single(params.clone()),
        };
        push_entry(state, setup, i.to_string());
    }
}

pub fn enter_mixed_adjacent_view(state: &mut GalleryState, base_setup: &SimSetup, slot: usize) {
    let base = &base_setup.rules[slot];
    state.num_states = base.rule.num_states;
    state.half_width = base.rule.half_width;
    state.noise = base.noise;
    state.reroll_setup = None;
    state.entries.clear();
    for entry_idx in 0..base.rule.lookup.len() {
        let mut setup = base_setup.clone();
        setup.rules[slot].rule.lookup[entry_idx] = CellSource::Static(
            (setup.rules[slot].rule.lookup[entry_idx].static_value().unwrap_or(0) + 1)
                % base.rule.num_states as u8,
        );
        push_entry(state, setup, entry_idx.to_string());
    }
}

pub fn enter_mixed_glance_view(state: &mut GalleryState, base_setup: &SimSetup, slot: usize) {
    let base = &base_setup.rules[slot];
    state.num_states = base.rule.num_states;
    state.half_width = base.rule.half_width;
    state.noise = base.noise;
    state.reroll_setup = Some((base_setup.clone(), slot));
    state.entries.clear();
    for i in 0..50 {
        let rule = random_rule(base.rule.num_states, base.rule.half_width, &mut rand::rng());
        let mut setup = base_setup.clone();
        setup.rules[slot].rule = rule;
        push_entry(state, setup, i.to_string());
    }
}

pub fn enter_mode_explore_view(state: &mut GalleryState, entries: Vec<(SimSetup, String)>) {
    state.reroll_setup = None;
    state.entries.clear();
    if let Some((first, _)) = entries.first() {
        state.num_states = first.max_num_states();
    }
    for (setup, name) in entries {
        push_entry(state, setup, name);
    }
}

pub fn draw_gallery(state: &mut GalleryState, ctx: &egui::Context) -> GlanceAction {
    let expected_size = state.sim_size * state.render_scale as usize;

    for entry in &mut state.entries {
        let tex_size = entry.pixels.len().isqrt();
        if tex_size != expected_size || entry.texture.is_none() || entry.dirty {
            entry.pixels = compute_sim_setup(&entry.setup, expected_size, expected_size, state.prerun_size);
            entry.dirty = false;
            entry.texture = None;
        }
        if entry.texture.is_none() {
            let tex_name = format!(
                "gallery_{}_{}_{}", entry.name,
                rule_string_from_lookup(&entry.setup.rules[0].rule),
                entry.setup.rules[0].seed,
            );
            entry.texture = Some(crate::texture::make_sim_texture(
                ctx, &tex_name, &entry.pixels, expected_size, expected_size, &state.palette,
            ));
        }
    }

    let mut action = GlanceAction::None;

    egui::SidePanel::right("gallery_side").show(ctx, |ui| {
        ui.heading(state.title);
        if state.allow_reroll
            && (ui.button("Re-roll (E)").clicked()
                || ui.input(|i| i.key_pressed(egui::Key::E)))
        {
            if let Some((setup, slot)) = state.reroll_setup.clone() {
                enter_mixed_glance_view(state, &setup, slot);
            } else {
                enter_glance_view(state, state.num_states, state.half_width);
            }
        }
        if ui.button("Back to Main").clicked() {
            action = GlanceAction::Back;
        }

        ui.separator();

        let mut num_states = state.num_states;
        let mut half_width = state.half_width;
        let mut noise = state.noise;
        let meta_resp = draw_rule_meta_params(ui, &mut num_states, &mut half_width, &mut noise, state.allow_reroll);
        if (meta_resp.num_states_changed || meta_resp.half_width_changed) && state.allow_reroll {
            state.noise = noise;
            if let Some((setup, slot)) = state.reroll_setup.clone() {
                enter_mixed_glance_view(state, &setup, slot);
            } else {
                enter_glance_view(state, num_states, half_width);
            }
            state.palette = build_palette(state.selected_palette, state.num_states);
        }
        if meta_resp.noise_changed {
            state.noise = noise;
            let reroll_slot = state.reroll_setup.as_ref().map(|(_, s)| *s);
            for entry in &mut state.entries {
                if let Some(slot) = reroll_slot {
                    entry.setup.rules[slot].noise = noise;
                } else {
                    for rule in &mut entry.setup.rules {
                        rule.noise = noise;
                    }
                }
                entry.dirty = true;
            }
        }

        ui.separator();

        if draw_palette_params(ui, &mut state.selected_palette, &mut state.palette, state.num_states) {
            for entry in &mut state.entries {
                entry.texture = None;
            }
        }

        ui.separator();
        ui.label("Render Scale:");
        let mut scale = state.render_scale;
        if ui.add(egui::Slider::new(&mut scale, 1..=16).text("x")).changed() {
            state.render_scale = scale;
        }
        ui.separator();
        ui.label("Columns:");
        ui.add(egui::Slider::new(&mut state.cols, 1..=20));
        ui.separator();
        ui.label("Pre-run Steps:");
        let mut prerun = state.prerun_size;
        if ui.add(egui::Slider::new(&mut prerun, 0..=500)).changed() {
            state.prerun_size = prerun;
            for entry in &mut state.entries {
                entry.dirty = true;
            }
        }
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let cols = state.cols;
        let gap = 8.0;

        let mut clicked_idx: Option<usize> = None;
        let mut delete_clicked_idx: Option<usize> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let avail_width = ui.available_width();
            let thumb_size = (avail_width - gap * (cols - 1) as f32) / cols as f32;
            ui.spacing_mut().item_spacing = egui::vec2(gap, gap);

            for (row_idx, chunk) in state.entries.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (j, entry) in chunk.iter().enumerate() {
                        let idx = row_idx * cols + j;
                        ui.vertical(|ui| {
                            ui.set_max_width(thumb_size);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let resp = ui.allocate_response(
                                egui::vec2(thumb_size, thumb_size),
                                egui::Sense::click(),
                            );
                            let rect = resp.rect;
                            let hover_scale = if resp.hovered() { 1.06 } else { 1.0 };
                            let display_rect = egui::Rect::from_center_size(
                                rect.center(),
                                rect.size() * hover_scale,
                            );
                            let painter = ui.painter_at(display_rect);
                            if let Some(tex) = &entry.texture {
                                let uv = egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                );
                                painter.image(tex.id(), display_rect, uv, egui::Color32::WHITE);
                            }
                            let border_color = if resp.hovered() {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_gray(100)
                            };
                            painter.rect_stroke(
                                display_rect,
                                1.0,
                                egui::Stroke::new(2.0, border_color),
                            );
                            if resp.clicked() {
                                clicked_idx = Some(idx);
                            }

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&entry.name).size(15.0));
                                if state.show_delete {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(egui::RichText::new("×").size(15.0)).clicked() {
                                            delete_clicked_idx = Some(idx);
                                        }
                                    });
                                }
                            });
                        });
                    }
                });
            }
        });

        if let Some(idx) = delete_clicked_idx {
            state.delete_confirm_idx = Some(idx);
        }

        if let Some(idx) = clicked_idx {
            action = GlanceAction::SelectSetup(state.entries[idx].setup.clone());
        }
    });

    if let Some(idx) = state.delete_confirm_idx {
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new("Delete Rule?")
            .id(egui::Id::new("delete_confirm_modal"))
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Delete saved rule {}?", idx));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
        if confirmed {
            action = GlanceAction::DeleteRule(idx);
            state.delete_confirm_idx = None;
        } else if cancelled {
            state.delete_confirm_idx = None;
        }
    }

    action
}
