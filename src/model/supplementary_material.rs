use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SupplementaryMaterial {
    pub material: String,
    pub filename: String,
}