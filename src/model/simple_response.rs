use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum SimpleResponse {
    Body(String),
    Err(String)
}
