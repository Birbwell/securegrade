use crate::model::validation_object::ValidationObject;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddToClassObject {
    pub session_hash: String,
    pub class_name: String,
    pub user_name: String,
}

impl Into<ValidationObject> for AddToClassObject {
    fn into(self) -> ValidationObject {
        let session_id = BASE64_STANDARD.decode(&self.session_hash).unwrap();
        let session_hash = Sha512::digest(session_id).to_vec();
        ValidationObject {
            class: Some(self.class_name),
            session_hash,
        }
    }
}
