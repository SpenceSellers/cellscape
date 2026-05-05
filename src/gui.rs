use eframe::egui;
use rand::Rng;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Instant;

use crate::rule_editor;
use crate::simulation::{spawn_sim, SimBatch, rule_lookup_from_no, noise_from_slider, parse_seed};

pub struct CellularApp {
    pub receiver: mpsc::Receiver<SimBatch>,
    pub rows_done: usize,
    pub texture: Option<egui::TextureHandle>,
    pub sim_width: usize,
    pub sim_height: usize,
    pub sim_size: usize,
    pub rule_no: u128,
    pub rule_text: String,
    pub rule_lookup: Vec<u8>,
    pub show_rule_editor: bool,
    pub seed: u64,
    pub seed_text: String,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub view_initialized: bool,
    pub noise_slider: f64,
    pub noise_atomic: Arc<AtomicU64>,
    pub cells_data: Vec<u8>,
    pub highlighted_state: Option<usize>,
    pub highlighted_cell: Option<(usize, usize)>,
    pub saved_at: Option<Instant>,
}

impl CellularApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize) -> Self {
        let noise_slider = 0.5f64;
        let noise_atomic =
            Arc::new(AtomicU64::new(noise_from_slider(noise_slider).to_bits()));
        let rule_no = rand::rng().random::<u128>();
        let seed = rand::rng().random::<u64>();
        let receiver =
            spawn_sim(rule_no, sim_width, sim_height, Arc::clone(&noise_atomic), seed);
        let rule_lookup = rule_lookup_from_no(rule_no);

        CellularApp {
            receiver,
            rows_done: 0,
            texture: None,
            sim_size: sim_width,
            sim_width,
            sim_height,
            rule_no,
            rule_text: rule_no.to_string(),
            rule_lookup,
            show_rule_editor: false,
            seed,
            seed_text: seed.to_string(),
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            view_initialized: false,
            noise_slider,
            noise_atomic,
            cells_data: vec![0u8; sim_width * sim_height],
            highlighted_state: None,
            highlighted_cell: None,
            saved_at: None,
        }
    }

    pub fn restart_same_rule(&mut self) {
        self.receiver = spawn_sim(
            self.rule_no,
            self.sim_width,
            self.sim_height,
            Arc::clone(&self.noise_atomic),
            self.seed,
        );
        self.rows_done = 0;
    }

    pub fn resize_and_restart(&mut self, size: usize) {
        self.sim_size = size;
        self.sim_width = size;
        self.sim_height = size;
        self.receiver = spawn_sim(self.rule_no, size, size, Arc::clone(&self.noise_atomic), self.seed);
        self.rows_done = 0;
        self.texture = None;
        self.cells_data = vec![0u8; size * size];
        self.view_initialized = false;
    }

    pub fn new_rule(&mut self) {
        self.rule_no = rand::rng().random::<u128>();
        self.rule_text = self.rule_no.to_string();
        self.rule_lookup = rule_lookup_from_no(self.rule_no);
        self.highlighted_state = None;
        self.highlighted_cell = None;
        self.restart_same_rule();
    }

    fn save_image(&mut self) {
        use chrono::Local;
        use image::GrayImage;
        use std::fs;
        fs::create_dir_all("output").ok();
        let ts = Local::now().format("%Y-%m-%dT%H-%M-%S");
        let path = format!("output/{}-{}.png", ts, self.rule_no);
        let pixels: Vec<u8> = self.cells_data.iter().map(|&v| v.saturating_mul(255)).collect();
        match GrayImage::from_raw(self.sim_width as u32, self.sim_height as u32, pixels) {
            Some(img) => { img.save(&path).ok(); }
            None => {}
        }
        self.saved_at = Some(Instant::now());
    }
}

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        ..Default::default()
    }
}

