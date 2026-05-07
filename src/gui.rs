use eframe::egui;
use rand::Rng;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use crate::glance_view::{Screen, GalleryState, GlanceAction, enter_glance_view, enter_adjacent_view, draw_gallery};
use crate::palette::{ColorPalette, build_palette, draw_palette_params};
use crate::rule_editor::{self, RandomEditor};
use crate::rule_meta::{draw_rule_meta_params, max_num_states};
use crate::simulation::{SimBatch, noise_from_slider, parse_seed, rule_id_from_lookup, parse_rule_id, random_rule, SimParameters};
#[cfg(target_arch = "wasm32")]
use crate::simulation::BATCH_SIZE;

#[cfg(not(target_arch = "wasm32"))]
use crate::simulation::spawn_sim;
#[cfg(target_arch = "wasm32")]
use crate::simulation::WasmSimRunner;

#[cfg(target_arch = "wasm32")]
fn read_url_hash() -> Option<Rule> {
    use crate::simulation::parse_rule_id;
    let hash = web_sys::window()?.location().hash().ok()?;
    let id = hash.trim_start_matches('#');
    parse_rule_id(id)
}

#[cfg(target_arch = "wasm32")]
fn write_url_hash(rule_id: &str) {
    if let Some(win) = web_sys::window() {
        let _ = win.location().set_hash(rule_id);
    }
}

fn wrapping_idx(i: isize, m: usize) -> usize {
    ((i % m as isize + m as isize) % m as isize) as usize
}



pub struct CellularApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub receiver: mpsc::Receiver<SimBatch>,
    #[cfg(target_arch = "wasm32")]
    pub wasm_runner: Option<WasmSimRunner>,

    pub rows_done: usize,
    pub texture: Option<egui::TextureHandle>,
    pub sim_width: usize,
    pub sim_height: usize,
    pub sim_size: usize,
    pub rule_text: String,
    pub params: SimParameters,
    pub state_palette: Vec<egui::Color32>,
    pub selected_palette: ColorPalette,
    pub show_rule_editor: bool,
    pub seed_text: String,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub view_initialized: bool,
    pub cells_data: Vec<u8>,
    pub highlighted_state: Option<usize>,
    pub highlighted_cell: Option<(usize, usize)>,
    #[cfg(not(target_arch = "wasm32"))]
    pub saved_at: Option<std::time::Instant>,
    pub current_screen: Screen,
    pub glance_state: GalleryState,
    pub adjacent_state: GalleryState,
    pub random_editor: Option<RandomEditor>,
}

impl CellularApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize, initial_rule: Option<&str>) -> Self {
        let mut params = SimParameters {
            rule: random_rule(2, 3, &mut rand::rng()),
            noise: noise_from_slider(0.5),
            seed: rand::rng().random::<u64>(),
        };

        if let Some(parsed) = initial_rule.and_then(|s| parse_rule_id(s)) {
            params.rule = parsed;
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(hash_rule) = read_url_hash() {
            params.rule = hash_rule;
        }

        let rule_text = rule_id_from_lookup(&params.rule);

        #[cfg(not(target_arch = "wasm32"))]
        let receiver = spawn_sim(params.clone(), sim_width, sim_height);

        #[cfg(target_arch = "wasm32")]
        let wasm_runner = Some(WasmSimRunner::new(params.clone(), sim_width, sim_height));

        let state_palette = build_palette(ColorPalette::Classic, params.rule.num_states);

        CellularApp {
            #[cfg(not(target_arch = "wasm32"))]
            receiver,
            #[cfg(target_arch = "wasm32")]
            wasm_runner,

            rows_done: 0,
            texture: None,
            sim_size: sim_width,
            sim_width,
            sim_height,
            rule_text,
            seed_text: params.seed.to_string(),
            params,
            state_palette,
            selected_palette: ColorPalette::Classic,
            show_rule_editor: false,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            view_initialized: false,
            cells_data: vec![0u8; sim_width * sim_height],
            highlighted_state: None,
            highlighted_cell: None,
            #[cfg(not(target_arch = "wasm32"))]
            saved_at: None,
            current_screen: Screen::Main,
            glance_state: GalleryState::new_glance(),
            adjacent_state: GalleryState::new_adjacent(),
            random_editor: None,
        }
    }

