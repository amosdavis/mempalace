use crate::error::MpError;

pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 1.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    (1.0 - dot / (norm_a * norm_b)).clamp(0.0, 2.0)
}

pub struct Embedder;

impl Embedder {
    pub fn new(_device: &str) -> Self {
        Self
    }

    pub fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, MpError> {
        Err(MpError::Embedding(
            "Embedding not compiled in. Enable fastembed ort features.".to_string(),
        ))
    }

    pub fn is_available(&self) -> bool {
        false
    }
}
