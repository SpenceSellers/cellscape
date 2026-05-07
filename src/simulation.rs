use rand::{Rng, SeedableRng, rngs::SmallRng};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, atomic::{AtomicU64, Ordering}, mpsc};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

pub struct Looped<'a> {
    slice: &'a [u8],
    len: isize,
}

impl<'a> Looped<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice, len: slice.len() as isize }
    }

    #[inline]
    pub fn get(&self, key: isize) -> u8 {
        self.slice[((key % self.len + self.len) % self.len) as usize]
    }
}

pub fn build_arena(n: usize, options: &[u8], seed: u64) -> Vec<u8> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..n).map(|_| options[rng.random_range(0..options.len())]).collect()
}

pub fn all_states(num_states: usize) -> Vec<u8> {
    (0..num_states).map(|i| i as u8).collect()
}

pub struct Rule {
    lookup: Vec<u8>,
    half_width: isize,
    num_states: usize,
}

impl Rule {
    pub fn from_lookup(lookup: Vec<u8>, num_states: usize, half_width: usize) -> Self {
        Self { lookup, half_width: half_width as isize, num_states }
    }

    #[inline]
    fn apply(&self, arena: &Looped, i: usize) -> u8 {
        let i_isize = i as isize;
        let mut state = 0usize;
        for di in -self.half_width..=self.half_width {
            state = state * self.num_states + arena.get(i_isize + di) as usize;
        }
        self.lookup[state]
    }
}

pub fn apply_step(arena: &[u8], rule: &Rule, out: &mut Vec<u8>) {
    let looped = Looped::new(arena);
    out.resize(arena.len(), 0);
    for (i, cell) in out.iter_mut().enumerate() {
        *cell = rule.apply(&looped, i);
    }
}

pub fn apply_noise(arena: &mut [u8], noise: f64, num_states: usize, rng: &mut SmallRng) {
    for cell in arena.iter_mut() {
        if rng.random::<f64>() <= noise {
            *cell = rng.random_range(0..num_states as u8);
        }
    }
}

pub fn noise_from_slider(s: f64) -> f64 {
    if s <= 0.0 { 0.0 } else { 10f64.powf(s * 6.0 - 7.0) }
}

pub const BATCH_SIZE: usize = 10;

pub struct SimBatch {
    pub start: usize,
    pub pixels: Vec<u8>,
}

pub struct SimRunner {
    rule: Rule,
    num_states: usize,
    current: Vec<u8>,
    noise_rng: SmallRng,
    next: Vec<u8>,
    next_output_row: usize,
    sim_width: usize,
    sim_height: usize,
}

impl SimRunner {
    pub fn new(lookup: Vec<u8>, num_states: usize, half_width: usize, sim_width: usize, sim_height: usize, seed: u64) -> Self {
        Self {
            rule: Rule::from_lookup(lookup, num_states, half_width),
            current: build_arena(sim_width, &all_states(num_states), seed),
            noise_rng: SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15),
            next: vec![0u8; sim_width],
            next_output_row: 0,
            num_states,
            sim_width,
            sim_height,
        }
    }

    pub fn step_batch(&mut self, noise: f64) -> Option<SimBatch> {
        if self.next_output_row >= self.sim_height {
            return None;
        }
        let start = self.next_output_row;
        let mut pixels = Vec::with_capacity(BATCH_SIZE * self.sim_width);
        if start == 0 {
            pixels.extend_from_slice(&self.current);
            self.next_output_row = 1;
        }
        while pixels.len() < BATCH_SIZE * self.sim_width && self.next_output_row < self.sim_height {
            apply_noise(&mut self.current, noise, self.num_states, &mut self.noise_rng);
            apply_step(&self.current, &self.rule, &mut self.next);
            std::mem::swap(&mut self.current, &mut self.next);
            pixels.extend_from_slice(&self.current);
            self.next_output_row += 1;
        }
        Some(SimBatch { start, pixels })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_sim(
    lookup: Vec<u8>,
    num_states: usize,
    half_width: usize,
    sim_width: usize,
    sim_height: usize,
    noise_atomic: Arc<AtomicU64>,
    seed: u64,
) -> mpsc::Receiver<SimBatch> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut runner = SimRunner::new(lookup, num_states, half_width, sim_width, sim_height, seed);
        let t0 = std::time::Instant::now();
        loop {
            let noise = f64::from_bits(noise_atomic.load(Ordering::Relaxed));
            match runner.step_batch(noise) {
                Some(batch) => { if tx.send(batch).is_err() { return; } }
                None => break,
            }
        }
        println!("simulation done in {:.2?}", t0.elapsed());
    });
    rx
}

#[cfg(target_arch = "wasm32")]
pub struct WasmSimRunner {
    runner: SimRunner,
    pub noise: f64,
}

#[cfg(target_arch = "wasm32")]
impl WasmSimRunner {
    pub fn new(
        lookup: Vec<u8>,
        num_states: usize,
        half_width: usize,
        sim_width: usize,
        sim_height: usize,
        noise: f64,
        seed: u64,
    ) -> Self {
        Self { runner: SimRunner::new(lookup, num_states, half_width, sim_width, sim_height, seed), noise }
    }

    pub fn step_batch(&mut self) -> Option<SimBatch> {
        self.runner.step_batch(self.noise)
    }
}

pub fn compute_sim(
    lookup: &[u8],
    num_states: usize,
    half_width: usize,
    sim_width: usize,
    sim_height: usize,
    noise: f64,
    seed: u64,
    prerun: usize,
) -> Vec<u8> {
    let rule = Rule::from_lookup(lookup.to_vec(), num_states, half_width);
    let mut current = build_arena(sim_width, &all_states(num_states), seed);
    let mut noise_rng = SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15);
    let mut next = vec![0u8; sim_width];
    let mut result = Vec::with_capacity(sim_width * sim_height);
    result.extend_from_slice(&current);
    for i in 1..(sim_height + prerun) {
        apply_noise(&mut current, noise, num_states, &mut noise_rng);
        apply_step(&current, &rule, &mut next);
        std::mem::swap(&mut current, &mut next);
        if i > prerun {
            result.extend_from_slice(&current);
        }
    }
    result
}

pub fn rule_string_from_lookup(lookup: &[u8]) -> String {
    lookup.iter().map(|&v| char::from_digit(v as u32, 10).unwrap()).collect()
}

pub fn rule_lookup_from_string(s: &str, num_states: usize, half_width: usize) -> Option<Vec<u8>> {
    let width = 2 * half_width + 1;
    let expected_len = num_states.pow(width as u32);
    if s.len() != expected_len { return None; }
    s.chars()
        .map(|c| c.to_digit(10).and_then(|d| if (d as usize) < num_states { Some(d as u8) } else { None }))
        .collect()
}

pub fn random_rule_lookup(num_states: usize, half_width: usize, rng: &mut impl Rng) -> Vec<u8> {
    let width = 2 * half_width + 1;
    (0..num_states.pow(width as u32)).map(|_| rng.random_range(0..num_states as u8)).collect()
}

pub fn parse_seed(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    if let Ok(n) = text.trim().parse::<u64>() {
        return n;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
