use serde::Deserialize;
use sha2::{Digest, Sha512};

use crate::model::{submission_object::file_object::FileObject, validation_object::ValidationObject};
use base64::prelude::*;

mod file_object;

#[derive(Clone, Debug, Deserialize)]
pub struct SubmissionObject {
    pub session_hash: String,
    pub class_name: String,
    pub assignment_id: u64,
    pub banner_id: String,
    pub lang: String,
    pub files: Vec<FileObject>,
}

impl Into<ValidationObject> for SubmissionObject {
    fn into(self) -> ValidationObject {
        let session_id = BASE64_STANDARD.decode(&self.session_hash).unwrap().to_vec();
        let session_hash = Sha512::digest(session_id).to_vec();
        ValidationObject { class: Some(self.class_name), session_hash }
    }
}
