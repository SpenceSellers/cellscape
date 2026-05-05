use eframe::egui;
use rand::Rng;

use crate::simulation::compute_sim;

#[derive(PartialEq)]
pub enum Screen {
    Main,
    Glance,
    Adjacent,
}

pub enum GlanceAction {
    None,
    SelectRule(u128, u64),
    Back,
}

struct GlanceEntry {
    rule_no: u128,
    seed: u64,
    pixels: Vec<u8>,
    texture: Option<egui::TextureHandle>,
}

pub struct GalleryState {
    entries: Vec<GlanceEntry>,
    sim_size: usize,
    prerun_size: usize, // How many steps to run simulation before actually showing the results
    render_scale: u32,
    cols: usize,
    title: &'static str,
    allow_reroll: bool,
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
        }
    }
}

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        ..Default::default()
    }
}

pub fn enter_glance_view(state: &mut GalleryState) {
    let size = state.sim_size * state.render_scale as usize;
    state.entries.clear();
    for _ in 0..50 {
        let rule_no = rand::rng().random::<u128>();
        let seed = rand::rng().random::<u64>();
        let pixels = compute_sim(rule_no, size, size, 0.0, seed, state.prerun_size);
        state.entries.push(GlanceEntry { rule_no, seed, pixels, texture: None });
    }
}

pub fn enter_adjacent_view(state: &mut GalleryState, base_rule: u128, seed: u64) {
    let size = state.sim_size * state.render_scale as usize;
    state.entries.clear();
    for bit in 0..128u32 {
        let rule_no = base_rule ^ (1u128 << bit);
        let pixels = compute_sim(rule_no, size, size, 0.0, seed, state.prerun_size);
        state.entries.push(GlanceEntry { rule_no, seed, pixels, texture: None });
    }
}

pub fn draw_gallery(state: &mut GalleryState, ctx: &egui::Context) -> GlanceAction {
    let expected_size = state.sim_size * state.render_scale as usize;

    for entry in &mut state.entries {
        let tex_size = entry.pixels.len().isqrt();
        if tex_size != expected_size || entry.texture.is_none() {
            entry.pixels = compute_sim(entry.rule_no, expected_size, expected_size, 0.0, entry.seed, state.prerun_size);
            entry.texture = None;
        }
        if entry.texture.is_none() {
            let pixels: Vec<egui::Color32> = entry.pixels.iter()
                .map(|&v| egui::Color32::from_gray(v.saturating_mul(255)))
                .collect();
            let image = egui::ColorImage { size: [expected_size, expected_size], pixels };
            let tex_name = format!("gallery_{}_{}", entry.rule_no, entry.seed);
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
                enter_glance_view(state);
            }
            if ui.button("Back to Main").clicked() {
                action = GlanceAction::Back;
            }
            ui.separator();
            ui.label("Render Scale:");
            let mut scale = state.render_scale;
            if ui.add(egui::Slider::new(&mut scale, 1..=16).text("x")).changed() {
                state.render_scale = scale;
                for entry in &mut state.entries {
                    entry.texture = None;
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
            action = GlanceAction::SelectRule(entry.rule_no, entry.seed);
        }
    });

    action
}
