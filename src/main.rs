use eframe::egui;
use rand::Rng;
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

fn build_arena(n: usize, options: &[u8]) -> Vec<u8> {
    let mut rng = rand::rng();
    (0..n).map(|_| options[rng.random_range(0..options.len())]).collect()
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

fn apply_noise(arena: &mut [u8], noise: f64) {
    if noise <= 0.0 {
        return;
    }
    let mut rng = rand::rng();
    for cell in arena.iter_mut() {
        if rng.random::<f64>() <= noise {
            *cell = if rng.random::<bool>() { 1 } else { 0 };
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

struct SimRow {
    index: usize,
    pixels: Vec<u8>,
}

fn spawn_sim(
    rule_no: u64,
    sim_width: usize,
    sim_height: usize,
    noise_atomic: Arc<AtomicU64>,
) -> mpsc::Receiver<SimRow> {
    let (tx, rx) = mpsc::channel();
    let rn = rule_no as u128;
    thread::spawn(move || {
        let rule = Rule::new(rn, 6);
        let mut current = build_arena(sim_width, &[0, 1]);
        let mut next = vec![0u8; sim_width];

        if tx.send(SimRow { index: 0, pixels: current.clone() }).is_err() {
            return;
        }
        let t0 = std::time::Instant::now();
        for row in 1..sim_height {
            let noise = f64::from_bits(noise_atomic.load(Ordering::Relaxed));
            apply_noise(&mut current, noise);
            // Write new state into `next` — no allocation inside apply_step.
            apply_step(&current, &rule, &mut next);
            // Swap: `current` becomes the new state, `next` becomes free scratch.
            std::mem::swap(&mut current, &mut next);
            // `current` is the row we want to display; clone it for the channel.
            // One allocation per step (4 kB), down from three.
            if tx.send(SimRow { index: row, pixels: current.clone() }).is_err() {
                return;
            }
        }
        println!("simulation done in {:.2?}", t0.elapsed());
    });
    rx
}

struct CellularApp {
    receiver: mpsc::Receiver<SimRow>,
    image_buffer: Vec<egui::Color32>,
    rows_done: usize,
    texture: Option<egui::TextureHandle>,
    sim_width: usize,
    sim_height: usize,
    rule_no: u64,
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
        let receiver =
            spawn_sim(rule_no, sim_width, sim_height, Arc::clone(&noise_atomic));

        CellularApp {
            receiver,
            image_buffer: vec![egui::Color32::BLACK; sim_width * sim_height],
            rows_done: 0,
            texture: None,
            sim_width,
            sim_height,
            rule_no,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            view_initialized: false,
            noise_slider,
            noise_atomic,
        }
    }

    fn new_rule(&mut self) {
        self.rule_no = rand::rng().random::<u64>();
        self.receiver = spawn_sim(
            self.rule_no,
            self.sim_width,
            self.sim_height,
            Arc::clone(&self.noise_atomic),
        );
        self.image_buffer.fill(egui::Color32::BLACK);
        self.rows_done = 0;
        self.texture = None;
        self.view_initialized = false;
    }
}

impl eframe::App for CellularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sync slider → atomic so the running sim thread sees changes immediately.
        self.noise_atomic
            .store(noise_from_slider(self.noise_slider).to_bits(), Ordering::Relaxed);

        let prev_rows = self.rows_done;
        loop {
            match self.receiver.try_recv() {
                Ok(msg) => {
                    let offset = msg.index * self.sim_width;
                    for (col, &val) in msg.pixels.iter().enumerate() {
                        self.image_buffer[offset + col] =
                            egui::Color32::from_gray(val.saturating_mul(255));
                    }
                    if msg.index + 1 > self.rows_done {
                        self.rows_done = msg.index + 1;
                    }
                }
                Err(_) => break,
            }
        }

        if self.rows_done > prev_rows {
            let h = self.rows_done;
            let pixels = self.image_buffer[..self.sim_width * h].to_vec();
            let image = egui::ColorImage { size: [self.sim_width, h], pixels };
            match &mut self.texture {
                Some(tex) => tex.set(image, tex_options()),
                None => {
                    self.texture = Some(ctx.load_texture("sim", image, tex_options()));
                }
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

                ui.separator();
                ui.label("Noise:");
                ui.add(
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
            });
        });

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

            if let Some(tex) = &self.texture {
                let img_w = self.sim_width as f32 * self.zoom;
                let img_h = self.rows_done as f32 * self.zoom;
                let origin = (canvas.min.to_vec2() + self.pan).to_pos2();
                let rect = egui::Rect::from_min_size(origin, egui::vec2(img_w, img_h));
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter_at(canvas).image(tex.id(), rect, uv, egui::Color32::WHITE);
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
        Box::new(|cc| Ok(Box::new(CellularApp::new(cc, 4000, 4000)))),
    )
}
