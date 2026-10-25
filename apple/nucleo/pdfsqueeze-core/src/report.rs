use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    /// output / input
    pub ratio: f64,
    pub pages: u32,
    pub profile: String,
    pub elapsed_ms: u128,
    /// Bytes by category before optimisation.
    pub budget_before: BTreeMap<String, u64>,
    /// Bytes by category after optimisation.
    pub budget_after: BTreeMap<String, u64>,
    pub stages: Vec<StageStat>,
    pub images: Vec<ImageDecision>,
    pub warnings: Vec<String>,
    /// True when the original bytes were returned because nothing beat them.
    pub returned_original: bool,
    pub verified: bool,
}

impl Report {
    pub fn saved_percent(&self) -> f64 {
        if self.input_bytes == 0 {
            0.0
        } else {
            100.0 * (1.0 - self.output_bytes as f64 / self.input_bytes as f64)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StageStat {
    pub name: String,
    pub detail: String,
    pub bytes_saved: i64,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImageDecision {
    pub object: u32,
    pub width: u32,
    pub height: u32,
    pub kind: String,
    pub effective_dpi: Option<f32>,
    pub before_filter: String,
    pub before_bytes: u64,
    pub after_filter: String,
    pub after_bytes: u64,
    pub after_width: u32,
    pub after_height: u32,
    pub ssim: Option<f64>,
    pub candidates_tried: u32,
    pub action: String,
}
