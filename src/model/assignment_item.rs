use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignmentItem {
    pub assignment_id: i32,
    pub assignment_name: String,
    pub assignment_description: Option<String>,
    pub assignment_deadline: String,
}
