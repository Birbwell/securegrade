use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AssignmentGrade {
    pub name: String,
    pub username: String,
    pub score: f32,
}
