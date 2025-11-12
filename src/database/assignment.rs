use std::time::Duration;

use serde::Serialize;

pub mod operations;

#[derive(Serialize)]
enum Method {
    Stdio,
    Http(u16),
}

impl Into<Method> for String {
    fn into(self) -> Method {
        if self == "stdio" {
            Method::Stdio
        } else {
            let [_, port] = &self.split(':').collect::<Vec<&str>>()[..] else {
                panic!("INVALID DATA FOUND IN DATABASE");
            };
            let p = port.parse::<u16>().unwrap();
            Method::Http(p)
        }
    }
}

#[derive(Serialize)]
pub struct Assignment {
    assignment_id: i32,
    name: String,
    description: Option<String>,
    tasks: Vec<Task>,
    deadline: String,
}

#[derive(Serialize)]
struct Task {
    description: Option<String>,
    task_id: i32,
    placement: i32,
    allow_editor: bool,
}

#[derive(Debug)]
pub struct Test {
    pub test_name: Option<String>,
    pub public: bool,
    pub output: String,
    pub input: String,
    pub timeout: Option<Duration>,
}
