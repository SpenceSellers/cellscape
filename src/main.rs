use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use image::{ImageBuffer, Luma};

struct Looped {
    collection: Vec<u8>,
}

impl Looped {
    fn new(collection: Vec<u8>) -> Self {
        Self { collection }
    }

    fn get(&self, key: isize) -> u8 {
        let len = self.collection.len() as isize;
        let mut adjusted_key = key;
        if key >= len {
            adjusted_key = key % len;
        } else if key < 0 {
            adjusted_key = (key % len + len) % len;
        }
        self.collection[adjusted_key as usize]
    }
}

fn build_arena(n: usize, options: &[u8]) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut arena = Vec::with_capacity(n);
    for _ in 0..n {
        let val = options[rng.gen_range(0..options.len())];
        arena.push(val);
    }
    arena
}

fn apply_step<F>(arena: &[u8], step: usize, rule_func: F) -> Vec<u8>
where
    F: Fn(&Looped, usize, usize) -> u8,
{
    let mut new_arena = Vec::with_capacity(arena.len());
    let looped = Looped::new(arena.to_vec());
    for i in 0..arena.len() {
        new_arena.push(rule_func(&looped, i, step));
    }
    new_arena
}

fn cell_to_color(cell: u8) -> char {
    if cell < 64 {
        ' '
    } else if cell < 128 {
        '░'
    } else if cell < 192 {
        '▒'
    } else if cell <= 255 {
        '█'
    } else {
        // This case shouldn't happen with u8
        '?'
    }
}

fn print_arena(arena: &[u8]) {
    let result: String = arena.iter().map(|&a| cell_to_color(a)).collect();
    println!("{}", result);
}

fn save_image(arenas: &[Vec<u8>]) -> Result<(), image::ImageError> {
    let height = arenas.len();
    let width = arenas[0].len();

    // Find min and max values for normalization
    let mut min_val = u8::MAX;
    let mut max_val = u8::MIN;
    for row in arenas {
        for &val in row {
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }
    }

    // Create image buffer
    let mut img = ImageBuffer::<Luma<u8>, Vec<u8>>::new(width as u32, height as u32);

    // Fill image with normalized pixel values
    for (y, row) in arenas.iter().enumerate() {
        for (x, &val) in row.iter().enumerate() {
            let normalized_val = if max_val > min_val {
                ((val as f32 - min_val as f32) / (max_val as f32 - min_val as f32) * 255.0) as u8
            } else {
                0
            };
            img.put_pixel(x as u32, y as u32, Luma([normalized_val]));
        }
    }

    // Save image
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("outputs/image-{}.png", timestamp);

    // Create the outputs directory if it doesn't exist
    std::fs::create_dir_all("outputs")?;
    
    img.save(filename)?;
    Ok(())
}

fn the_func(arena: &Looped, i: usize, step: usize) -> u8 {
    // num = 4442, like in the Python code
    let num = 4442;
    
    // Convert to binary and pad to 16 bits
    let rule = format!("{:016b}", num);
    
    // Reverse the string
    let rule: String = rule.chars().rev().collect();
    
    // Convert to vector of chars
    let rule: Vec<char> = rule.chars().collect();
    
    do_rule(&rule, arena, i, 4)
}

fn do_rule(rule: &[char], arena: &Looped, i: usize, width: usize) -> u8 {
    let n = width / 2;
    
    let mut binary_string = String::new();
    for di in -((n as isize))..=((n as isize)) {
        binary_string.push_str(&arena.get((i as isize) + di).to_string());
    }
    
    // Convert binary string to integer
    let as_int = usize::from_str_radix(&binary_string, 2).unwrap_or(0);
    
    // Check if index is within bounds
    if as_int >= rule.len() {
        return 0;
    }
    
    // Convert rule char to u8
    if rule[as_int] == '1' { 1 } else { 0 }
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
    let mut generations = Vec::new();
    let gens: i64 = 4000;
    let last: i64 = 1000000000000;
    let noise = 0.0001;
    
    for i in 0..gens {
        if i > gens - last {
            generations.push(arena.clone());
        }
        
        apply_noise(&mut arena, noise);
        arena = apply_step(&arena, i as usize, the_func);
        
        if i % 100 == 0 {
            println!("Gen {} out of {}", i, gens);
        }
    }
    
    save_image(&generations)?;
    Ok(())
}