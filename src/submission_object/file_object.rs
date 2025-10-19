use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FileObject(pub String, pub String, pub String);