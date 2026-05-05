use eframe::egui;
use rand::Rng;

use crate::gui::CellularApp;
use crate::simulation::{compute_sim, rule_lookup_from_no};

#[derive(PartialEq)]
pub enum Screen {
    Main,
    Glance,
}

pub struct GlanceEntry {
    pub rule_no: u128,
    pub seed: u64,
    pub pixels: Vec<u8>,
    pub texture: Option<egui::TextureHandle>,
}

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        ..Default::default()
    }
}

pub fn enter_glance_view(app: &mut CellularApp) {
    let size = app.glance_sim_size;
    app.glance_entries.clear();
    for _ in 0..25 {
        let rule_no = rand::rng().random::<u128>();
        let seed = rand::rng().random::<u64>();
        let pixels = compute_sim(rule_no, size, size, 0.0, seed);
        app.glance_entries.push(GlanceEntry {
            rule_no,
            seed,
            pixels,
            texture: None,
        });
    }
    app.current_screen = Screen::Glance;
}

pub fn draw_glance_view(app: &mut CellularApp, ctx: &egui::Context) {
    let size = app.glance_sim_size;
    for entry in &mut app.glance_entries {
        if entry.texture.is_none() {
            let pixels: Vec<egui::Color32> = entry.pixels.iter()
                .map(|&v| egui::Color32::from_gray(v.saturating_mul(255)))
                .collect();
            let image = egui::ColorImage { size: [size, size], pixels };
            let tex_name = format!("glance_{}", entry.rule_no);
            entry.texture = Some(ctx.load_texture(tex_name, image, tex_options()));
        }
    }

    let mut clicked: Option<(u128, u64)> = None;

    egui::TopBottomPanel::top("glance_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Glance View");
            if ui.button("Re-roll").clicked() {
                enter_glance_view(app);
            }
            if ui.button("Back to Main").clicked() {
                app.current_screen = Screen::Main;
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

        for (i, entry) in app.glance_entries.iter().enumerate() {
            let col = i % 5;
            let row = i / 5;
            let x = grid_start.x + col as f32 * (thumb_size + gap);
            let y = grid_start.y + row as f32 * (thumb_size + gap);
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(thumb_size, thumb_size),
            );

            let resp = ui.allocate_rect(rect, egui::Sense::click());
            let painter = ui.painter_at(rect);

            if let Some(tex) = &entry.texture {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(tex.id(), rect, uv, egui::Color32::WHITE);
            }

            painter.rect_stroke(rect, 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(100)));

            if resp.clicked() {
                clicked = Some((entry.rule_no, entry.seed));
            }
        }
    });

    if let Some((rule_no, seed)) = clicked {
        app.rule_no = rule_no;
        app.rule_text = rule_no.to_string();
        app.rule_lookup = rule_lookup_from_no(rule_no);
        app.seed = seed;
        app.seed_text = seed.to_string();
        app.clear_highlight();
        app.restart_same_rule();
        app.current_screen = Screen::Main;
    }
}
