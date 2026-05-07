use eframe::egui;
use rand::Rng;

use crate::simulation::{compute_sim, random_rule_lookup, rule_string_from_lookup};

#[derive(PartialEq)]
pub enum Screen {
    Main,
    Glance,
    Adjacent,
}

pub enum GlanceAction {
    None,
    SelectRule(Vec<u8>, usize, usize, u64),
    Back,
}

struct GlanceEntry {
    lookup: Vec<u8>,
    num_states: usize,
    half_width: usize,
    seed: u64,
    pixels: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
}

pub struct GalleryState {
    entries: Vec<GlanceEntry>,
    sim_size: usize,
    prerun_size: usize,
    render_scale: u32,
    cols: usize,
    title: &'static str,
    allow_reroll: bool,
    num_states: usize,
    half_width: usize,
    pub palette: Vec<egui::Color32>,
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
            num_states: 2,
            half_width: 3,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
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
            num_states: 2,
            half_width: 3,
            palette: vec![egui::Color32::BLACK, egui::Color32::WHITE],
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

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..Default::default()
    }
}

pub fn enter_glance_view(state: &mut GalleryState, num_states: usize, half_width: usize) {
    state.half_width = half_width;
    let size = state.sim_size * state.render_scale as usize;
    state.entries.clear();
    for _ in 0..50 {
        let lookup = random_rule_lookup(num_states, half_width, &mut rand::rng());
        let seed = rand::rng().random::<u64>();
        let pixels = compute_sim(&lookup, num_states, half_width, size, size, 0.0, seed, state.prerun_size);
        state.entries.push(GlanceEntry { lookup, num_states, half_width, seed, pixels, texture: None, dirty: false });
    }
}

pub fn enter_adjacent_view(state: &mut GalleryState, base_lookup: &[u8], num_states: usize, half_width: usize, seed: u64) {
    state.half_width = half_width;
    let size = state.sim_size * state.render_scale as usize;
    state.entries.clear();
    let n_entries = base_lookup.len();
    for entry_idx in 0..n_entries {
        let mut lookup = base_lookup.to_vec();
        lookup[entry_idx] = (lookup[entry_idx] + 1) % num_states as u8;
        let pixels = compute_sim(&lookup, num_states, half_width, size, size, 0.0, seed, state.prerun_size);
        state.entries.push(GlanceEntry { lookup, num_states, half_width, seed, pixels, texture: None, dirty: false });
    }
}

pub fn draw_gallery(state: &mut GalleryState, ctx: &egui::Context) -> GlanceAction {
    let expected_size = state.sim_size * state.render_scale as usize;

    for entry in &mut state.entries {
        let tex_size = entry.pixels.len().isqrt();
        if tex_size != expected_size || entry.texture.is_none() || entry.dirty {
            entry.pixels = compute_sim(&entry.lookup, entry.num_states, entry.half_width, expected_size, expected_size, 0.0, entry.seed, state.prerun_size);
            entry.dirty = false;
            entry.texture = None;
        }
        if entry.texture.is_none() {
            let pixels: Vec<egui::Color32> = entry.pixels.iter()
                .map(|&v| state.palette[v as usize])
                .collect();
            let image = egui::ColorImage { size: [expected_size, expected_size], pixels };
            let tex_name = format!("gallery_{}_{}", rule_string_from_lookup(&entry.lookup), entry.seed);
            entry.texture = Some(ctx.load_texture(tex_name, image, tex_options()));
        }
    }

    let mut action = GlanceAction::None;

    egui::TopBottomPanel::top("gallery_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(state.title);
            if state.allow_reroll
                && (ui.button("Re-roll (E)").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::E)))
            {
                enter_glance_view(state, state.num_states, state.half_width);
            }
            if ui.button("Back to Main").clicked() {
                action = GlanceAction::Back;
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
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let cols = state.cols;
        let gap = 8.0;

        let mut clicked_idx: Option<usize> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let avail_width = ui.available_width();
            let thumb_size = (avail_width - gap * (cols - 1) as f32) / cols as f32;
            ui.spacing_mut().item_spacing = egui::vec2(gap, gap);

            for (row_idx, chunk) in state.entries.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (j, entry) in chunk.iter().enumerate() {
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
                            clicked_idx = Some(row_idx * cols + j);
                        }
                    }
                });
            }
        });

        if let Some(idx) = clicked_idx {
            let entry = &state.entries[idx];
            action = GlanceAction::SelectRule(entry.lookup.clone(), entry.num_states, entry.half_width, entry.seed);
        }
    });

    action
}