    fn start_sim(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.receiver = spawn_sim(self.params.clone(), self.sim_width, self.sim_height);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.wasm_runner = Some(WasmSimRunner::new(self.params.clone(), self.sim_width, self.sim_height));
        }
        self.rows_done = 0;
    }

    pub fn restart_same_rule(&mut self) {
        self.start_sim();
        #[cfg(target_arch = "wasm32")]
        write_url_hash(&self.rule_text);
    }

    pub fn resize_and_restart(&mut self, size: usize) {
        self.sim_size = size;
        self.sim_width = size;
        self.sim_height = size;
        self.texture = None;
        self.cells_data = vec![0u8; size * size];
        self.view_initialized = false;
        self.start_sim();
    }

    pub fn new_rule(&mut self) {
        self.params.rule = random_rule(self.params.rule.num_states, self.params.rule.half_width, &mut rand::rng());
        self.rule_text = rule_id_from_lookup(&self.params.rule);
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn change_num_states(&mut self, new_k: usize) {
        self.state_palette = build_palette(self.selected_palette, new_k);
        self.params.rule = random_rule(new_k, self.params.rule.half_width, &mut rand::rng());
        self.rule_text = rule_id_from_lookup(&self.params.rule);
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn change_half_width(&mut self, new_hw: usize) {
        self.params.rule = random_rule(self.params.rule.num_states, new_hw, &mut rand::rng());
        self.rule_text = rule_id_from_lookup(&self.params.rule);
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn clear_highlight(&mut self) {
        self.highlighted_state = None;
        self.highlighted_cell = None;
    }

    fn rebuild_texture(&mut self, ctx: &egui::Context) {
        if self.rows_done == 0 { return; }
        let pixels: Vec<egui::Color32> = self.cells_data[..self.rows_done * self.sim_width]
            .iter()
            .map(|&v| self.state_palette[v as usize])
            .collect();
        let image = egui::ColorImage { size: [self.sim_width, self.rows_done], pixels };
        match &mut self.texture {
            Some(tex) => { tex.set_partial([0, 0], image, tex_options()); }
            None => {
                let black = egui::ColorImage::new([self.sim_width, self.sim_height], egui::Color32::BLACK);
                let mut tex = ctx.load_texture("sim", black, tex_options());
                tex.set_partial([0, 0], image, tex_options());
                self.texture = Some(tex);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_image(&mut self) {
        use chrono::Local;
        use image::RgbImage;
        use std::fs;
        fs::create_dir_all("output").ok();
        let ts = Local::now().format("%Y-%m-%dT%H-%M-%S");
        let rule_prefix: String = self.rule_text.chars().take(16).collect();
        let path = format!("output/{}-{}_k{}.png", ts, rule_prefix, self.params.rule.num_states);
        let w = self.sim_width as u32;
        let h = self.sim_height as u32;
        let pixels: Vec<u8> = self.cells_data.iter().flat_map(|&v| {
            let c = self.state_palette[v as usize];
            [c.r(), c.g(), c.b()]
        }).collect();
        if let Some(img) = RgbImage::from_raw(w, h, pixels) {
            img.save(&path).ok();
        }
        self.saved_at = Some(std::time::Instant::now());
    }

    fn process_batch(&mut self, ctx: &egui::Context, batch: SimBatch) {
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
}

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        mipmap_mode: Some(egui::TextureFilter::Linear),
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
                GlanceAction::SelectRule(params) => {
                    self.state_palette = build_palette(self.selected_palette, params.rule.num_states);
                    self.rule_text = rule_id_from_lookup(&params.rule);
                    self.seed_text = params.seed.to_string();
                    self.params = params;
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


        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_rule();
        }

        // Poll simulation results (native: drain mpsc channel; wasm: step runner)
        #[cfg(not(target_arch = "wasm32"))]
        while let Ok(batch) = self.receiver.try_recv() {
            self.process_batch(ctx, batch);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let rows_per_frame = (BATCH_SIZE * 10).min(self.sim_height);
            let mut rows_this_frame = 0;
            while rows_this_frame < rows_per_frame {
                let batch = match &mut self.wasm_runner {
                    Some(runner) => runner.step_batch(),
                    None => None,
                };
                match batch {
                    Some(b) => {
                        rows_this_frame += b.pixels.len() / self.sim_width;
                        self.process_batch(ctx, b);
                    }
                    None => break,
                }
            }
        }

        egui::SidePanel::right("controls")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let mut num_states = self.params.rule.num_states;
                    let mut half_width = self.params.rule.half_width;
                    let mut noise = self.params.noise;
                    let meta_resp = draw_rule_meta_params(ui, &mut num_states, &mut half_width, &mut noise, true);
                    if meta_resp.num_states_changed {
                        self.change_num_states(num_states.min(max_num_states(self.params.rule.half_width)));
                    }
                    if meta_resp.half_width_changed {
                        if self.params.rule.num_states > max_num_states(half_width) {
                            self.state_palette = build_palette(self.selected_palette, max_num_states(half_width));
                        }
                        self.change_half_width(half_width);
                    }
                    if meta_resp.noise_changed {
                        self.params.noise = noise;
                        self.restart_same_rule();
                    }

                    ui.horizontal(|ui| {
                        ui.label("Rule ID:");
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.rule_text).desired_width(220.0),
                        );
                        if resp.lost_focus() {
                            if let Some(parsed) = parse_rule_id(&self.rule_text) {
                                self.state_palette = build_palette(self.selected_palette, parsed.num_states);
                                self.params.rule = parsed;
                                self.rule_text = rule_id_from_lookup(&self.params.rule);
                                self.clear_highlight();
                                self.restart_same_rule();
                            } else {
                                self.rule_text = rule_id_from_lookup(&self.params.rule);
                            }
                        }
                    });
                    ui.label(format!("{}/{} rows   zoom: {:.2}x", self.rows_done, self.sim_height, self.zoom));

                    ui.separator();

                    #[cfg(not(target_arch = "wasm32"))]
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
                        self.glance_state.set_num_states(self.params.rule.num_states);
                        self.glance_state.selected_palette = self.selected_palette;
                        self.glance_state.set_palette(self.state_palette.clone());
                        self.glance_state.noise = self.params.noise;
                        enter_glance_view(&mut self.glance_state, self.params.rule.num_states, self.params.rule.half_width);
                        self.current_screen = Screen::Glance;
                    }
                    if ui.button("Explore adjacent rules").clicked() {
                        self.adjacent_state.selected_palette = self.selected_palette;
                        self.adjacent_state.set_palette(self.state_palette.clone());
                        enter_adjacent_view(&mut self.adjacent_state, &self.params);
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
                    if draw_palette_params(ui, &mut self.selected_palette, &mut self.state_palette, self.params.rule.num_states) {
                        self.rebuild_texture(ui.ctx());
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
                        self.params.seed = parse_seed(&self.seed_text);
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

        rule_editor::draw_random_editor(self, ctx);

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
                            let hw = self.params.rule.half_width as isize;
                            for di in -hw..=hw {
                                let nc = wrapping_idx(col as isize + di, self.sim_width);
                                let idx = (row - 1) * self.sim_width + nc;
                                state = state * self.params.rule.num_states + self.cells_data[idx] as usize;
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
                    for di in -(self.params.rule.half_width as isize)..=(self.params.rule.half_width as isize) {
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
