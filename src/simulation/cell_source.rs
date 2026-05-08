use rand::{Rng, rngs::SmallRng};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone)]
pub enum CellSource {
    Static(u8),
    Random { cumulative: Vec<f32>, values: Vec<u8> },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum CellSourceHelper {
    Static(u8),
    Random { weights: Vec<(u8, f32)> },
}

impl Serialize for CellSource {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let helper = match self {
            CellSource::Static(d) => CellSourceHelper::Static(*d),
            CellSource::Random { cumulative, values } => {
                let mut weights = Vec::with_capacity(values.len());
                let mut prev = 0.0f32;
                for (&cum, &val) in cumulative.iter().zip(values.iter()) {
                    weights.push((val, cum - prev));
                    prev = cum;
                }
                CellSourceHelper::Random { weights }
            }
        };
        helper.serialize(s)
    }
}

impl<'de> Deserialize<'de> for CellSource {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match CellSourceHelper::deserialize(d)? {
            CellSourceHelper::Static(v) => Ok(CellSource::Static(v)),
            CellSourceHelper::Random { weights } => {
                if weights.is_empty() {
                    return Err(serde::de::Error::custom("empty weights"));
                }
                Ok(CellSource::random(weights.into_iter().map(|(s, w)| (w, s)).collect()))
            }
        }
    }
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
