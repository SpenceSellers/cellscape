use eframe::egui;
use rand::Rng;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use crate::glance_view::{Screen, GalleryState, GlanceAction, enter_glance_view, enter_adjacent_view, enter_saved_rules_view, draw_gallery};
use crate::palette::{ColorPalette, build_palette, draw_palette_params};
use crate::rule_editor::{self, RandomEditor};
use crate::rule_meta::max_num_states;
use crate::simulation::{
    SimBatch, MixingMode, SimSetup, SimParameters,
    noise_from_slider, parse_seed, params_to_json,
    setup_to_json_display, parse_setup_json, random_rule, cell_rule_index,
    load_saved_rules, persist_saved_rules,
};
#[cfg(target_arch = "wasm32")]
use crate::simulation::BATCH_SIZE;

#[cfg(not(target_arch = "wasm32"))]
use crate::simulation::spawn_sim;
#[cfg(target_arch = "wasm32")]
use crate::simulation::WasmSimRunner;

mod mixing_mode_gui;
mod rule_slots_gui;

#[cfg(not(target_arch = "wasm32"))]
fn scale_mask(img: &image::GrayImage, w: usize, h: usize) -> Vec<u8> {
    use image::imageops;
    let scaled = imageops::resize(img, w as u32, h as u32, imageops::FilterType::Lanczos3);
    scaled.pixels().map(|p| if p[0] >= 128 { 1u8 } else { 0u8 }).collect()
}

#[cfg(target_arch = "wasm32")]
fn read_url_hash() -> Option<SimSetup> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let hash = web_sys::window()?.location().hash().ok()?;
    let id = hash.trim_start_matches('#');
    let json = String::from_utf8(URL_SAFE_NO_PAD.decode(id).ok()?).ok()?;
    parse_setup_json(&json)
}

#[cfg(target_arch = "wasm32")]
fn write_url_hash(s: &str) {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let encoded = URL_SAFE_NO_PAD.encode(s);
    if let Some(win) = web_sys::window() {
        let _ = win.location().set_hash(&encoded);
    }
}

fn wrapping_idx(i: isize, m: usize) -> usize {
    ((i % m as isize + m as isize) % m as isize) as usize
}

pub struct RuleSlot {
    pub text: String,
    pub preview_texture: Option<egui::TextureHandle>,
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

    pub setup: SimSetup,
    pub setup_text: String,
    pub rule_slots: Vec<RuleSlot>,

    pub state_palette: Vec<egui::Color32>,
    pub selected_palette: ColorPalette,
    pub show_rule_editor: bool,
    pub seed_text: String,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub view_initialized: bool,
    pub cells_data: Vec<u8>,
    pub highlighted_state: Option<(usize, usize)>,
    pub highlighted_cell: Option<(usize, usize)>,
    pub editor_active_rule: usize,

    #[cfg(not(target_arch = "wasm32"))]
    pub saved_at: Option<std::time::Instant>,
    pub current_screen: Screen,
    pub glance_state: GalleryState,
    pub adjacent_state: GalleryState,
    pub saved_rules: Vec<SimParameters>,
    pub saved_rules_state: GalleryState,
    pub random_editor: Option<RandomEditor>,

    // Which slot "Load from Saved" was triggered from (None = replace whole setup)
    pub saved_rules_slot: Option<usize>,

    #[cfg(not(target_arch = "wasm32"))]
    pub mask_source: Option<image::GrayImage>,
}

impl CellularApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize, initial_rule: Option<&str>) -> Self {
        let default_params = SimParameters {
            rule: random_rule(2, 3, &mut rand::rng()),
            noise: noise_from_slider(0.5),
            seed: rand::rng().random::<u64>(),
        };
        let mut setup = SimSetup::single(default_params);

