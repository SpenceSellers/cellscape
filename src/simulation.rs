use rand::{Rng, SeedableRng, rngs::SmallRng};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

mod cell_source;
pub use cell_source::CellSource;

mod rule_io;
pub use rule_io::{rule_string_from_lookup, rule_id_from_lookup, parse_rule_id,
                  params_to_json, parse_params_json, setup_to_json, parse_setup_json};

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

#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    #[serde(rename="l")]
    pub lookup: Vec<CellSource>,
    #[serde(rename="w")]
    pub half_width: usize,
    #[serde(rename="s")]
    pub num_states: usize,
}

impl Rule {
    pub fn new(lookup: Vec<CellSource>, num_states: usize, half_width: usize) -> Self {
        Self { lookup, half_width, num_states }
    }

    #[inline]
    fn apply(&self, arena: &Looped, i: usize, rng: &mut SmallRng) -> u8 {
        let i_isize = i as isize;
        let hw = self.half_width as isize;
        let mut state = 0usize;
        for di in -hw..=hw {
            // Modulo fold keeps neighbor values in-range even when adjacent cells come from a
            // different rule's state space (e.g., k=3 neighbor read by a k=2 rule).
            state = state * self.num_states + (arena.get(i_isize + di) as usize % self.num_states);
        }
        self.lookup[state].get(rng)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SimParameters {
    pub rule: Rule,
    pub noise: f64,
    pub seed: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum MixingMode {
    Single,
    #[serde(rename = "V")]
    VerticalDivide { #[serde(rename = "f")] fraction: f32 },
    #[serde(rename = "H")]
    HorizontalDivide { #[serde(rename = "f")] fraction: f32 },
    #[serde(rename = "A")]
    Alternating { #[serde(rename = "h")] stripe_height: u32 },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SimSetup {
    #[serde(rename = "m")]
    pub mode: MixingMode,
    #[serde(rename = "r")]
    pub rules: Vec<SimParameters>,
}

impl SimSetup {
    pub fn single(params: SimParameters) -> Self {
        SimSetup { mode: MixingMode::Single, rules: vec![params] }
    }

    pub fn primary(&self) -> &SimParameters { &self.rules[0] }
    pub fn primary_mut(&mut self) -> &mut SimParameters { &mut self.rules[0] }

    pub fn max_num_states(&self) -> usize {
        self.rules.iter().map(|r| r.rule.num_states).max().unwrap_or(2)
    }
}

pub fn apply_step(arena: &[u8], rule: &Rule, out: &mut Vec<u8>, rng: &mut SmallRng) {
    let looped = Looped::new(arena);
    out.resize(arena.len(), 0);
    for (i, cell) in out.iter_mut().enumerate() {
        *cell = rule.apply(&looped, i, rng);
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

fn rule_index_for(setup: &SimSetup, col: usize, row: usize, w: usize, h: usize) -> usize {
    let n = setup.rules.len();
    if n <= 1 { return 0; }
    match setup.mode {
        MixingMode::Single => 0,
        MixingMode::VerticalDivide { fraction } =>
            if col < (w as f32 * fraction) as usize { 0 } else { 1 },
        MixingMode::HorizontalDivide { fraction } =>
            if row < (h as f32 * fraction) as usize { 0 } else { 1 },
        MixingMode::Alternating { stripe_height } =>
            (row / stripe_height.max(1) as usize) % 2,
    }
}

pub fn cell_rule_index(setup: &SimSetup, col: usize, row: usize, w: usize, h: usize) -> usize {
    rule_index_for(setup, col, row, w, h)
}

fn apply_step_multi(arena: &[u8], row_idx: usize, setup: &SimSetup, sim_h: usize, out: &mut Vec<u8>, rng: &mut SmallRng) {
    let looped = Looped::new(arena);
    let w = arena.len();
    out.resize(w, 0);
    for (i, cell) in out.iter_mut().enumerate() {
        let ri = rule_index_for(setup, i, row_idx, w, sim_h);
        *cell = setup.rules[ri].rule.apply(&looped, i, rng);
    }
}

fn apply_noise_multi(arena: &mut [u8], row_idx: usize, setup: &SimSetup, sim_h: usize, rng: &mut SmallRng) {
    let w = arena.len();
    for i in 0..w {
        let ri = rule_index_for(setup, i, row_idx, w, sim_h);
        let r = &setup.rules[ri];
        if rng.random::<f64>() <= r.noise {
            arena[i] = rng.random_range(0..r.rule.num_states as u8);
        }
    }
}

pub const BATCH_SIZE: usize = 10;

pub struct SimBatch {
    pub start: usize,
    pub pixels: Vec<u8>,
}

pub struct SimRunner {
    setup: SimSetup,
    current: Vec<u8>,
    noise_rng: SmallRng,
    next: Vec<u8>,
    next_output_row: usize,
    sim_width: usize,
    sim_height: usize,
}

impl SimRunner {
    pub fn new(setup: SimSetup, sim_width: usize, sim_height: usize) -> Self {
        let seed = setup.rules[0].seed;
        let num_states = setup.rules[0].rule.num_states;
        Self {
            current: build_arena(sim_width, &all_states(num_states), seed),
            noise_rng: SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15),
            next: vec![0u8; sim_width],
            next_output_row: 0,
            sim_width,
            sim_height,
            setup,
        }
    }

    pub fn step_batch(&mut self) -> Option<SimBatch> {
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
            let current_row = self.next_output_row - 1;
            apply_noise_multi(&mut self.current, current_row, &self.setup, self.sim_height, &mut self.noise_rng);
            apply_step_multi(&self.current, self.next_output_row, &self.setup, self.sim_height, &mut self.next, &mut self.noise_rng);
            std::mem::swap(&mut self.current, &mut self.next);
            pixels.extend_from_slice(&self.current);
            self.next_output_row += 1;
        }
        Some(SimBatch { start, pixels })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_sim(
    setup: SimSetup,
    sim_width: usize,
    sim_height: usize,
) -> mpsc::Receiver<SimBatch> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut runner = SimRunner::new(setup, sim_width, sim_height);
        let t0 = std::time::Instant::now();
        loop {
            match runner.step_batch() {
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
}

#[cfg(target_arch = "wasm32")]
impl WasmSimRunner {
    pub fn new(setup: SimSetup, sim_width: usize, sim_height: usize) -> Self {
        Self { runner: SimRunner::new(setup, sim_width, sim_height) }
    }

    pub fn step_batch(&mut self) -> Option<SimBatch> {
        self.runner.step_batch()
    }
}

pub fn compute_sim(
    params: &SimParameters,
    sim_width: usize,
    sim_height: usize,
    prerun: usize,
) -> Vec<u8> {
    let rule = &params.rule;
    let mut current = build_arena(sim_width, &all_states(rule.num_states), params.seed);
    let mut noise_rng = SmallRng::seed_from_u64(params.seed ^ 0x9e3779b97f4a7c15);
    let mut next = vec![0u8; sim_width];
    let mut result = Vec::with_capacity(sim_width * sim_height);
    result.extend_from_slice(&current);
    for i in 1..(sim_height + prerun) {
        apply_noise(&mut current, params.noise, rule.num_states, &mut noise_rng);
        apply_step(&current, rule, &mut next, &mut noise_rng);
        std::mem::swap(&mut current, &mut next);
        if i > prerun {
            result.extend_from_slice(&current);
        }
    }
    result
}

pub fn random_rule(num_states: usize, half_width: usize, rng: &mut impl Rng) -> Rule {
    let width = 2 * half_width + 1;
    let lookup = (0..num_states.pow(width as u32))
        .map(|_| CellSource::Static(rng.random_range(0..num_states as u8)))
        .collect();
    Rule::new(lookup, num_states, half_width)
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
