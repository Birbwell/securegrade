use std::{io::Read, process::Command};

use axum::body::Bytes;
use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{
    database::{
        POSTGRES,
        assignment::{Assignment, Task, Test},
    },
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

        let task_rows = match sqlx::query("SELECT * FROM tasks WHERE assignment_id = $1;")
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

                Task {
                    description: task_desc,
                    task_id,
                    allow_editor,
                    placement,
                }
            })
            .collect::<Vec<Task>>();

        return Ok(Assignment {
            assignment_id: assignment_id,
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

                let timeout = timeout.and_then(|f| Some(std::time::Duration::from_secs(f as u64)));

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
            "SELECT DISTINCT a.id, a.assignment_name, a.assignment_description, a.deadline
            FROM assignments a
            JOIN assignment_class c ON c.class_number = $1
            ORDER BY a.id ASC;",
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
                .and_then(|f| Some(f.score))
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

pub async fn add_assignment(
    class_number: String,
    assignment_name: String,
    assignment_description: Option<String>,
) -> Result<(), String> {
    let postgres_pool = POSTGRES.read().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let new_assignment_id: i32 = match sqlx::query(
            "INSERT INTO assignments (assignment_name, assignment_description, deadline)
            VALUES ($1, $2, $3)
            RETURNING id;",
        )
        .bind(assignment_name)
        .bind(assignment_description)
        .bind(Utc::now())
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

        transaction.commit().await.unwrap();

        return Ok(());
    }

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
    let postgres_pool = POSTGRES.read().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

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
    }

    Err("Failed to acquire database lock".into())
}

pub async fn container_add_task_grade(
    user_id: i32,
    task_id: i32,
    results: &[u8],
    grade: f32,
) -> Result<(), String> {
    let postgres_pool = POSTGRES.read().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

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
    }

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

    return Err("Failed to acquire database lock".into());
}

pub async fn get_assignment_scores(assignment_id: i32) -> Result<Vec<AssignmentGrade>, String> {
    postgres_lock!(transaction, {
        let rows = match sqlx::query(
            "SELECT id, first_name, last_name, user_name
            FROM users
            JOIN user_class c ON c.user_id = id
            JOIN assignment_class ON assignment_class.assignment_id = $1
            WHERE c.is_instructor = FALSE;
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
                &format!("{}", workdir),
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

pub async fn submission_in_progress(user_id: i32, task_id: i32) -> bool {
    postgres_lock!(transaction, {
        return match sqlx::query(
            "SELECT * FROM user_task_grade WHERE user_id = $1 AND task_id = $2 AND grade IS NULL;",
        )
        .bind(user_id)
        .bind(task_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(Some(_)) => true,
            _ => false,
        };
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

    return Err("Failed to acquire transaction lock".into());
}
