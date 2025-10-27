use serde::{Deserialize, Serialize};
use base64::prelude::*;
use sha2::{Digest, Sha512};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationObject {
    pub class: Option<String>,
    pub session_hash: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    pub class: Option<String>,
    pub session_base: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub is_valid: bool,
    pub is_admin: bool,
    pub is_instructor: bool,
    pub is_student: bool,
}

impl Into<ValidationObject> for ValidationRequest {
    fn into(self) -> ValidationObject {
        let session_id = BASE64_STANDARD.decode(self.session_base).unwrap();
        let session_hash = Sha512::digest(session_id).to_vec();
        ValidationObject { class: self.class, session_hash }
    }
}
