use eframe::egui;
use rand::Rng;
use std::sync::mpsc;
use std::thread;

struct Looped {
    collection: Vec<u8>,
    len: isize,
}

impl Looped {
    fn new(collection: Vec<u8>) -> Self {
        let len = collection.len() as isize;
        Self { collection, len }
    }

    #[inline]
    fn get(&self, key: isize) -> u8 {
        let adjusted_key = ((key % self.len + self.len) % self.len) as usize;
        self.collection[adjusted_key]
    }
}

fn build_arena(n: usize, options: &[u8]) -> Vec<u8> {
    let mut rng = rand::rng();
    (0..n).map(|_| options[rng.random_range(0..options.len())]).collect()
}

struct Rule {
    lookup: Vec<u8>,
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
        Self { lookup }
    }

    fn apply(&self, arena: &Looped, i: usize, width: usize) -> u8 {
        let n = width / 2;
        let i_isize = i as isize;
        let mut state = 0usize;
        for di in -(n as isize)..=(n as isize) {
            state = (state << 1) | (arena.get(i_isize + di) as usize);
        }
        if state < self.lookup.len() {
            self.lookup[state]
        } else {
            0
        }
    }
}

fn apply_step(arena: &[u8], rule: &Rule, width: usize) -> Vec<u8> {
    let looped = Looped::new(arena.to_vec());
    (0..arena.len()).map(|i| rule.apply(&looped, i, width)).collect()
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

struct SimRow {
    index: usize,
    pixels: Vec<u8>,
}

struct CellularApp {
    receiver: mpsc::Receiver<SimRow>,
    image_buffer: Vec<egui::Color32>,
    rows_done: usize,
    texture: Option<egui::TextureHandle>,
    sim_width: usize,
    sim_height: usize,
    rule_no: u64,
}

impl CellularApp {
    fn new(_cc: &eframe::CreationContext<'_>, sim_width: usize, sim_height: usize) -> Self {
        let (tx, rx) = mpsc::channel::<SimRow>();

        let rule_no = rand::rng().random::<u64>();
        let rn = rule_no as u128;

        thread::spawn(move || {
            let rule = Rule::new(rn, 6);
            let mut arena = build_arena(sim_width, &[0, 1]);

            if tx.send(SimRow { index: 0, pixels: arena.clone() }).is_err() {
                return;
            }

            for row in 1..sim_height {
                apply_noise(&mut arena, 0.0001);
                arena = apply_step(&arena, &rule, 6);
                if tx.send(SimRow { index: row, pixels: arena.clone() }).is_err() {
                    return;
                }
            }
        });

        CellularApp {
            receiver: rx,
            image_buffer: vec![egui::Color32::BLACK; sim_width * sim_height],
            rows_done: 0,
            texture: None,
            sim_width,
            sim_height,
            rule_no,
        }
    }
}

impl eframe::App for CellularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            let image = egui::ColorImage {
                size: [self.sim_width, h],
                pixels,
            };
            match &mut self.texture {
                Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
                None => {
                    self.texture =
                        Some(ctx.load_texture("sim", image, egui::TextureOptions::NEAREST));
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!(
                "Rule: {}   {}/{} rows",
                self.rule_no, self.rows_done, self.sim_height
            ));

            if let Some(tex) = &self.texture {
                let available = ui.available_size();
                let img_w = self.sim_width as f32;
                let img_h = self.rows_done as f32;
                let scale = (available.x / img_w).min(available.y / img_h);
                let display = egui::vec2(img_w * scale, img_h * scale);
                ui.image((tex.id(), display));
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
