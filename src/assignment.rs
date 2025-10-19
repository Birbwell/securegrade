use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use test::Test;
use preferences::Preferences;

mod preferences;
mod test;

#[derive(Debug, Serialize, Deserialize)]
pub struct Assignment {
    preferences: Preferences,
    pub tests: HashMap<String, Test>
}
