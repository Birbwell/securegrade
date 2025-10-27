use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationObject {
    pub class: Option<String>,
    pub session_hash: Vec<u8>,
}
