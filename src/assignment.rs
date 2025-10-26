use preferences::Preferences;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use test::Test;

mod preferences;
mod test;

#[derive(Debug, Serialize, Deserialize)]
pub struct Assignment {
    #[serde(default)]
    preferences: Preferences,
    pub tests: BTreeMap<String, Test>,
}

impl Assignment {
    pub fn get_description(&self) -> Option<String> {
        self.preferences.description.clone()
    }

    pub fn get_timeout(&self) -> Option<Duration> {
        self.preferences
            .timeout
            .and_then(|f| Some(Duration::from_secs(f)))
    }
}
