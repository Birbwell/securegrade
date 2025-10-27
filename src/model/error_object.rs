use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ErrorObject(pub String);
