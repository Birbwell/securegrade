use serde::Deserialize;

use crate::submission_object::file_object::FileObject;

mod file_object;

#[derive(Debug, Deserialize)]
pub struct SubmissionObject {
    pub assignment_id: u64,
    pub banner_id: String,
    pub lang: String,
    pub files: Vec<FileObject>,
}
