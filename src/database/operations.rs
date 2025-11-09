use crate::database::POSTGRES;
use crate::model::assignment_grade::AssignmentGrade;
use crate::model::class_info::AssignmentInfo;
use crate::model::class_item::ClassItem;
use crate::model::class_info::InstructorInfo;
use crate::model::request::ClientRequest;
use crate::model::user_info::UserInfo;

use axum::body::Body;
use axum::http::{Response, StatusCode};
use chrono::Utc;
use sqlx::Row;

pub async fn new_class(obj: ClientRequest) -> Result<(), String> {
    let Some((class_number, class_description, instructor_user_name)) = obj.get_new_class() else {
        return Err("Missing fields class_number or instructor_user_name".into());
    };

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        if let Err(e) =
            sqlx::query("INSERT INTO classes (class_number, class_description) VALUES ($1, $2);")
                .bind(&class_number)
                .bind(&class_description)
                .execute(&mut *transaction)
                .await
        {
            return Err(format!("Unable to add new class: {e}"));
        }

        let id_row = match sqlx::query("SELECT id FROM users WHERE user_name = $1;")
            .bind(&instructor_user_name)
            .fetch_one(&mut *transaction)
            .await
        {
            Ok(n) => n,
            Err(e) => return Err(format!("Given instructor does not exist: {e}")),
        };

        let id: i32 = id_row.get("id");

        if let Err(e) = sqlx::query(
            "INSERT INTO user_class (user_id, class_number, is_instructor) VALUES ($1, $2, $3);",
        )
        .bind(id)
        .bind(obj.class_number)
        .bind(true)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create user-class relation: {e}"));
        };

        transaction.commit().await.unwrap();
    }
    Ok(())
}

pub async fn add_student(obj: ClientRequest) -> Result<(), String> {
    let Some((class_number, student_user_name)) = obj.get_new_student() else {
        return Err("Missing fields class_number or student_user_name".into());
    };

    // Add student
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        if let Err(e) = sqlx::query(
            "INSERT INTO user_class (user_id, class_number, is_instructor)
                SELECT id, $1, FALSE FROM users
                WHERE user_name = $2;",
        )
        .bind(&class_number)
        .bind(&student_user_name)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Unable to add to user_class table: {e}"));
        }
        transaction.commit().await.unwrap();
    }

    Ok(())
}

pub async fn list_all_students(
    exclude_from_class: Option<String>,
) -> Result<Vec<UserInfo>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let rows = if let Some(exclude) = exclude_from_class {
            match sqlx::query(
                "SELECT DISTINCT first_name, last_name, user_name
                    FROM users
                    LEFT JOIN user_class ON users.id = user_class.user_id
                    WHERE user_class.class_number IS NULL OR user_class.class_number <> $1;",
            )
            .bind(exclude)
            .fetch_all(&mut *transaction)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!("{e}"));
                }
            }
        } else {
            match sqlx::query("SELECT * FROM users;")
                .fetch_all(&mut *transaction)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!("{e}"));
                }
            }
        };

        let users = rows
            .iter()
            .map(|f| {
                let username: String = f.get("user_name");
                let first_name: String = f.get("first_name");
                let last_name: String = f.get("last_name");

                UserInfo::new(first_name, last_name, username)
            })
            .collect::<Vec<UserInfo>>();

        Ok(users)
    } else {
        Err("Failed to acquire postgres lock".into())
    }
}

pub async fn add_instructor(obj: ClientRequest) -> Result<(), String> {
    let Some((class_number, instructor_user_name)) = obj.get_new_instructor() else {
        return Err("Missing fields class_number or student_user_name".into());
    };

    // Add instructor
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        if let Err(e) = sqlx::query(
            "INSERT INTO user_class (user_id, class_number, is_instructor)
                SELECT id, $1, TRUE FROM users
                WHERE user_name = $2;",
        )
        .bind(&class_number)
        .bind(&instructor_user_name)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Unable to add to user_class table: {e}"));
        }
        transaction.commit().await.unwrap();
    }

    Ok(())
}

pub async fn get_classes(user_id: i32) -> Result<Vec<ClassItem>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        let rows = match sqlx::query(
            "SELECT c.class_number, c.class_description
            FROM classes c
            JOIN user_class u ON u.class_number = c.class_number
            WHERE u.user_id = $1;",
        )
        .bind(user_id)
        .fetch_all(&mut *transaction)
        .await
        {
            Err(e) => return Err(format!("Unable to get classes: {e}")),
            Ok(r) => r,
        };

        let class_items = rows
            .iter()
            .map(|r| {
                let class_number: String = r.get("class_number");
                let class_description: Option<String> = r.get("class_description");
                ClassItem {
                    class_number,
                    class_description,
                }
            })
            .collect::<Vec<ClassItem>>();

        transaction.commit().await.unwrap();

        return Ok(class_items);
    }

    Err("Server Error".into())
}

