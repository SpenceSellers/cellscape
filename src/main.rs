use chrono::Local;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use image::{ImageBuffer, Luma};

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
        // Faster modulo calculation for wrapping
        let adjusted_key = ((key % self.len + self.len) % self.len) as usize;
        self.collection[adjusted_key]
    }
}

fn build_arena(n: usize, options: &[u8]) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut arena = Vec::with_capacity(n);
    
    // Pre-allocate and fill in one step
    arena.extend((0..n).map(|_| options[rng.gen_range(0..options.len())]));
    arena
}

// Pre-computed rule lookup table
struct Rule {
    lookup: Vec<u8>,
}

impl Rule {
    fn new(rule_num: u128, width: usize) -> Self {
        // Calculate the number of possible states: 2^width
        let state_count = 1 << width;
        
        // Convert rule to binary and precompute lookup table
        let binary = format!("{:064b}", rule_num);
        let rule_chars: Vec<char> = binary.chars().rev().collect();
        
        // Create lookup table
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
        
        // Compute state index directly using bitwise operations
        let mut state = 0;
        for di in -((n as isize))..=((n as isize)) {
            state = (state << 1) | (arena.get(i_isize + di) as usize);
        }
        
        // Return the precomputed result
        if state < self.lookup.len() {
            self.lookup[state]
        } else {
            0
        }
    }
}

fn apply_step(arena: &[u8], rule: &Rule, width: usize) -> Vec<u8> {
    let mut new_arena = Vec::with_capacity(arena.len());
    let looped = Looped::new(arena.to_vec());
    
    // Optimize by pre-allocating and directly setting values
    new_arena.extend((0..arena.len()).map(|i| rule.apply(&looped, i, width)));
    new_arena
}

fn save_image(arenas: &[Vec<u8>], rule_no: u64) -> Result<(), image::ImageError> {
    let height = arenas.len();
    if height == 0 {
        return Ok(()); // Nothing to save
    }
    
    let width = arenas[0].len();

    // Find min and max values for normalization (single pass)
    let (min_val, max_val) = arenas.iter()
        .flat_map(|row| row.iter())
        .fold((u8::MAX, u8::MIN), |(min, max), &val| {
            (min.min(val), max.max(val))
        });

    // Create image buffer
    let mut img = ImageBuffer::<Luma<u8>, Vec<u8>>::new(width as u32, height as u32);

    // Fill image with normalized pixel values
    let range = if max_val > min_val { max_val as f32 - min_val as f32 } else { 1.0 };
    
    for (y, row) in arenas.iter().enumerate() {
        for (x, &val) in row.iter().enumerate() {
            let normalized_val = if max_val > min_val {
                ((val as f32 - min_val as f32) / range * 255.0) as u8
            } else {
                0
            };
            img.put_pixel(x as u32, y as u32, Luma([normalized_val]));
        }
    }

    // Save image
    // let timestamp = SystemTime::now()
    //     .duration_since(UNIX_EPOCH)
    //     .unwrap()
    //     .as_secs();
    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("outputs/image-{}-{}.png", rule_no, timestamp);

    // Create the outputs directory if it doesn't exist
    std::fs::create_dir_all("outputs")?;
    
    img.save(filename)?;
    Ok(())
}

fn apply_noise(arena: &mut [u8], noise: f64) {
    if noise <= 0.0 {
        return;
    }
    
    let mut rng = rand::thread_rng();
    for cell in arena.iter_mut() {
        if rng.gen::<f64>() <= noise {
            *cell = if rng.gen::<bool>() { 1 } else { 0 };
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = vec![0, 1];
    let mut arena = build_arena(4000, &options);
    let gens: i64 = 4000;
    let mut generations = Vec::with_capacity(gens as usize); // Pre-allocate with a reasonable size
    let save_last = 4000; // Fixed to a reasonable value instead of 1,000,000,000,000
    let noise = 0.0001;
    
    // Pre-compute the rule once
    // let rule_num = 7650;
    let rule_num = rand::thread_rng().gen::<u64>();;
    println!("Using rule number {}", rule_num);
    let width = 6;
    let rule = Rule::new(rule_num as u128, width);
    
    for i in 0..gens {
        // Only save necessary generations
        if i > gens - save_last {
            generations.push(arena.clone());
        }
        
        apply_noise(&mut arena, noise);
        arena = apply_step(&arena, &rule, width);
        
        if i % 100 == 0 {
            println!("Gen {} out of {}", i, gens);
        }
    }
    
    save_image(&generations, rule_num)?;
    Ok(())
}