use rand::{Rng, rngs::SmallRng};

#[derive(Clone)]
pub enum CellSource {
    Static(u8),
    Random { cumulative: Vec<f32>, values: Vec<u8> },
}

impl CellSource {
    pub fn random(weights: Vec<(f32, u8)>) -> Self {
        let total: f32 = weights.iter().map(|(w, _)| w).sum();
        let mut cumulative = Vec::with_capacity(weights.len());
        let mut values = Vec::with_capacity(weights.len());
        let mut acc = 0.0f32;
        for (w, v) in &weights {
            acc += w / total;
            cumulative.push(acc);
            values.push(*v);
        }
        if let Some(last) = cumulative.last_mut() {
            *last = 1.0;
        }
        CellSource::Random { cumulative, values }
    }

    #[inline]
    pub fn get(&self, rng: &mut SmallRng) -> u8 {
        match self {
            CellSource::Static(x) => *x,
            CellSource::Random { cumulative, values } => {
                let r: f32 = rng.random();
                let idx = cumulative.partition_point(|&w| w < r);
                values[idx.min(values.len() - 1)]
            }
        }
    }

    pub fn static_value(&self) -> Option<u8> {
        match self {
            CellSource::Static(x) => Some(*x),
            CellSource::Random { .. } => None,
        }
    }
}
