use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct NewUserObject {
    pub first_name: String,
    pub last_name: String,
    pub user_name: String,
    pub email: String,
    pub pass: String,
}

// impl NewUserObject {
//     pub fn new(
//         first_name: impl Into<String>,
//         last_name: impl Into<String>,
//         user_name: impl Into<String>,
//         email: impl Into<String>,
//         pass: impl Into<String>,
//     ) -> Self {
//         Self {
//             first_name: first_name.into(),
//             last_name: last_name.into(),
//             user_name: user_name.into(),
//             email: email.into(),
//             pass: pass.into(),
//         }
//     }
// }
