use rand::{Rng, SeedableRng, rngs::SmallRng};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

mod cell_source;
pub use cell_source::CellSource;

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

#[derive(Clone)]
pub struct Rule {
    pub lookup: Vec<CellSource>,
    pub half_width: usize,
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
            state = state * self.num_states + arena.get(i_isize + di) as usize;
        }
        self.lookup[state].get(rng)
    }
}

#[derive(Clone)]
pub struct SimParameters {
    pub rule: Rule,
    pub noise: f64,
    pub seed: u64,
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

pub const BATCH_SIZE: usize = 10;

pub struct SimBatch {
    pub start: usize,
    pub pixels: Vec<u8>,
}

pub struct SimRunner {
    rule: Rule,
    noise: f64,
    current: Vec<u8>,
    noise_rng: SmallRng,
    next: Vec<u8>,
    next_output_row: usize,
    sim_width: usize,
    sim_height: usize,
}

impl SimRunner {
    pub fn new(params: SimParameters, sim_width: usize, sim_height: usize) -> Self {
        let num_states = params.rule.num_states;
        let seed = params.seed;
        Self {
            rule: params.rule,
            noise: params.noise,
            current: build_arena(sim_width, &all_states(num_states), seed),
            noise_rng: SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15),
            next: vec![0u8; sim_width],
            next_output_row: 0,
            sim_width,
            sim_height,
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
            apply_noise(&mut self.current, self.noise, self.rule.num_states, &mut self.noise_rng);
            apply_step(&self.current, &self.rule, &mut self.next, &mut self.noise_rng);
            std::mem::swap(&mut self.current, &mut self.next);
            pixels.extend_from_slice(&self.current);
            self.next_output_row += 1;
        }
        Some(SimBatch { start, pixels })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_sim(
    params: SimParameters,
    sim_width: usize,
    sim_height: usize,
) -> mpsc::Receiver<SimBatch> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut runner = SimRunner::new(params, sim_width, sim_height);
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
    pub fn new(params: SimParameters, sim_width: usize, sim_height: usize) -> Self {
        Self { runner: SimRunner::new(params, sim_width, sim_height) }
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
        apply_step(&current, &rule, &mut next, &mut noise_rng);
        std::mem::swap(&mut current, &mut next);
        if i > prerun {
            result.extend_from_slice(&current);
        }
    }
    result
}

pub fn rule_string_from_lookup(rule: &Rule) -> String {
    rule.lookup.iter().map(|v| match v.static_value() {
        Some(d) => char::from_digit(d as u32, 10).unwrap(),
        None => '?',
    }).collect()
}

pub fn rule_id_from_lookup(rule: &Rule) -> String {
    let rule_width = 2 * rule.half_width + 1;
    let digits = rule_string_from_lookup(rule);
    format!("{};{};{}", rule.num_states, rule_width, digits)
}

pub fn parse_rule_id(id: &str) -> Option<Rule> {
    let mut parts = id.splitn(3, ';');
    let num_states: usize = parts.next()?.parse().ok()?;
    let rule_width: usize = parts.next()?.parse().ok()?;
    if rule_width == 0 || rule_width % 2 == 0 { return None; }
    let half_width = (rule_width - 1) / 2;
    let digits_str = parts.next()?;
    rule_lookup_from_string(digits_str, num_states, half_width)
}

pub fn rule_lookup_from_string(s: &str, num_states: usize, half_width: usize) -> Option<Rule> {
    let width = 2 * half_width + 1;
    let expected_len = num_states.pow(width as u32);
    if s.len() != expected_len { return None; }
    let lookup: Option<Vec<CellSource>> = s.chars()
        .map(|c| c.to_digit(10).and_then(|d| if (d as usize) < num_states { Some(CellSource::Static(d as u8)) } else { None }))
        .collect();
    Some(Rule::new(lookup?, num_states, half_width))
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
