use std::time::Duration;
use std::{io::Read, process::Command};

use crate::model::request::Task as ReqTask;
use crate::model::request::Test as ReqTest;

use axum::body::Bytes;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::Row;

#[derive(Serialize)]
enum Method {
    Stdio,
    Http(u16),
}

impl From<String> for Method {
    fn from(value: String) -> Self {
        if value == "stdio" {
            Method::Stdio
        } else {
            let [_, port] = &value.split(":").collect::<Vec<&str>>()[..] else {
                panic!("Invalid port specified");
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
    has_material: bool,
}

#[derive(Debug)]
pub struct Test {
    pub test_name: Option<String>,
    pub public: bool,
    pub output: String,
    pub input: String,
    pub timeout: Option<Duration>,
}

#[derive(Serialize)]
pub struct FullAssignmentInfo {
    assignment_name: String,
    deadline: String,
    tasks: Vec<ReqTask>,
}

use crate::{
    database::POSTGRES,
    model::{
        assignment_grade::AssignmentGrade, class_info::AssignmentInfo,
        submission_response::SubmissionResponse,
    },
    postgres_lock,
};

pub async fn get_assignment_info(assignment_id: i32) -> Result<Assignment, String> {
    postgres_lock!(transaction, {
        let assignment_row = match sqlx::query("SELECT * FROM assignments WHERE id = $1;")
            .bind(assignment_id)
            .fetch_one(&mut *transaction)
            .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let assignment_name: String = assignment_row.get("assignment_name");
        let assignment_desc: Option<String> = assignment_row.get("assignment_description");
        let assignment_deadline: DateTime<Utc> = assignment_row.get("deadline");

        let task_rows = match sqlx::query("SELECT task_description, allow_editor, placement, id, supplementary_material IS NOT NULL has_material
            FROM tasks WHERE assignment_id = $1;"
        )
            .bind(assignment_id)
            .fetch_all(&mut *transaction)
            .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let tasks = task_rows
            .iter()
            .map(|row| {
                let task_desc: Option<String> = row.get("task_description");
                let allow_editor: bool = row.get("allow_editor");
                let placement: i32 = row.get("placement");
                let task_id: i32 = row.get("id");
                let has_material: bool = row.get("has_material");

                Task {
                    description: task_desc,
                    task_id,
                    allow_editor,
                    placement,
                    has_material,
                }
            })
            .collect::<Vec<Task>>();

        return Ok(Assignment {
            assignment_id,
            name: assignment_name,
            description: assignment_desc,
            tasks,
            deadline: assignment_deadline.to_string(),
        });
    });

    Err("Failed to acquire database lock".into())
}

pub async fn container_get_task_details(task_id: i32) -> Result<Vec<Test>, String> {
    postgres_lock!(transaction, {
        let rows = match sqlx::query("SELECT * FROM tests WHERE task_id = $1;")
            .bind(task_id)
            .fetch_all(&mut *transaction)
            .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        transaction.commit().await.unwrap();

        let tests = rows
            .iter()
            .map(|row| {
                let input: String = row.get("input");
                let output: String = row.get("output");
                let public: bool = row.get("public");
                let timeout: Option<i32> = row.get("timeout");
                let test_name: Option<String> = row.get("test_name");

                let timeout = timeout.map(|f| std::time::Duration::from_secs(f as u64));

                Test {
                    test_name,
                    input,
                    output,
                    public,
                    timeout,
                }
            })
            .collect::<Vec<Test>>();

        return Ok(tests);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn get_assignments_for_class(
    class_number: String,
    user_id: i32,
) -> Result<Vec<AssignmentInfo>, String> {
    postgres_lock!(transaction, {
        let rows = match sqlx::query(
            "SELECT a.id, a.assignment_name, a.assignment_description, a.deadline
            FROM assignments a
            JOIN assignment_class c ON c.assignment_id = a.id
            WHERE c.class_number = $1;",
        )
        .bind(class_number)
        .fetch_all(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("{e}"));
            }
        };

        transaction.commit().await.unwrap();

        let mut assignments = vec![];
        for row in rows {
            let assignment_id: i32 = row.get("id");
            let assignment_name: String = row.get("assignment_name");
            let assignment_description: Option<String> = row.get("assignment_description");
            let assignment_deadline: DateTime<Utc> = row.get("deadline");

            let assignment_score = get_assignment_score(user_id, assignment_id)
                .await
                .unwrap()
                .map(|f| f.score)
                .unwrap_or_default();

            assignments.push(AssignmentInfo {
                assignment_id,
                assignment_name,
                assignment_description,
                assignment_deadline: assignment_deadline.to_string(),
                assignment_score,
            });
        }

        return Ok(assignments);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn retrieve_full_assignment_info(
    assignment_id: i32,
) -> Result<FullAssignmentInfo, String> {
    postgres_lock!(transaction, {
        let assignment_row = match sqlx::query(
            "SELECT * FROM assignments
            WHERE id = $1;",
        )
        .bind(assignment_id)
        .fetch_one(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let deadline: DateTime<Utc> = assignment_row.get("deadline");
        let assignment_name: String = assignment_row.get("assignment_name");

        let task_rows = match sqlx::query(
            "SELECT * FROM tasks
            WHERE assignment_id = $1
            ORDER BY placement ASC;",
        )
        .bind(assignment_id)
        .fetch_all(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let mut tasks = vec![];
        for task in task_rows {
            let task_id: i32 = task.get("id");
            let timeout = None::<i32>;
            let material_vec: Option<Vec<u8>> = task.get("supplementary_material");

            let material_base64 = material_vec.map(|f| base64::prelude::BASE64_STANDARD.encode(f));

            let test_rows = match sqlx::query(
                "SELECT * FROM tests
                WHERE task_id = $1;",
            )
            .bind(task_id)
            .fetch_all(&mut *transaction)
            .await
            {
                Ok(r) => r,
                Err(e) => return Err(format!("{e}")),
            };

            let tests = test_rows
                .iter()
                .map(|test| {
                    let test_name: Option<String> = test.get("test_name");
                    let input: String = test.get("input");
                    let output: String = test.get("output");
                    let is_public: bool = test.get("public");

                    ReqTest {
                        test_name,
                        is_public,
                        input: Some(input),
                        output: Some(output),
                        input_file_base64: None,
                        output_file_base64: None,
                    }
                })
                .collect::<Vec<ReqTest>>();

            tasks.push(ReqTask {
                task_description: task.get("task_description"),
                allow_editor: task.get("allow_editor"),
                material_base64,
                material_filename: task.get("supplementary_filename"),
                timeout,
                tests,
            });
        }

        let fai = FullAssignmentInfo {
            assignment_name,
            deadline: deadline.to_string(),
            tasks,
        };

        return Ok(fai);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn add_assignment(
    class_number: String,
    assignment_name: String,
    assignment_description: Option<String>,
    deadline: String,
    tasks: Vec<ReqTask>,
) -> Result<(), String> {
    postgres_lock!(transaction, {
        let deadline_date_time: DateTime<Utc> = match deadline.parse() {
            Ok(d) => d,
            Err(e) => return Err(format!("Could not parse deadline: {e}")),
        };

        let new_assignment_id: i32 = match sqlx::query(
            "INSERT INTO assignments (assignment_name, assignment_description, deadline)
            VALUES ($1, $2, $3)
            RETURNING id;",
        )
        .bind(assignment_name)
        .bind(assignment_description)
        .bind(deadline_date_time)
        .fetch_one(&mut *transaction)
        .await
        {
            Ok(r) => r.get("id"),
            Err(e) => return Err(format!("{e}")),
        };

        if let Err(e) = sqlx::query("INSERT INTO assignment_class VALUES ($1, $2);")
            .bind(new_assignment_id)
            .bind(class_number)
            .execute(&mut *transaction)
            .await
        {
            return Err(format!("{e}"));
        }

        for (placement, task) in tasks.iter().enumerate() {
            let material = task
                .material_base64
                .as_ref()
                .and_then(|f| base64::prelude::BASE64_STANDARD.decode(f).ok());

            let new_task_id: i32 = match sqlx::query(
                "INSERT INTO tasks (assignment_id, task_description, allow_editor, placement, template, supplementary_material, supplementary_filename, test_method)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id;",
            )
            .bind(new_assignment_id)
            .bind(&task.task_description)
            .bind(task.allow_editor)
            .bind(placement as i32)
            .bind(None::<Vec<u8>>)
            .bind(material)
            .bind(&task.material_filename)
            .bind("stdio")
            .fetch_one(&mut *transaction)
            .await
            {
                Ok(r) => r.get("id"),
                Err(e) => return Err(format!("{e}")),
            };

            for test in &task.tests {
                let input = if let Some(i_f) = &test.input_file_base64 {
                    base64::prelude::BASE64_STANDARD
                        .decode(i_f)
                        .map(|f| String::from_utf8(f).unwrap())
                        .unwrap()
                } else {
                    test.input.clone().unwrap()
                };

                let output = if let Some(o_f) = &test.output_file_base64 {
                    base64::prelude::BASE64_STANDARD
                        .decode(o_f)
                        .map(|f| String::from_utf8(f).unwrap())
                        .unwrap()
                } else {
                    test.output.clone().unwrap()
                };

                if let Err(e) = sqlx::query(
                    "INSERT INTO tests (task_id, input, output, public, timeout, test_name)
                    VALUES ($1, $2, $3, $4, $5, $6);",
                )
                .bind(new_task_id)
                .bind(input)
                .bind(output)
                .bind(test.is_public)
                .bind(task.timeout)
                .bind(&test.test_name)
                .execute(&mut *transaction)
                .await
                {
                    return Err(format!("{e}"));
                }
            }
        }

        transaction.commit().await.unwrap();

        return Ok(());
    });

    Err("Failed to acquire database lock".into())
}

/// Returns if the submission was late
pub async fn mark_as_submitted(
    user_id: i32,
    assignment_id: i32,
    task_id: i32,
    submission_time: DateTime<Utc>,
    zip_file: Bytes,
) -> Result<bool, String> {
    postgres_lock!(transaction, {
        let deadline: DateTime<Utc> =
            match sqlx::query("SELECT deadline FROM assignments WHERE id = $1;")
                .bind(assignment_id)
                .fetch_one(&mut *transaction)
                .await
            {
                Ok(r) => r.get("deadline"),
                Err(e) => return Err(format!("{e}")),
            };

        let was_late = submission_time >= deadline;

        if let Err(e) = sqlx::query(
            "INSERT INTO user_task_grade (user_id, task_id, assignment_id, was_late, submission_zip)
            VALUES ($1, $2, $3, $4, $5);",
        )
        .bind(user_id)
        .bind(task_id)
        .bind(assignment_id)
        .bind(was_late)
        .bind(zip_file.to_vec())
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        transaction.commit().await.unwrap();

        return Ok(was_late);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn container_add_task_grade(
    user_id: i32,
    task_id: i32,
    results: &[u8],
    grade: f32,
) -> Result<(), String> {
    postgres_lock!(transaction, {
        if let Err(e) = sqlx::query(
            "UPDATE user_task_grade
            SET json_results = $1, grade = $2
            WHERE user_id = $3 AND task_id = $4;",
        )
        .bind(results)
        .bind(grade)
        .bind(user_id)
        .bind(task_id)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        transaction.commit().await.unwrap();

        return Ok(());
    });

    Err("Failed to acquire database lock".into())
}

pub async fn get_task_score(
    user_id: i32,
    task_id: i32,
) -> Result<Option<SubmissionResponse>, String> {
    postgres_lock!(transaction, {
        let json_results: Vec<u8> = match sqlx::query(
            "SELECT json_results FROM user_task_grade
            WHERE user_id = $1 AND task_id = $2;",
        )
        .bind(user_id)
        .bind(task_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(Some(r)) => r.get("json_results"),
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("{e}")),
        };

        transaction.commit().await.unwrap();

        let sr = serde_json::from_slice(&json_results).unwrap();
        return Ok(sr);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn get_assignment_score(
    user_id: i32,
    assignment_id: i32,
) -> Result<Option<AssignmentGrade>, String> {
    postgres_lock!(transaction, {
        let row = match sqlx::query(
            "SELECT first_name, last_name, user_name
            FROM users
            JOIN user_class c ON c.user_id = id
            JOIN assignment_class ON assignment_class.assignment_id = $1
            WHERE c.is_instructor = FALSE AND users.id = $2;
        ",
        )
        .bind(assignment_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("{e}")),
        };

        let first_name: String = row.get("first_name");
        let last_name: String = row.get("last_name");
        let username: String = row.get("user_name");

        let name = format!("{} {}", first_name, last_name);

        let tasks = match sqlx::query(
            "SELECT task_id, COUNT(tests.id) n_tests
            FROM tests
            JOIN tasks ON tasks.id = tests.task_id AND tasks.assignment_id = $1
            GROUP BY task_id;",
        )
        .bind(assignment_id)
        .fetch_all(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let mut sum_tests = 0;
        let mut sum_grade = 0.0;

        for task in tasks {
            let n_tests: i64 = task.get("n_tests");
            let task_id: i32 = task.get("task_id");

            let (grade, was_late) = match sqlx::query(
                "SELECT grade, was_late
                FROM user_task_grade
                WHERE user_id = $1 AND task_id = $2;",
            )
            .bind(user_id)
            .bind(task_id)
            .fetch_optional(&mut *transaction)
            .await
            {
                Ok(Some(r)) => {
                    let grade: f32 = r.get("grade");
                    let was_late: bool = r.get("was_late");
                    (grade, was_late)
                }
                Ok(None) => (0.0, false),
                Err(e) => return Err(format!("{e}")),
            };

            sum_tests += n_tests;
            sum_grade += (grade * if was_late { 0.5 } else { 1.0 }) * n_tests as f32;
        }

        let total_grade = AssignmentGrade {
            name,
            username,
            score: sum_grade / sum_tests as f32,
        };

        return Ok(Some(total_grade));
    });

    Err("Failed to acquire database lock".into())
}

pub async fn get_assignment_scores(assignment_id: i32) -> Result<Vec<AssignmentGrade>, String> {
    postgres_lock!(transaction, {
        let rows = match sqlx::query(
            "SELECT id, first_name, last_name, user_name
            FROM users
            JOIN user_class c ON c.user_id = id
            JOIN assignment_class ac ON ac.class_number = c.class_number
            WHERE c.is_instructor = FALSE AND ac.assignment_id = $1;
        ",
        )
        .bind(assignment_id)
        .fetch_all(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let mut grades = vec![];

        for row in rows {
            let user_id: i32 = row.get("id");
            let first_name: String = row.get("first_name");
            let last_name: String = row.get("last_name");
            let username: String = row.get("user_name");

            let name = format!("{} {}", first_name, last_name);

            let tasks = match sqlx::query(
                "SELECT task_id, COUNT(tests.id) n_tests
                FROM tests
                JOIN tasks ON tasks.id = tests.task_id AND tasks.assignment_id = $1
                GROUP BY task_id;",
            )
            .bind(assignment_id)
            .fetch_all(&mut *transaction)
            .await
            {
                Ok(r) => r,
                Err(e) => return Err(format!("{e}")),
            };

            let mut sum_tests = 0;
            let mut sum_grade = 0.0;

            for task in tasks {
                let n_tests: i64 = task.get("n_tests");
                let task_id: i32 = task.get("task_id");

                let (grade, was_late) = match sqlx::query(
                    "SELECT grade, was_late
                    FROM user_task_grade
                    WHERE user_id = $1 AND task_id = $2;",
                )
                .bind(user_id)
                .bind(task_id)
                .fetch_optional(&mut *transaction)
                .await
                {
                    Ok(Some(r)) => {
                        let grade: f32 = r.get("grade");
                        let was_late: bool = r.get("was_late");
                        (grade, was_late)
                    }
                    Ok(None) => (0.0, false),
                    Err(e) => return Err(format!("{e}")),
                };

                sum_tests += n_tests;
                sum_grade += (grade * if was_late { 0.5 } else { 1.0 }) * n_tests as f32;
            }

            let total_grade = AssignmentGrade {
                name,
                username,
                score: sum_grade / sum_tests as f32,
            };

            grades.push(total_grade);
        }

        transaction.commit().await.unwrap();
        return Ok(grades);
    });

    Err("Failed to acquire database lock".into())
}

pub async fn download_submission(
    username: String,
    assignment_id: i32,
) -> Result<Option<Vec<u8>>, String> {
    postgres_lock!(transaction, {
        let Ok(user_row) = sqlx::query("SELECT id FROM users WHERE user_name = $1;")
            .bind(&username)
            .fetch_one(&mut *transaction)
            .await
        else {
            return Err("Bad username".into());
        };

        let user_id: i32 = user_row.get("id");

        let rows = sqlx::query(
            "SELECT task_id, task_description, submission_zip FROM user_task_grade
            JOIN tasks ON tasks.id = task_id
            WHERE user_id = $1 AND tasks.assignment_id = $2;",
        )
        .bind(user_id)
        .bind(assignment_id)
        .fetch_all(&mut *transaction)
        .await
        .unwrap();

        transaction.commit().await.unwrap();

        if rows.is_empty() {
            return Ok(None);
        }

        let workdir = format!("/tmp/securegrade/download/{}-{}", username, assignment_id);
        std::fs::create_dir_all(&workdir).unwrap();

        for row in &rows {
            let file: Vec<u8> = row.get("submission_zip");
            let task_id: i32 = row.get("task_id");
            std::fs::write(format!("{}/Task{}.zip", workdir, task_id), file).unwrap();
        }

        Command::new("zip")
            .args([
                "-rj",
                &format!("{}/{}-{}.zip", workdir, username, assignment_id),
                &workdir,
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        let mut zip_file = vec![];
        let mut f =
            std::fs::File::open(format!("{}/{}-{}.zip", workdir, username, assignment_id)).unwrap();
        f.read_to_end(&mut zip_file).unwrap();

        std::fs::remove_dir_all(&workdir).unwrap();

        return Ok(Some(zip_file));
    });

    Err("Failed to acquire database lock".into())
}

pub async fn download_material(task_id: i32) -> Result<Option<(String, String)>, String> {
    postgres_lock!(transaction, {
        let row = match sqlx::query(
            "SELECT supplementary_material, supplementary_filename FROM tasks
            WHERE id = $1;",
        )
        .bind(task_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("{e}")),
        };

        let material: Option<Vec<u8>> = row.get("supplementary_material");
        let filename: String = row.get("supplementary_filename");

        let material_base64 =
            base64::prelude::BASE64_STANDARD.encode(material.as_ref().unwrap_or(&vec![]));

        transaction.commit().await.unwrap();

        return Ok(Some((material_base64, filename)));
    });

    Err("Failed to acquire database lock".into())
}

pub async fn submission_in_progress(user_id: i32, task_id: i32) -> bool {
    postgres_lock!(transaction, {
        return matches!(sqlx::query(
                "SELECT * FROM user_task_grade WHERE user_id = $1 AND task_id = $2 AND grade IS NULL;"
            )
                .bind(user_id)
                .bind(task_id)
                .fetch_optional(&mut *transaction)
                .await,
            Ok(Some(_))
        );
    });

    false
}

pub async fn remove_old_grade(user_id: i32, task_id: i32) -> Result<(), String> {
    postgres_lock!(transaction, {
        sqlx::query("DELETE FROM user_task_grade WHERE user_id = $1 AND task_id = $2;")
            .bind(user_id)
            .bind(task_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        transaction.commit().await.unwrap();
        return Ok(());
    });

    Err("Failed to acquire transaction lock".into())
}

pub async fn update_assignment(
    assignment_id: i32,
    assignment_name: String,
    assignment_description: Option<String>,
    deadline: String,
    tasks: Vec<ReqTask>,
) -> Result<(), String> {
    postgres_lock!(transaction, {
        let Ok(deadline) = deadline.parse::<DateTime<Utc>>() else {
            return Err("Invalid deadline date string.".into());
        };

        if let Err(e) = sqlx::query(
            "UPDATE assignments
            SET assignment_name = $1, assignment_description = $2, deadline = $3
            WHERE id = $4;",
        )
        .bind(assignment_name)
        .bind(assignment_description)
        .bind(deadline)
        .bind(assignment_id)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        if let Err(e) = sqlx::query(
            "DELETE FROM tasks
            WHERE assignment_id = $1;",
        )
        .bind(assignment_id)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        for (
            i,
            ReqTask {
                task_description,
                allow_editor,
                material_base64,
                material_filename,
                timeout,
                tests,
            },
        ) in tasks.iter().enumerate()
        {
            let material_bytes = material_base64
                .as_ref()
                .map(|f| base64::prelude::BASE64_STANDARD.decode(f).unwrap());

            let task_row = match sqlx::query(
                "INSERT INTO tasks (assignment_id, task_description, allow_editor, placement, supplementary_material, supplementary_filename)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id;"
            ).bind(assignment_id)
            .bind(task_description)
            .bind(allow_editor)
            .bind(i as i32)
            .bind(material_bytes)
            .bind(material_filename)
            .fetch_one(&mut *transaction)
            .await {
                Ok(r) => r,
                Err(e) => return Err(format!("{e}")),
            };

            let task_id: i32 = task_row.get("id");

            for ReqTest {
                test_name,
                is_public,
                input,
                output,
                input_file_base64,
                output_file_base64,
            } in tests
            {
                let input = if let Some(i_f) = &input_file_base64 {
                    base64::prelude::BASE64_STANDARD
                        .decode(i_f)
                        .map(|f| String::from_utf8(f).unwrap())
                        .unwrap()
                } else {
                    input.clone().unwrap()
                };

                let output = if let Some(o_f) = &output_file_base64 {
                    base64::prelude::BASE64_STANDARD
                        .decode(o_f)
                        .map(|f| String::from_utf8(f).unwrap())
                        .unwrap()
                } else {
                    output.clone().unwrap()
                };

                if let Err(e) = sqlx::query(
                    "INSERT INTO tests (task_id, test_name, input, output, public, timeout)
                    VALUES ($1, $2, $3, $4, $5, $6);",
                )
                .bind(task_id)
                .bind(test_name)
                .bind(input)
                .bind(output)
                .bind(is_public)
                .bind(timeout)
                .execute(&mut *transaction)
                .await
                {
                    return Err(format!("{e}"));
                }
            }
        }

        transaction.commit().await.unwrap();
        return Ok(());
    });

    Err("Failed to acquire transaction lock".into())
}
