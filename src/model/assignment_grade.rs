use serde::Serialize;

#[derive(Serialize)]
pub struct AssignmentGrade {
    pub name: String,
    pub username: String,
    pub score: f32
}
