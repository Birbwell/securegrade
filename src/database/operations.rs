use crate::database::POSTGRES;
use crate::model::assignment_item::AssignmentItem;
use crate::model::class_item::ClassItem;
use crate::model::request::Request;

use chrono::Utc;
use sqlx::Row;

pub async fn new_class(obj: Request) -> Result<(), String> {
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

pub async fn add_student(obj: Request) -> Result<(), String> {
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

pub async fn add_instructor(obj: Request) -> Result<(), String> {
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

pub async fn get_assignment(
    class_number: String,
    assignment_id: i32,
) -> Result<AssignmentItem, String> {
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
            let assignment_description: String = row.get("assignment_description");
            let deadline: chrono::DateTime<Utc> = row.get("deadline");
            AssignmentItem {
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

pub async fn get_assignments(class_number: String) -> Result<Vec<AssignmentItem>, String> {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();
        let rows = match sqlx::query(
            "SELECT DISTINCT a.id, a.assignment_name, a.assignment_description, a.deadline
            FROM assignments a
            JOIN assignment_class c ON c.class_number = $1
            ORDER BY a.id ASC;",
        )
        .bind(class_number.to_lowercase())
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
                let assignment_description: String = r.get("assignment_description");
                let deadline: chrono::DateTime<Utc> = r.get("deadline");
                AssignmentItem {
                    assignment_id,
                    assignment_name,
                    assignment_description,
                    assignment_deadline: deadline.to_string(),
                }
            })
            .collect::<Vec<AssignmentItem>>();

        transaction.commit().await.unwrap();

        return Ok(class_items);
    }

    Err("Server Error".into())
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
        .bind(class_number.to_lowercase())
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("{e}"));
        }

        transaction.commit().await.unwrap();
    }

    Err("Internal Error".into())
}
