use eframe::egui;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::thread;

struct Looped<'a> {
    slice: &'a [u8],
    len: isize,
}

impl<'a> Looped<'a> {
    fn new(slice: &'a [u8]) -> Self {
        Self { slice, len: slice.len() as isize }
    }

    #[inline]
    fn get(&self, key: isize) -> u8 {
        self.slice[((key % self.len + self.len) % self.len) as usize]
    }
}

fn build_arena(n: usize, options: &[u8], seed: u64) -> Vec<u8> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..n).map(|_| options[rng.random_range(0..options.len())]).collect()
}

fn parse_seed(text: &str) -> u64 {
    if let Ok(n) = text.trim().parse::<u64>() {
        return n;
    }
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

struct Rule {
    lookup: Vec<u8>,
    half_width: isize,
}

impl Rule {
    fn new(rule_num: u128, width: usize) -> Self {
        let state_count = 1 << width;
        let binary = format!("{:064b}", rule_num);
        let rule_chars: Vec<char> = binary.chars().rev().collect();
        let mut lookup = vec![0u8; state_count];
        for i in 0..state_count {
            if i < rule_chars.len() && rule_chars[i] == '1' {
                lookup[i] = 1;
            }
        }
        Self { lookup, half_width: (width / 2) as isize }
    }

    #[inline]
    fn apply(&self, arena: &Looped, i: usize) -> u8 {
        let i_isize = i as isize;
        let mut state = 0usize;
        for di in -self.half_width..=self.half_width {
            state = (state << 1) | (arena.get(i_isize + di) as usize);
        }
        if state < self.lookup.len() { self.lookup[state] } else { 0 }
    }
}

// Writes into `out` in-place; no allocation.
fn apply_step(arena: &[u8], rule: &Rule, out: &mut Vec<u8>) {
    let looped = Looped::new(arena);
    out.resize(arena.len(), 0);
    for i in 0..arena.len() {
        out[i] = rule.apply(&looped, i);
    }
}

fn apply_noise(arena: &mut [u8], noise: f64, rng: &mut SmallRng) {
    // Always consume exactly 2 RNG values per cell so the flip set at any
    // higher noise level is a strict superset of the set at any lower level.
    for cell in arena.iter_mut() {
        let threshold: f64 = rng.random();
        let new_val: u8 = rng.random::<bool>() as u8;
        if threshold <= noise {
            *cell = new_val;
        }
    }
}

// Exponential mapping: s=0 → ~1e-7, s=0.5 → 1e-4, s=1 → 0.1
fn noise_from_slider(s: f64) -> f64 {
    10f64.powf(s * 6.0 - 7.0)
}

fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        ..Default::default()
    }
}

const BATCH_SIZE: usize = 10;

// Flat row-major pixel buffer for a contiguous range of rows.
struct SimBatch {
    start: usize,
    pixels: Vec<u8>, // len == count * sim_width; count derived by receiver
}

fn spawn_sim(
    rule_no: u64,
    sim_width: usize,
    sim_height: usize,
    noise_atomic: Arc<AtomicU64>,
    seed: u64,
) -> mpsc::Receiver<SimBatch> {
    let (tx, rx) = mpsc::channel();
    let rn = rule_no as u128;
    thread::spawn(move || {
        let rule = Rule::new(rn, 6);
        let mut current = build_arena(sim_width, &[0, 1], seed);
        let mut noise_rng = SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15);
        let mut next = vec![0u8; sim_width];

        let mut batch_pixels = Vec::with_capacity(BATCH_SIZE * sim_width);
        let mut batch_start = 0usize;

        // Seed the first batch with row 0 (initial state).
        batch_pixels.extend_from_slice(&current);

        let t0 = std::time::Instant::now();
        for row in 1..sim_height {
            let noise = f64::from_bits(noise_atomic.load(Ordering::Relaxed));
            apply_noise(&mut current, noise, &mut noise_rng);
            apply_step(&current, &rule, &mut next);
            std::mem::swap(&mut current, &mut next);

            batch_pixels.extend_from_slice(&current);

            if batch_pixels.len() == BATCH_SIZE * sim_width {
                let pixels = std::mem::replace(
                    &mut batch_pixels,
                    Vec::with_capacity(BATCH_SIZE * sim_width),
                );
                if tx.send(SimBatch { start: batch_start, pixels }).is_err() {
                    return;
                }
                batch_start = row + 1;
            }
        }
        // Flush any remaining rows (last partial batch).
        if !batch_pixels.is_empty() {
            tx.send(SimBatch { start: batch_start, pixels: batch_pixels }).ok();
        }
        println!("simulation done in {:.2?}", t0.elapsed());
    });
    rx
}