impl eframe::App for CellularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.noise_atomic
            .store(noise_from_slider(self.noise_slider).to_bits(), Ordering::Relaxed);

        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_rule();
        }

        loop {
            match self.receiver.try_recv() {
                Ok(batch) => {
                    let count = batch.pixels.len() / self.sim_width;
                    for (i, &v) in batch.pixels.iter().enumerate() {
                        let row = batch.start + i / self.sim_width;
                        let col = i % self.sim_width;
                        self.cells_data[row * self.sim_width + col] = v;
                    }
                    let pixels: Vec<egui::Color32> = batch
                        .pixels
                        .iter()
                        .map(|&v| egui::Color32::from_gray(v.saturating_mul(255)))
                        .collect();
                    let partial = egui::ColorImage { size: [self.sim_width, count], pixels };
                    match &mut self.texture {
                        Some(tex) => {
                            tex.set_partial([0, batch.start], partial, tex_options());
                        }
                        None => {
                            let black = egui::ColorImage::new(
                                [self.sim_width, self.sim_height],
                                egui::Color32::BLACK,
                            );
                            let mut tex = ctx.load_texture("sim", black, tex_options());
                            tex.set_partial([0, batch.start], partial, tex_options());
                            self.texture = Some(tex);
                        }
                    }
                    self.rows_done = self.rows_done.max(batch.start + count);
                }
                Err(_) => break,
            }
        }

        egui::SidePanel::right("controls")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Rule:");
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.rule_text).desired_width(220.0),
                        );
                        if resp.lost_focus() {
                            if let Ok(n) = self.rule_text.parse::<u128>() {
                                self.rule_no = n;
                                self.rule_lookup = rule_lookup_from_no(n);
                                self.highlighted_state = None;
                                self.highlighted_cell = None;
                                self.restart_same_rule();
                            } else {
                                self.rule_text = self.rule_no.to_string();
                            }
                        }
                    });
                    ui.label(format!("{}/{} rows   zoom: {:.2}x", self.rows_done, self.sim_height, self.zoom));

                    ui.separator();

                    if ui.button("New Rule").clicked() {
                        self.new_rule();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Save PNG").clicked() {
                            self.save_image();
                        }
                        if let Some(t) = self.saved_at {
                            if t.elapsed() < std::time::Duration::from_secs(2) {
                                ui.label("Saved!");
                            } else {
                                self.saved_at = None;
                            }
                        }
                    });

                    let editor_label = if self.show_rule_editor { "Close Editor" } else { "Edit Rule" };
                    if ui.button(editor_label).clicked() {
                        self.show_rule_editor = !self.show_rule_editor;
                        if !self.show_rule_editor {
                            self.highlighted_state = None;
                            self.highlighted_cell = None;
                        }
                    }

                    ui.separator();
                    ui.label("Noise:");
                    let noise_resp = ui.add(
                        egui::Slider::new(&mut self.noise_slider, 0.0f64..=1.0)
                            .custom_formatter(|v, _| format!("{:.2e}", noise_from_slider(v)))
                            .custom_parser(|s| {
                                s.parse::<f64>().ok().and_then(|noise| {
                                    if noise > 0.0 {
                                        Some(((noise.log10() + 7.0) / 6.0).clamp(0.0, 1.0))
                                    } else {
                                        Some(0.0)
                                    }
                                })
                            }),
                    );
                    if noise_resp.changed() {
                        self.noise_atomic
                            .store(noise_from_slider(self.noise_slider).to_bits(), Ordering::Relaxed);
                        self.restart_same_rule();
                    }

                    ui.separator();
                    ui.label("Size:");
                    let size_resp = ui.add(
                        egui::Slider::new(&mut self.sim_size, 100..=16000)
                            .suffix("px")
                            .integer(),
                    );
                    if size_resp.drag_stopped() || size_resp.lost_focus() {
                        let new_size = self.sim_size;
                        self.resize_and_restart(new_size);
                    }

                    ui.separator();
                    ui.label("Seed:");
                    let seed_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.seed_text).desired_width(140.0),
                    );
                    if seed_resp.lost_focus() {
                        self.seed = parse_seed(&self.seed_text);
                        self.restart_same_rule();
                    }
                });
            });

        if self.show_rule_editor {
            egui::TopBottomPanel::bottom("rule_editor")
                .resizable(true)
                .default_height(320.0)
                .show(ctx, |ui| {
                    rule_editor::draw_rule_editor(self, ui);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let canvas = ui.available_rect_before_wrap();

            if !self.view_initialized && canvas.width() > 10.0 && canvas.height() > 10.0 {
                let sx = canvas.width() / self.sim_width as f32;
                let sy = canvas.height() / self.sim_height as f32;
                self.zoom = sx.min(sy);
                let iw = self.sim_width as f32 * self.zoom;
                let ih = self.sim_height as f32 * self.zoom;
                self.pan = egui::vec2(
                    (canvas.width() - iw) * 0.5,
                    (canvas.height() - ih) * 0.5,
                );
                self.view_initialized = true;
            }

            let response = ui.allocate_rect(canvas, egui::Sense::click_and_drag());

            if response.dragged() {
                self.pan += response.drag_delta();
            }

            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 && response.hovered() {
                let cursor = ctx
                    .input(|i| i.pointer.hover_pos())
                    .unwrap_or(canvas.center());
                let cursor_local = cursor.to_vec2() - canvas.min.to_vec2();
                let factor = (scroll * 0.001).exp();
                let new_zoom = (self.zoom * factor).clamp(0.001, 50.0);
                let actual_factor = new_zoom / self.zoom;
                self.pan = cursor_local + (self.pan - cursor_local) * actual_factor;
                self.zoom = new_zoom;
            }

            if response.clicked() && self.show_rule_editor {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    let local = (pos.to_vec2() - canvas.min.to_vec2() - self.pan) / self.zoom;
                    let col = local.x as usize;
                    let row = local.y as usize;
                    if col < self.sim_width && row < self.sim_height {
                        if row > 0 {
                            let mut state = 0usize;
                            let sim_w = self.sim_width as isize;
                            for di in -3isize..=3isize {
                                let neighbor_col = ((col as isize + di) % sim_w + sim_w) % sim_w;
                                let idx = (row - 1) * self.sim_width + neighbor_col as usize;
                                state = (state << 1) | (self.cells_data[idx] as usize);
                            }
                            self.highlighted_state = Some(state);
                            self.highlighted_cell = Some((col, row));
                        } else {
                            self.highlighted_state = None;
                            self.highlighted_cell = None;
                        }
                    } else {
                        self.highlighted_state = None;
                        self.highlighted_cell = None;
                    }
                }
            }

            let painter = ui.painter_at(canvas);
            let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            let img_w = self.sim_width as f32 * self.zoom;
            let origin = (canvas.min.to_vec2() + self.pan).to_pos2();

            if let Some(tex) = &self.texture {
                let img_h = self.sim_height as f32 * self.zoom;
                let rect = egui::Rect::from_min_size(origin, egui::vec2(img_w, img_h));
                painter.image(tex.id(), rect, full_uv, egui::Color32::WHITE);
            }

            if let Some((col, row)) = self.highlighted_cell {
                if row > 0 && col < self.sim_width && row <= self.sim_height {
                    let sim_w = self.sim_width as isize;
                    let z = self.zoom;
                    for di in -3isize..=3isize {
                        let nc = ((col as isize + di) % sim_w + sim_w) % sim_w;
                        let nr = row - 1;
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(origin.x + nc as f32 * z, origin.y + nr as f32 * z),
                            egui::vec2(z, z),
                        );
                        painter.rect_filled(cell_rect, 1.0, egui::Color32::from_rgba_premultiplied(60, 160, 255, 60));
                        painter.rect_stroke(
                            cell_rect, 1.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 160, 255)),
                        );
                    }
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(origin.x + col as f32 * z, origin.y + row as f32 * z),
                        egui::vec2(z, z),
                    );
                    painter.rect_filled(cell_rect, 1.0, egui::Color32::from_rgba_premultiplied(255, 200, 50, 80));
                    painter.rect_stroke(
                        cell_rect, 1.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 50)),
                    );
                }
            }
        });

        if self.rows_done < self.sim_height {
            ctx.request_repaint();
        }
    }
}
