use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmissionObject {
    zip_data: Vec<u8>,
}