struct CellularApp {
    receiver: mpsc::Receiver<SimBatch>,
    rows_done: usize,
    texture: Option<egui::TextureHandle>,
    sim_width: usize,
    sim_height: usize,
    sim_size: usize,
    rule_no: u64,
    rule_lookup: Vec<u8>,
    show_rule_editor: bool,
    seed: u64,
    seed_text: String,
    zoom: f32,
    pan: egui::Vec2,
    view_initialized: bool,
    noise_slider: f64,
    noise_atomic: Arc<AtomicU64>,
}

impl CellularApp {
    fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize) -> Self {
        let noise_slider = 0.5f64;
        let noise_atomic =
            Arc::new(AtomicU64::new(noise_from_slider(noise_slider).to_bits()));
        let rule_no = rand::rng().random::<u64>();
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
            rule_lookup,
            show_rule_editor: false,
            seed,
            seed_text: seed.to_string(),
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            view_initialized: false,
            noise_slider,
            noise_atomic,
        }
    }

    fn restart_same_rule(&mut self) {
        self.receiver = spawn_sim(
            self.rule_no,
            self.sim_width,
            self.sim_height,
            Arc::clone(&self.noise_atomic),
            self.seed,
        );
        self.rows_done = 0;
    }

    fn resize_and_restart(&mut self, size: usize) {
        self.sim_size = size;
        self.sim_width = size;
        self.sim_height = size;
        self.receiver = spawn_sim(self.rule_no, size, size, Arc::clone(&self.noise_atomic), self.seed);
        self.rows_done = 0;
        self.texture = None;
        self.view_initialized = false;
    }

    fn new_rule(&mut self) {
        self.rule_no = rand::rng().random::<u64>();
        self.rule_lookup = rule_lookup_from_no(self.rule_no);
        self.restart_same_rule();
    }

    fn draw_rule_editor(&mut self, ui: &mut egui::Ui) {
        let cell_sz = 9.0_f32;
        let cell_gap = 1.0_f32;
        let nbr_cells = 7usize;
        let pat_gap = 14.0_f32;
        let out_gap = 4.0_f32;

        let nbr_w = nbr_cells as f32 * cell_sz + (nbr_cells - 1) as f32 * cell_gap;
        let tile_w = nbr_w + pat_gap;
        let tile_h = cell_sz + out_gap + cell_sz;

        ui.label(egui::RichText::new("Rule editor — click an output cell to toggle it").small());
        ui.separator();

        let mut clicked: Option<usize> = None;

        egui::ScrollArea::vertical()
            .id_salt("rule_editor_scroll")
            .max_height(ui.available_height())
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let cols = ((avail_w / tile_w) as usize).max(1).min(64);
                let rows = (64 + cols - 1) / cols;

                for row in 0..rows {
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(avail_w, tile_h),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();

                    for col in 0..cols {
                        let state = row * cols + col;
                        if state >= 64 { break; }

                        let x0 = row_rect.min.x + col as f32 * tile_w;
                        let y0 = row_rect.min.y;

                        // Neighborhood cells (7 cells, MSB first)
                        for bit_pos in 0..nbr_cells {
                            let bit_idx = nbr_cells - 1 - bit_pos;
                            let alive = (state >> bit_idx) & 1 == 1;
                            let x = x0 + bit_pos as f32 * (cell_sz + cell_gap);
                            let r = egui::Rect::from_min_size(
                                egui::pos2(x, y0),
                                egui::vec2(cell_sz, cell_sz),
                            );
                            let fill = if alive { egui::Color32::WHITE } else { egui::Color32::from_gray(35) };
                            painter.rect_filled(r, 1.0, fill);
                            let border = if bit_pos == 3 {
                                // center cell: blue border
                                egui::Color32::from_rgb(80, 130, 220)
                            } else {
                                egui::Color32::from_gray(80)
                            };
                            painter.rect_stroke(r, 1.0, egui::Stroke::new(0.5, border));
                        }

                        // Output cell (centered under center cell, clickable)
                        let out_x = x0 + (nbr_w - cell_sz) / 2.0;
                        let out_y = y0 + cell_sz + out_gap;
                        let out_rect = egui::Rect::from_min_size(
                            egui::pos2(out_x, out_y),
                            egui::vec2(cell_sz, cell_sz),
                        );
                        let output = self.rule_lookup[state];
                        let fill = if output == 1 { egui::Color32::WHITE } else { egui::Color32::from_gray(35) };
                        painter.rect_filled(out_rect, 1.0, fill);
                        painter.rect_stroke(out_rect, 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(140)));

                        let resp = ui.interact(
                            out_rect,
                            egui::Id::new(("rule_out", state)),
                            egui::Sense::click(),
                        );
                        if resp.clicked() {
                            clicked = Some(state);
                        }
                        if resp.hovered() {
                            painter.rect_stroke(
                                out_rect, 1.0,
                                egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 180, 255)),
                            );
                        }
                    }

                    ui.add_space(6.0);
                }
            });

        if let Some(state) = clicked {
            self.rule_lookup[state] = 1 - self.rule_lookup[state];
            self.rule_no = rule_no_from_lookup(&self.rule_lookup);
            self.restart_same_rule();
        }
    }
}

