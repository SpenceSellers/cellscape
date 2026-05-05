use eframe::egui;
use rand::Rng;

use crate::simulation::compute_sim;

#[derive(PartialEq)]
pub enum Screen {
    Main,
    Glance,
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

pub struct GlanceState {
    entries: Vec<GlanceEntry>,
    sim_size: usize,
    render_scale: u32,
}

impl GlanceState {
    pub fn new() -> Self {
        GlanceState {
            entries: Vec::new(),
            sim_size: 80,
            render_scale: 2,
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

pub fn enter_glance_view(state: &mut GlanceState) {
    let size = state.sim_size * state.render_scale as usize;
    state.entries.clear();
    for _ in 0..25 {
        let rule_no = rand::rng().random::<u128>();
        let seed = rand::rng().random::<u64>();
        let pixels = compute_sim(rule_no, size, size, 0.0, seed);
        state.entries.push(GlanceEntry {
            rule_no,
            seed,
            pixels,
            texture: None,
        });
    }
}

pub fn draw_glance_view(state: &mut GlanceState, ctx: &egui::Context) -> GlanceAction {
    let size = state.sim_size;

    // Re-render all entries if their texture resolution doesn't match current scale
    for entry in &mut state.entries {
        let tex_size = entry.pixels.len().isqrt();
        let expected_size = size * state.render_scale as usize;
        if tex_size != expected_size || entry.texture.is_none() {
            entry.pixels = compute_sim(entry.rule_no, expected_size, expected_size, 0.0, entry.seed);
            entry.texture = None;
        }
        if entry.texture.is_none() {
            let pixels: Vec<egui::Color32> = entry.pixels.iter()
                .map(|&v| egui::Color32::from_gray(v.saturating_mul(255)))
                .collect();
            let image = egui::ColorImage { size: [expected_size, expected_size], pixels };
            let tex_name = format!("glance_{}", entry.rule_no);
            entry.texture = Some(ctx.load_texture(tex_name, image, tex_options()));
        }
    }

    let mut action = GlanceAction::None;

    egui::TopBottomPanel::top("glance_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Glance View");
            if ui.button("Re-roll (E)").clicked()
                || ui.input(|i| i.key_pressed(egui::Key::E))
            {
                enter_glance_view(state);
            }
            if ui.button("Back to Main").clicked() {
                action = GlanceAction::Back;
            }
            ui.separator();
            ui.label("Render Scale:");
            let mut scale = state.render_scale as u32;
            if ui
                .add(egui::Slider::new(&mut scale, 1..=16).text("x"))
                .changed()
            {
                state.render_scale = scale;
                // re-render by resetting all textures
                for entry in &mut state.entries {
                    entry.texture = None;
                }
            }
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_rect_before_wrap();
        let padding = 8.0;
        let gap = 8.0;
        let thumb_w = (avail.width() - padding * 2.0 - gap * 4.0) / 5.0;
        let thumb_h = (avail.height() - padding * 2.0 - gap * 4.0) / 5.0;
        let thumb_size = thumb_w.min(thumb_h);

        let grid_start = egui::pos2(
            avail.min.x + (avail.width() - thumb_size * 5.0 - gap * 4.0) / 2.0,
            avail.min.y + (avail.height() - thumb_size * 5.0 - gap * 4.0) / 2.0,
        );

        for (i, entry) in state.entries.iter().enumerate() {
            let col = i % 5;
            let row = i / 5;
            let x = grid_start.x + col as f32 * (thumb_size + gap);
            let y = grid_start.y + row as f32 * (thumb_size + gap);
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(thumb_size, thumb_size),
            );

            let resp = ui.allocate_rect(rect, egui::Sense::click());

            let hover_scale = if resp.hovered() { 1.06 } else { 1.0 };
            let display_rect = egui::Rect::from_center_size(rect.center(), rect.size() * hover_scale);

            let painter = ui.painter_at(display_rect);

            if let Some(tex) = &entry.texture {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(tex.id(), display_rect, uv, egui::Color32::WHITE);
            }

            let border_color = if resp.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_gray(100)
            };
            painter.rect_stroke(display_rect, 1.0, egui::Stroke::new(2.0, border_color));

            if resp.clicked() {
                action = GlanceAction::SelectRule(entry.rule_no, entry.seed);
            }
        }
    });

    action
}
