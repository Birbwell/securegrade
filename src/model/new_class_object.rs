use crate::model::validation_object::ValidationObject;

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewClassObject {
    pub session_base: String,
    pub class_number: String,
    pub instructor_user_name: String,
}

impl Into<ValidationObject> for NewClassObject {
    fn into(self) -> ValidationObject {
        let session_id = BASE64_STANDARD.decode(&self.session_base).unwrap();
        let session_hash = Sha512::digest(session_id).to_vec();
        ValidationObject {
            class: Some(self.class_number),
            session_hash,
        }
    }
}
