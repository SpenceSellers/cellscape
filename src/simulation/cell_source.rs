#[derive(Clone)]
pub enum CellSource {
    Static(u8)
}

impl CellSource {
    pub fn get(&self) -> u8 {
        match self {
            CellSource::Static(x) => *x
        }
    }
}