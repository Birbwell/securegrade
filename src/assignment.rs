use preferences::Preferences;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use test::Test;

mod preferences;
mod test;

#[derive(Debug, Serialize, Deserialize)]
pub struct Assignment {
    preferences: Preferences,
    pub tests: BTreeMap<String, Test>,
}
