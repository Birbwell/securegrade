use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Test {
    pub test_name: Option<String>,
    pub is_public: bool,
    pub input: Option<String>,
    pub output: Option<String>,
    pub input_file_base64: Option<String>,
    pub output_file_base64: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Task {
    pub task_description: String,
    pub allow_editor: bool,
    pub material_base64: Option<String>,
    pub material_filename: Option<String>,
    pub timeout: Option<i32>,
    pub tests: Vec<Test>
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientRequest {
    // Login
    pub user_name: Option<String>,
    pub pass: Option<String>,

    // New Class
    pub class_number: Option<String>,
    pub instructor_user_name: Option<String>,
    pub class_description: Option<String>,

    // New Student
    pub student_user_name: Option<String>,

    // New User (Sign Up)
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,

    // New Assignment
    pub assignment_name: Option<String>,
    pub assignment_description: Option<String>,
    pub deadline: Option<String>,
    pub tasks: Option<Vec<Task>>,

    // Submission
    pub assignment_id: Option<i32>,
    pub lang: Option<String>,
    pub zip_file: Option<Vec<u8>>,

    // Join Class
    pub join_code: Option<String>,
}

impl ClientRequest {
    /// Returns (user_name, pass)
    pub fn get_login(&self) -> Option<(String, String)> {
        if let (Some(uname), Some(pass)) = (self.user_name.clone(), self.pass.clone()) {
            Some((uname, pass))
        } else {
            None
        }
    }

    /// Returns (class_number, instructor_user_name)
    pub fn get_new_class(&self) -> Option<(String, String, String)> {
        if let (Some(class_number), Some(class_description), Some(instructor_user_name)) = (
            self.class_number.clone(),
            self.class_description.clone(),
            self.instructor_user_name.clone(),
        ) {
            Some((class_number, class_description, instructor_user_name))
        } else {
            None
        }
    }

    /// Returns (class_number, student_user_name)
    pub fn get_new_student(&self) -> Option<(String, String)> {
        if let (Some(class_number), Some(student_user_name)) =
            (self.class_number.clone(), self.student_user_name.clone())
        {
            Some((class_number, student_user_name))
        } else {
            None
        }
    }

    /// Returns (class_number, instructor_user_name)
    pub fn get_new_instructor(&self) -> Option<(String, String)> {
        if let (Some(class_number), Some(instructor_user_name)) =
            (self.class_number.clone(), self.instructor_user_name.clone())
        {
            Some((class_number, instructor_user_name))
        } else {
            None
        }
    }
}
