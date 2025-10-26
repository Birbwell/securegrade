use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Preferences {
    pub(super) assignment_name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) timeout: Option<u64>,
}