        if let Some(parsed) = initial_rule.and_then(|s| parse_setup_json(s)) {
            setup = parsed;
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(hash_setup) = read_url_hash() {
            setup = hash_setup;
        }

        let setup_text = setup_to_json_display(&setup);
        let rule_slots: Vec<RuleSlot> = setup.rules.iter()
            .map(|r| RuleSlot { text: params_to_json(r), preview_texture: None })
            .collect();

        #[cfg(not(target_arch = "wasm32"))]
        let receiver = spawn_sim(setup.clone(), sim_width, sim_height);
        #[cfg(target_arch = "wasm32")]
        let wasm_runner = Some(WasmSimRunner::new(setup.clone(), sim_width, sim_height));

        let max_k = setup.max_num_states();
        let state_palette = build_palette(ColorPalette::Classic, max_k);

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

            seed_text: setup.rules[0].seed.to_string(),
            setup_text,
            rule_slots,
            setup,

            state_palette,
            selected_palette: ColorPalette::Classic,
            show_rule_editor: false,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            view_initialized: false,
            cells_data: vec![0u8; sim_width * sim_height],
            highlighted_state: None,
            highlighted_cell: None,
            editor_active_rule: 0,

            #[cfg(not(target_arch = "wasm32"))]
            saved_at: None,
            current_screen: Screen::Main,
            glance_state: GalleryState::new_glance(),
            adjacent_state: GalleryState::new_adjacent(),
            saved_rules: load_saved_rules(),
            saved_rules_state: GalleryState::new_saved(),
            random_editor: None,
            saved_rules_slot: None,

            #[cfg(not(target_arch = "wasm32"))]
            mask_source: None,
        }
    }

    fn start_sim(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        { self.receiver = spawn_sim(self.setup.clone(), self.sim_width, self.sim_height); }
        #[cfg(target_arch = "wasm32")]
        { self.wasm_runner = Some(WasmSimRunner::new(self.setup.clone(), self.sim_width, self.sim_height)); }
        self.rows_done = 0;
    }

    pub fn restart_same_rule(&mut self) {
        self.start_sim();
        #[cfg(target_arch = "wasm32")]
        write_url_hash(&self.setup_text);
    }

    pub fn resize_and_restart(&mut self, size: usize) {
        self.sim_size = size;
        self.sim_width = size;
        self.sim_height = size;
        self.texture = None;
        self.cells_data = vec![0u8; size * size];
        self.view_initialized = false;
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(ref img), MixingMode::Masked { ref mut mask_data }) =
            (&self.mask_source, &mut self.setup.mode)
        {
            *mask_data = std::sync::Arc::new(scale_mask(img, size, size));
        }
        self.start_sim();
    }

    pub fn push_random_slot(&mut self) {
        let src = &self.setup.rules[0];
        let k = src.rule.num_states;
        let hw = src.rule.half_width;
        self.setup.rules.push(SimParameters {
            rule: random_rule(k, hw, &mut rand::rng()),
            noise: src.noise,
            seed: src.seed,
        });
    }

    pub fn new_rule_for_slot(&mut self, slot: usize) {
        let k = self.setup.rules[slot].rule.num_states;
        let hw = self.setup.rules[slot].rule.half_width;
        self.setup.rules[slot].rule = random_rule(k, hw, &mut rand::rng());
        self.sync_texts();
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn change_num_states_for_slot(&mut self, slot: usize, new_k: usize) {
        let clamped = new_k.min(max_num_states(self.setup.rules[slot].rule.half_width));
        self.setup.rules[slot].rule = random_rule(clamped, self.setup.rules[slot].rule.half_width, &mut rand::rng());
        self.state_palette = build_palette(self.selected_palette, self.setup.max_num_states());
        self.sync_texts();
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn change_half_width_for_slot(&mut self, slot: usize, new_hw: usize) {
        let k = self.setup.rules[slot].rule.num_states;
        if k > max_num_states(new_hw) {
            self.state_palette = build_palette(self.selected_palette, self.setup.max_num_states());
        }
        self.setup.rules[slot].rule = random_rule(k, new_hw, &mut rand::rng());
        self.sync_texts();
        self.clear_highlight();
        self.restart_same_rule();
    }

    pub fn clear_highlight(&mut self) {
        self.highlighted_state = None;
        self.highlighted_cell = None;
    }

    pub fn sync_texts(&mut self) {
        self.setup_text = setup_to_json_display(&self.setup);
        self.rule_slots.clear();
        for r in &self.setup.rules {
            self.rule_slots.push(RuleSlot { text: params_to_json(r), preview_texture: None });
        }
    }

    pub fn sync_slot_texts(&mut self) {
        self.sync_texts();
    }

    fn rebuild_texture(&mut self, ctx: &egui::Context) {
        if self.rows_done == 0 { return; }
        let image = crate::texture::cells_to_color_image(
            &self.cells_data[..self.rows_done * self.sim_width],
            self.sim_width, self.rows_done, &self.state_palette,
        );
        match &mut self.texture {
            Some(tex) => { tex.set_partial([0, 0], image, crate::texture::tex_options()); }
            None => {
                let black = egui::ColorImage::new([self.sim_width, self.sim_height], egui::Color32::BLACK);
                let mut tex = ctx.load_texture("sim", black, crate::texture::tex_options());
                tex.set_partial([0, 0], image, crate::texture::tex_options());
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
        let prefix: String = self.setup_text.chars().take(16).collect();
        let max_k = self.setup.max_num_states();
        let path = format!("output/{}-{}_k{}.png", ts, prefix, max_k);
        let w = self.sim_width as u32;
        let h = self.sim_height as u32;
        let pixels: Vec<u8> = self.cells_data.iter().flat_map(|&v| {
            let c = self.state_palette[v as usize % self.state_palette.len()];
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
        let partial = crate::texture::cells_to_color_image(
            &batch.pixels, self.sim_width, count, &self.state_palette,
        );
        match &mut self.texture {
            Some(tex) => { tex.set_partial([0, batch.start], partial, crate::texture::tex_options()); }
            None => {
                let black = egui::ColorImage::new([self.sim_width, self.sim_height], egui::Color32::BLACK);
                let mut tex = ctx.load_texture("sim", black, crate::texture::tex_options());
                tex.set_partial([0, batch.start], partial, crate::texture::tex_options());
                self.texture = Some(tex);
            }
        }
        self.rows_done = self.rows_done.max(batch.start + count);
    }
}


impl eframe::App for CellularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.current_screen != Screen::Main {
            let action = match self.current_screen {
                Screen::Glance => draw_gallery(&mut self.glance_state, ctx),
                Screen::Adjacent => draw_gallery(&mut self.adjacent_state, ctx),
                Screen::SavedRules => draw_gallery(&mut self.saved_rules_state, ctx),
                Screen::Main => unreachable!(),
            };
            match action {
                GlanceAction::SelectRule(params) => {
                    if let Some(slot) = self.saved_rules_slot {
                        // Load into a specific slot
                        self.setup.rules[slot] = params;
                        self.state_palette = build_palette(self.selected_palette, self.setup.max_num_states());
                    } else {
                        // Replace entire setup
                        self.state_palette = build_palette(self.selected_palette, params.rule.num_states);
                        self.seed_text = params.seed.to_string();
                        self.setup = SimSetup::single(params);
                        self.editor_active_rule = 0;
                    }
                    self.sync_texts();
                    self.clear_highlight();
                    self.restart_same_rule();
                    self.current_screen = Screen::Main;
                    self.saved_rules_slot = None;
                }
                GlanceAction::DeleteRule(idx) => {
                    if idx < self.saved_rules.len() {
                        self.saved_rules.remove(idx);
                        persist_saved_rules(&self.saved_rules);
                        enter_saved_rules_view(&mut self.saved_rules_state, &self.saved_rules);
                    }
                }
                GlanceAction::Back => {
                    self.current_screen = Screen::Main;
                    self.saved_rules_slot = None;
                }
                GlanceAction::None => {}
            }
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_rule_for_slot(self.editor_active_rule);
        }

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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    draw_sidebar(self, ui);
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
                let cursor = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(canvas.center());
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
                    if col < self.sim_width && row < self.sim_height && row > 0 {
                        let rule_idx = cell_rule_index(&self.setup, col, row, self.sim_width, self.sim_height);
                        let rule = &self.setup.rules[rule_idx].rule;
                        let mut state = 0usize;
                        let hw = rule.half_width as isize;
                        for di in -hw..=hw {
                            let nc = wrapping_idx(col as isize + di, self.sim_width);
                            let idx = (row - 1) * self.sim_width + nc;
                            state = state * rule.num_states + (self.cells_data[idx] as usize % rule.num_states);
                        }
                        self.highlighted_state = Some((rule_idx, state));
                        self.highlighted_cell = Some((col, row));
                        self.editor_active_rule = rule_idx;
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
                    let rule_idx = cell_rule_index(&self.setup, col, row, self.sim_width, self.sim_height);
                    let hw = self.setup.rules[rule_idx].rule.half_width;
                    for di in -(hw as isize)..=(hw as isize) {
                        let nc = wrapping_idx(col as isize + di, self.sim_width);
                        let nr = row - 1;
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(origin.x + nc as f32 * z, origin.y + nr as f32 * z),
                            egui::vec2(z, z),
                        );
                        painter.rect_filled(cell_rect, 1.0, egui::Color32::from_rgba_premultiplied(60, 160, 255, 60));
                        painter.rect_stroke(cell_rect, 1.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 160, 255)));
                    }
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(origin.x + col as f32 * z, origin.y + row as f32 * z),
                        egui::vec2(z, z),
                    );
                    painter.rect_filled(cell_rect, 1.0, egui::Color32::from_rgba_premultiplied(255, 200, 50, 80));
                    painter.rect_stroke(cell_rect, 1.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 50)));
                }
            }
        });

        if self.rows_done < self.sim_height {
            ctx.request_repaint();
        }
    }
}

