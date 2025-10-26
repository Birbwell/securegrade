use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LoginObject {
    pub user_name: String,
    pub pass: String,
}
