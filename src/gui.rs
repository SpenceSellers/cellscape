use eframe::egui;
use rand::Rng;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Instant;

use crate::glance_view::{Screen, GalleryState, GlanceAction, enter_glance_view, enter_adjacent_view, draw_gallery};
use crate::rule_editor;
use crate::simulation::{spawn_sim, SimBatch, noise_from_slider, parse_seed, rule_string_from_lookup, rule_lookup_from_string, random_rule_lookup};

fn wrapping_idx(i: isize, m: usize) -> usize {
    ((i % m as isize + m as isize) % m as isize) as usize
}

pub fn build_palette(num_states: usize) -> Vec<egui::Color32> {
    let colors: &[egui::Color32] = &[
        egui::Color32::BLACK,
        egui::Color32::WHITE,
        egui::Color32::from_rgb(200, 50, 50),
        egui::Color32::from_rgb(60, 100, 220),
        egui::Color32::from_rgb(50, 180, 80),
        egui::Color32::from_rgb(220, 180, 40),
        egui::Color32::from_rgb(50, 200, 200),
        egui::Color32::from_rgb(200, 80, 200),
    ];
    colors[..num_states.min(colors.len())].to_vec()
}


pub struct CellularApp {
    pub receiver: mpsc::Receiver<SimBatch>,
    pub rows_done: usize,
    pub texture: Option<egui::TextureHandle>,
    pub sim_width: usize,
    pub sim_height: usize,
    pub sim_size: usize,
    pub num_states: usize,
    pub rule_text: String,
    pub rule_lookup: Vec<u8>,
    pub state_palette: Vec<egui::Color32>,
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
    pub current_screen: Screen,
    pub glance_state: GalleryState,
    pub adjacent_state: GalleryState,
}

impl CellularApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize) -> Self {
        let noise_slider = 0.5f64;
        let noise_atomic =
            Arc::new(AtomicU64::new(noise_from_slider(noise_slider).to_bits()));
        let num_states = 2usize;
        let rule_lookup = random_rule_lookup(num_states, &mut rand::rng());
        let rule_text = rule_string_from_lookup(&rule_lookup);
        let seed = rand::rng().random::<u64>();
        let receiver =
            spawn_sim(rule_lookup.clone(), num_states, sim_width, sim_height, Arc::clone(&noise_atomic), seed);
        let state_palette = build_palette(num_states);

