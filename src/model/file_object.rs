use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileObject {
    pub parent_path: String,
    pub name: String,
    pub data: String,
}
