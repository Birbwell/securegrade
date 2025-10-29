use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassItem {
    pub class_number: String,
    pub class_description: Option<String>,
}