fn rule_lookup_from_no(rule_no: u64) -> Vec<u8> {
    (0..64).map(|i| ((rule_no >> i) & 1) as u8).collect()
}

fn rule_no_from_lookup(lookup: &[u8]) -> u64 {
    lookup.iter().take(64).enumerate().fold(0u64, |acc, (i, &v)| {
        acc | ((v as u64) << i)
    })
}

impl eframe::App for CellularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sync slider → atomic so the running sim thread sees changes immediately.
        self.noise_atomic
            .store(noise_from_slider(self.noise_slider).to_bits(), Ordering::Relaxed);

        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.new_rule();
        }

        loop {
            match self.receiver.try_recv() {
                Ok(batch) => {
                    let count = batch.pixels.len() / self.sim_width;
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

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Rule: {}   {}/{} rows   zoom: {:.2}x",
                    self.rule_no, self.rows_done, self.sim_height, self.zoom
                ));

                if ui.button("New Rule").clicked() {
                    self.new_rule();
                }

                let editor_label = if self.show_rule_editor { "Close Editor" } else { "Edit Rule" };
                if ui.button(editor_label).clicked() {
                    self.show_rule_editor = !self.show_rule_editor;
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
                    // Sync atomic before spawning so the new thread starts with the right value.
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
                    // Normalize display back to the resolved number only if it was a plain number.
                    // Leave arbitrary strings as-is so the user can see what they typed.
                    self.restart_same_rule();
                }
            });
        });

        if self.show_rule_editor {
            egui::TopBottomPanel::bottom("rule_editor")
                .resizable(true)
                .default_height(160.0)
                .show(ctx, |ui| {
                    self.draw_rule_editor(ui);
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
                self.pan = cursor_local + (self.pan - cursor_local) * factor;
                self.zoom = (self.zoom * factor).clamp(0.001, 500.0);
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
        });

        if self.rows_done < self.sim_height {
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("1D Cellular Automata")
            .with_inner_size([1200.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "1D Cellular Automata",
        native_options,
        Box::new(|cc| Ok(Box::new(CellularApp::new(cc, 2000, 2000)))),
    )
}
