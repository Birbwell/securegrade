use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassInfo {
    assignments: Vec<AssignmentInfo>,
    instructors: Vec<InstructorInfo>,
}

impl ClassInfo {
    pub fn new(assignments: Vec<AssignmentInfo>, instructors: Vec<InstructorInfo>) -> Self {
        Self {
            assignments,
            instructors
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignmentInfo {
    pub assignment_id: i32,
    pub assignment_name: String,
    pub assignment_description: Option<String>,
    pub assignment_deadline: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstructorInfo {
    first_name: String,
    last_name: String,
}

impl InstructorInfo {
    pub fn new(first_name: impl Into<String>, last_name: impl Into<String>) -> Self {
        Self {
            first_name: first_name.into(),
            last_name: last_name.into()
        }
    }
}

