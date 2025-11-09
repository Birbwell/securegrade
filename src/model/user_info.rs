use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    first_name: String,
    last_name: String,
    username: String
}

impl UserInfo {
    pub fn new(first_name: String, last_name: String, username: String) -> Self {
        Self {
            first_name,
            last_name,
            username
        }
    }
}

