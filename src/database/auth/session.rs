use base64::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    session_token: String,
}

impl Session {
    pub fn new(session_token: [u8; 16]) -> Self {
        let base64_session_token = BASE64_STANDARD.encode(&session_token);
        Self {
            session_token: base64_session_token,
        }
    }
}
