use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct FileObject {
    pub parent_path: String,
    pub name: String,
    pub data: String,
}