        CellularApp {
            receiver,
            rows_done: 0,
            texture: None,
            sim_size: sim_width,
            sim_width,
            sim_height,
            num_states,
            rule_text,
            rule_lookup,
            state_palette,
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
            current_screen: Screen::Main,
            glance_state: GalleryState::new_glance(),
            adjacent_state: GalleryState::new_adjacent(),
        }
    }

    pub fn restart_same_rule(&mut self) {
        self.receiver = spawn_sim(
            self.rule_lookup.clone(),
            self.num_states,
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
        self.receiver = spawn_sim(self.rule_lookup.clone(), self.num_states, size, size, Arc::clone(&self.noise_atomic), self.seed);
        self.rows_done = 0;
        self.texture = None;
        self.cells_data = vec![0u8; size * size];
        self.view_initialized = false;
    }

    pub fn new_rule(&mut self) {
        self.rule_lookup = random_rule_lookup(self.num_states, &mut rand::rng());
        self.rule_text = rule_string_from_lookup(&self.rule_lookup);
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn change_num_states(&mut self, new_k: usize) {
        self.num_states = new_k;
        self.state_palette = build_palette(new_k);
        self.rule_lookup = random_rule_lookup(new_k, &mut rand::rng());
        self.rule_text = rule_string_from_lookup(&self.rule_lookup);
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn clear_highlight(&mut self) {
        self.highlighted_state = None;
        self.highlighted_cell = None;
    }

    fn save_image(&mut self) {
        use chrono::Local;
        use image::RgbImage;
        use std::fs;
        fs::create_dir_all("output").ok();
        let ts = Local::now().format("%Y-%m-%dT%H-%M-%S");
        let rule_prefix: String = self.rule_text.chars().take(16).collect();
        let path = format!("output/{}-{}_k{}.png", ts, rule_prefix, self.num_states);
        let w = self.sim_width as u32;
        let h = self.sim_height as u32;
        let pixels: Vec<u8> = self.cells_data.iter().flat_map(|&v| {
            let c = self.state_palette[v as usize];
            [c.r(), c.g(), c.b()]
        }).collect();
        if let Some(img) = RgbImage::from_raw(w, h, pixels) {
            img.save(&path).ok();
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
        if self.current_screen != Screen::Main {
            let action = match self.current_screen {
                Screen::Glance => draw_gallery(&mut self.glance_state, ctx),
                Screen::Adjacent => draw_gallery(&mut self.adjacent_state, ctx),
                Screen::Main => unreachable!(),
            };
            match action {
                GlanceAction::SelectRule(lookup, num_states, seed) => {
                    self.rule_lookup = lookup;
                    self.num_states = num_states;
                    self.state_palette = build_palette(num_states);
                    self.rule_text = rule_string_from_lookup(&self.rule_lookup);
                    self.seed = seed;
                    self.seed_text = seed.to_string();
                    self.clear_highlight();
                    self.restart_same_rule();
                    self.current_screen = Screen::Main;
                }
                GlanceAction::Back => {
                    self.current_screen = Screen::Main;
                }
                GlanceAction::None => {}
            }
            return;
        }

        self.noise_atomic
            .store(noise_from_slider(self.noise_slider).to_bits(), Ordering::Relaxed);

        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_rule();
        }

        while let Ok(batch) = self.receiver.try_recv() {
            let count = batch.pixels.len() / self.sim_width;
            for (i, &v) in batch.pixels.iter().enumerate() {
                let row = batch.start + i / self.sim_width;
                let col = i % self.sim_width;
                self.cells_data[row * self.sim_width + col] = v;
            }
            let pixels: Vec<egui::Color32> = batch
                .pixels
                .iter()
                .map(|&v| self.state_palette[v as usize])
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

        egui::SidePanel::right("controls")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("States:");
                    let states_resp = ui.add(
                        egui::Slider::new(&mut self.num_states, 2..=8).integer(),
                    );
                    if states_resp.changed() {
                        self.change_num_states(self.num_states);
                    }

                    ui.horizontal(|ui| {
                        ui.label("Rule:");
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.rule_text).desired_width(220.0),
                        );
                        if resp.lost_focus() {
                            if let Some(lookup) = rule_lookup_from_string(&self.rule_text, self.num_states) {
                                self.rule_lookup = lookup;
                                self.clear_highlight();
                                self.restart_same_rule();
                            } else {
                                self.rule_text = rule_string_from_lookup(&self.rule_lookup);
                            }
                        }
                    });
                    ui.label(format!("{}/{} rows   zoom: {:.2}x", self.rows_done, self.sim_height, self.zoom));

                    ui.separator();

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

                    ui.separator();
                    if ui.button("Explore random rules").clicked() {
                        self.glance_state.set_num_states(self.num_states);
                        enter_glance_view(&mut self.glance_state, self.num_states);
                        self.current_screen = Screen::Glance;
                    }
                    if ui.button("Explore adjacent rules").clicked() {
                        enter_adjacent_view(&mut self.adjacent_state, &self.rule_lookup, self.num_states, self.seed);
                        self.current_screen = Screen::Adjacent;
                    }
                    ui.separator();

                    let editor_label = if self.show_rule_editor { "Close Editor" } else { "Edit Rule" };
                    if ui.button(editor_label).clicked() {
                        self.show_rule_editor = !self.show_rule_editor;
                        if !self.show_rule_editor {
                            self.clear_highlight();
                        }
                    }

                    ui.separator();
                    ui.label("Noise:");
                    let noise_resp = ui.add(
                        egui::Slider::new(&mut self.noise_slider, 0.0f64..=1.0)
                            .custom_formatter(|v, _| {
                            let n = noise_from_slider(v);
                            if n == 0.0 { "0".to_string() } else { format!("{:.2e}", n) }
                        })
                            .custom_parser(|s| {
                                s.parse::<f64>().ok().map(|noise| {
                                    if noise > 0.0 {
                                        ((noise.log10() + 7.0) / 6.0).clamp(0.0, 1.0)
                                    } else {
                                        0.0
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
                            for di in -3isize..=3isize {
                                let nc = wrapping_idx(col as isize + di, self.sim_width);
                                let idx = (row - 1) * self.sim_width + nc;
                                state = state * self.num_states + self.cells_data[idx] as usize;
                            }
                            self.highlighted_state = Some(state);
                            self.highlighted_cell = Some((col, row));
                        } else {
                            self.clear_highlight();
                        }
                    } else {
                        self.clear_highlight();
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
                    let z = self.zoom;
                    for di in -3isize..=3isize {
                        let nc = wrapping_idx(col as isize + di, self.sim_width);
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
