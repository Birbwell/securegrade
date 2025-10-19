use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FileObject {
    pub parent_path: String,
    pub name: String,
    pub data: String,
}