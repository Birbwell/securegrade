use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Preferences {
    assignment_name: Option<String>,
    description: Option<String>
}