fn draw_sidebar(app: &mut CellularApp, ui: &mut egui::Ui) {
    mixing_mode_gui::draw_mixing_mode(app, ui);

    ui.separator();
    rule_slots_gui::draw_rule_slots(app, ui);
    ui.separator();

    ui.label(format!("{}/{} rows   zoom: {:.2}x", app.rows_done, app.sim_height, app.zoom));

    ui.separator();

    #[cfg(not(target_arch = "wasm32"))]
    ui.horizontal(|ui| {
        if ui.button("Save PNG").clicked() {
            app.save_image();
        }
        if let Some(t) = app.saved_at {
            if t.elapsed() < std::time::Duration::from_secs(2) {
                ui.label("Saved!");
            } else {
                app.saved_at = None;
            }
        }
    });

    ui.separator();
    if ui.button(format!("Saved Rules ({})", app.saved_rules.len())).clicked()
        && !app.saved_rules.is_empty()
    {
        app.saved_rules_slot = None;
        app.saved_rules_state.selected_palette = app.selected_palette;
        app.saved_rules_state.set_palette(app.state_palette.clone());
        enter_saved_rules_view(&mut app.saved_rules_state, &app.saved_rules);
        app.current_screen = Screen::SavedRules;
    }

    if ui.button("Explore random rules").clicked() {
        let slot = app.editor_active_rule;
        app.saved_rules_slot = None;
        app.glance_state.set_num_states(app.setup.rules[slot].rule.num_states);
        app.glance_state.selected_palette = app.selected_palette;
        app.glance_state.set_palette(app.state_palette.clone());
        app.glance_state.noise = app.setup.rules[slot].noise;
        enter_glance_view(&mut app.glance_state, app.setup.rules[slot].rule.num_states, app.setup.rules[slot].rule.half_width);
        app.current_screen = Screen::Glance;
    }
    if ui.button("Explore adjacent rules").clicked() {
        let slot = app.editor_active_rule;
        app.saved_rules_slot = None;
        app.adjacent_state.selected_palette = app.selected_palette;
        app.adjacent_state.set_palette(app.state_palette.clone());
        enter_adjacent_view(&mut app.adjacent_state, &app.setup.rules[slot]);
        app.current_screen = Screen::Adjacent;
    }

    ui.separator();

    let editor_label = if app.show_rule_editor { "Close Editor" } else { "Edit Rule" };
    if ui.button(editor_label).clicked() {
        app.show_rule_editor = !app.show_rule_editor;
        if !app.show_rule_editor {
            app.clear_highlight();
        }
    }

    ui.separator();
    if draw_palette_params(ui, &mut app.selected_palette, &mut app.state_palette, app.setup.max_num_states()) {
        app.rebuild_texture(ui.ctx());
        app.rule_slots.iter_mut().for_each(|s| s.preview_texture = None);
    }

    ui.separator();
    ui.label("Size:");
    let size_resp = ui.add(egui::Slider::new(&mut app.sim_size, 100..=16000).suffix("px").integer());
    if size_resp.drag_stopped() || size_resp.lost_focus() {
        let new_size = app.sim_size;
        app.resize_and_restart(new_size);
    }

    ui.separator();
    ui.label("Seed:");
    let seed_resp = ui.add(egui::TextEdit::singleline(&mut app.seed_text).desired_width(140.0));
    if seed_resp.lost_focus() {
        app.setup.rules[0].seed = parse_seed(&app.seed_text);
        app.sync_texts();
        app.restart_same_rule();
    }
}
