use serde::{Deserialize, Serialize};

use crate::{database, model::file_object::FileObject};

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum RequestPermissions {
    Admin,
    Instructor,
    Student,
    User,
    #[default]
    None,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
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

    // Submission
    pub assignment_id: Option<u32>,
    pub lang: Option<String>,
    // pub files: Vec<FileObject>,
    pub zip_file: Option<Vec<u8>>,
}

impl Request {
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
        if let (Some(class_number), Some(class_description), Some(instructor_user_name)) =
            (self.class_number.clone(), self.class_description.clone(), self.instructor_user_name.clone())
        {
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