pub async fn get_assignment(assignment_id: i32) -> Result<AssignmentInfo, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        let row = match sqlx::query("SELECT * FROM assignments WHERE id = $1;")
            .bind(assignment_id)
            .fetch_one(&mut *transaction)
            .await
        {
            Err(e) => return Err(format!("Unable to get assignments: {e}")),
            Ok(r) => r,
        };

        let assignment = {
            let assignment_id: i32 = row.get("id");
            let assignment_name: String = row.get("assignment_name");
            let assignment_description: Option<String> = row.get("assignment_description");
            let deadline: chrono::DateTime<Utc> = row.get("deadline");
            AssignmentInfo {
                assignment_id,
                assignment_name,
                assignment_description,
                assignment_deadline: deadline.to_string(),
            }
        };

        transaction.commit().await.unwrap();

        return Ok(assignment);
    }

    Err("Server Error".into())
}

pub async fn get_assignments(
    class_number: impl Into<String>,
) -> Result<Vec<AssignmentInfo>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        let rows = match sqlx::query(
            "SELECT DISTINCT a.id, a.assignment_name, a.assignment_description, a.deadline
            FROM assignments a
            JOIN assignment_class c ON c.class_number = $1
            ORDER BY a.id ASC;",
        )
        .bind(class_number.into())
        .fetch_all(&mut *transaction)
        .await
        {
            Err(e) => return Err(format!("Unable to get classes: {e}")),
            Ok(r) => r,
        };

        let class_items = rows
            .iter()
            .map(|r| {
                let assignment_id: i32 = r.get("id");
                let assignment_name: String = r.get("assignment_name");
                let assignment_description: Option<String> = r.get("assignment_description");
                let deadline: chrono::DateTime<Utc> = r.get("deadline");
                AssignmentInfo {
                    assignment_id,
                    assignment_name,
                    assignment_description,
                    assignment_deadline: deadline.to_string(),
                }
            })
            .collect::<Vec<AssignmentInfo>>();

        transaction.commit().await.unwrap();

        return Ok(class_items);
    }

    Err("Server Error".into())
}

pub async fn get_instructors(class_number: impl Into<String>) -> Result<Vec<InstructorInfo>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let rows = match sqlx::query(
            "SELECT DISTINCT first_name, last_name, email
            FROM users
            JOIN user_class ON users.id = user_class.user_id
            WHERE user_class.class_number = $1 AND user_class.is_instructor = TRUE;",
        )
        .bind(class_number.into())
        .fetch_all(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        transaction.commit().await.unwrap();

        let user_info = rows.iter().map(|f| {
            let last_name: String = f.get("last_name");
            let first_name: String = f.get("first_name");
            InstructorInfo::new(first_name, last_name)
        }).collect::<Vec<InstructorInfo>>();

        return Ok(user_info);
    }

    Err("Could not acquire database lock".into())
}

pub async fn add_assignment(
    class_number: String,
    assignment_name: String,
    assignment_description: String,
) -> Result<(), String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        let row =
            match sqlx::query("INSERT INTO assignments (assignment_name, assignment_description, deadline) VALUES ($1, $2, now()) RETURNING id;")
                .bind(assignment_name)
                .bind(assignment_description)
                .fetch_one(&mut *transaction)
                .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("{e}")),
        };

        let assignment_id: i32 = row.get("id");

        if let Err(e) = sqlx::query(
            "INSERT INTO assignment_class (assignment_id, class_number) VALUES ($1, $2);",
        )
        .bind(assignment_id)
        .bind(class_number)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        transaction.commit().await.unwrap();
        return Ok(());
    }

    Err("Internal Error".into())
}

