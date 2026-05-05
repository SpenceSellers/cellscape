use rand::{Rng, SeedableRng, rngs::SmallRng};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
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

pub struct Rule {
    lookup: Vec<u8>,
    half_width: isize,
}

impl Rule {
    pub fn new(rule_num: u128, width: usize) -> Self {
        let state_count = 1 << width;
        let binary = format!("{:0128b}", rule_num);
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

pub fn apply_noise(arena: &mut [u8], noise: f64, rng: &mut SmallRng) {
    for cell in arena.iter_mut() {
        let threshold: f64 = rng.random();
        let new_val: u8 = rng.random::<bool>() as u8;
        if threshold <= noise {
            *cell = new_val;
        }
    }
}

pub fn noise_from_slider(s: f64) -> f64 {
    10f64.powf(s * 6.0 - 7.0)
}

pub const BATCH_SIZE: usize = 10;

pub struct SimBatch {
    pub start: usize,
    pub pixels: Vec<u8>,
}

pub fn spawn_sim(
    rule_no: u128,
    sim_width: usize,
    sim_height: usize,
    noise_atomic: Arc<AtomicU64>,
    seed: u64,
) -> mpsc::Receiver<SimBatch> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let rule = Rule::new(rule_no, 7);
        let mut current = build_arena(sim_width, &[0, 1], seed);
        let mut noise_rng = SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15);
        let mut next = vec![0u8; sim_width];

        let mut batch_pixels = Vec::with_capacity(BATCH_SIZE * sim_width);
        let mut batch_start = 0usize;

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
        if !batch_pixels.is_empty() {
            tx.send(SimBatch { start: batch_start, pixels: batch_pixels }).ok();
        }
        println!("simulation done in {:.2?}", t0.elapsed());
    });
    rx
}

pub fn compute_sim(rule_no: u128, sim_width: usize, sim_height: usize, noise: f64, seed: u64) -> Vec<u8> {
    let rule = Rule::new(rule_no, 7);
    let mut current = build_arena(sim_width, &[0, 1], seed);
    let mut noise_rng = SmallRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15);
    let mut next = vec![0u8; sim_width];
    let mut result = Vec::with_capacity(sim_width * sim_height);
    result.extend_from_slice(&current);
    for _ in 1..sim_height {
        apply_noise(&mut current, noise, &mut noise_rng);
        apply_step(&current, &rule, &mut next);
        std::mem::swap(&mut current, &mut next);
        result.extend_from_slice(&current);
    }
    result
}

pub fn rule_lookup_from_no(rule_no: u128) -> Vec<u8> {
    (0..128).map(|i| ((rule_no >> i) & 1) as u8).collect()
}

pub fn rule_no_from_lookup(lookup: &[u8]) -> u128 {
    lookup.iter().take(128).enumerate().fold(0u128, |acc, (i, &v)| {
        acc | ((v as u128) << i)
    })
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
