use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    Ok,
    Err
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseObject {
    status: Status,
    score: Option<f32>,
    tests: Vec<(String, bool)>,
    error_message: Option<String>
}