pub async fn get_assignment_scores(assignment_id: i32) -> Result<Vec<AssignmentGrade>, ()> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(rows) = sqlx::query(
            "SELECT grade, id, first_name, last_name, user_name
            FROM user_assignment_grade 
            JOIN users ON users.id = user_assignment_grade.user_id
            WHERE assignment_id = $1",
        )
        .bind(assignment_id)
        .fetch_all(&mut *transaction)
        .await
        else {
            return Err(());
        };

        let mut assignment_grades = vec![];
        for row in rows {
            let f_n: String = row.get("first_name");
            let l_n: String = row.get("last_name");
            let grade = AssignmentGrade {
                name: format!("{} {}", f_n, l_n),
                username: row.get("user_name"),
                score: row.get("grade"),
            };
            assignment_grades.push(grade);
        }

        transaction.commit().await.unwrap();
        return Ok(assignment_grades);
    }

    Err(())
}

pub async fn container_add_grade(
    user_id: i32,
    assignment_id: i32,
    results: &[u8],
    grade: f32,
) -> Result<(), String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        if let Err(e) = sqlx::query(
            "INSERT INTO user_assignment_grade (user_id, assignment_id, json_results, grade) VALUES ($1, $2, $3, $4) ON CONFLICT (user_id, assignment_id) DO UPDATE SET json_results = $3, grade = $4;"
        ).bind(user_id)
        .bind(assignment_id)
        .bind(results)
        .bind(grade)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        tracing::info!("Storing submission from {user_id} for {assignment_id}");
        transaction.commit().await.unwrap();
    }
    Ok(())
}

pub async fn remove_old_grade(user_id: i32, assignment_id: i32) -> Result<(), String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        sqlx::query("UPDATE user_assignment_grade SET json_results = NULL, grade = NULL, error = NULL WHERE user_id = $1 AND assignment_id = $2;")
            .bind(user_id)
            .bind(assignment_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        transaction.commit().await.unwrap();
        return Ok(());
    }

    return Err("Failed to acquire transaction lock".into());
}

pub async fn container_retrieve_grade(user_id: i32, assignment_id: i32) -> Response<Body> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let res = match sqlx::query("SELECT json_results, error FROM user_assignment_grade WHERE user_id = $1 AND assignment_id = $2;")
            .bind(user_id)
            .bind(assignment_id)
            .fetch_optional(&mut *transaction)
            .await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    return Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body("Resource not found.".into())
                        .unwrap();
                }
                Err(e) => {
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(format!("{e}").into())
                        .unwrap();
                }
            };

        let json_results: Option<Vec<u8>> = res.get("json_results");
        let err_msg: Option<String> = res.get("error");

        if let Some(e) = err_msg {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(format!("{e}").into())
                .unwrap();
        }

        if let Some(j) = json_results {
            return Response::builder()
                .status(StatusCode::OK)
                .body(j.into())
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Resource not found.".into())
        .unwrap()
}

pub async fn mark_as_submitted(
    user_id: i32,
    assignment_id: i32,
    zipfile: &[u8],
) -> Result<(), String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        if let Err(e) = sqlx::query(
            "INSERT INTO user_assignment_grade (user_id, assignment_id, submission_zip) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING;",
        )
        .bind(user_id)
        .bind(assignment_id)
        .bind(zipfile)
        .execute(&mut *transaction)
        .await {
            return Err(format!("{e}"));
        };

        transaction.commit().await.unwrap();
    }
    Ok(())
}

pub async fn download_submission(username: String, assignment_id: i32) -> Result<Vec<u8>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(user_row) = sqlx::query("SELECT id FROM users WHERE user_name = $1;")
            .bind(username)
            .fetch_one(&mut *transaction)
            .await
        else {
            return Err("Bad Username".into());
        };

        let user_id: i32 = user_row.get("id");

        let row = sqlx::query("SELECT submission_zip FROM user_assignment_grade WHERE user_id = $1 AND assignment_id = $2;")
            .bind(user_id)
            .bind(assignment_id)
            .fetch_one(&mut *transaction)
            .await.unwrap();

        transaction.commit().await.unwrap();

        let zip_file: Vec<u8> = row.get("submission_zip");

        return Ok(zip_file);
    }

    return Err("Unable to acquire database lock".into());
}

pub async fn submission_in_progress(user_id: i32, assignment_id: i32) -> bool {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let res = match sqlx::query(
            "SELECT * FROM user_assignment_grade WHERE user_id = $1 AND assignment_id = $2 AND grade IS NULL;",
        )
        .bind(user_id)
        .bind(assignment_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(Some(r)) => r,
            _ => return false,
        };

        let _user_id: i32 = res.get("user_id");
        let _assignment_id: i32 = res.get("assignment_id");

        if user_id == _user_id && assignment_id == _assignment_id {
            return true;
        } else {
            return false;
        }
    }
    false
}
