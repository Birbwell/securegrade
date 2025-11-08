use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Permissions {
    admin: bool,
    instructor: bool,
    student: bool,
    user: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SimpleResponse {
    body: String,
    perms: Permissions,
}

impl SimpleResponse {
    pub fn new(
        body: String,
        is_admin: bool,
        is_instructor: bool,
        is_student: bool,
        is_user: bool,
    ) -> Self {
        Self {
            body,
            perms: Permissions {
                admin: is_admin,
                instructor: is_instructor,
                student: is_student,
                user: is_user,
            },
        }
    }
}
