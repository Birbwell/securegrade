use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Test {
    pub input: Option<String>,
    pub output: Option<String>,
    pub input_file: Option<String>,
    pub output_file: Option<String>,
}
