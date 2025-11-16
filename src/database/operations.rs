use crate::database::POSTGRES;
use crate::model::class_item::ClassItem;
use crate::model::class_info::InstructorInfo;
use crate::model::request::ClientRequest;
use crate::model::user_info::UserInfo;
use crate::postgres_lock;

use sqlx::Row;

pub async fn new_class(obj: ClientRequest) -> Result<(), String> {
    let Some((class_number, class_description, instructor_user_name)) = obj.get_new_class() else {
        return Err("Missing fields class_number or instructor_user_name".into());
    };

    let postgres_pool = POSTGRES.read().await;
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
    let postgres_pool = POSTGRES.read().await;
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
    let postgres_pool = POSTGRES.read().await;
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
    let postgres_pool = POSTGRES.read().await;
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
    let postgres_pool = POSTGRES.read().await;
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

pub async fn get_instructors(class_number: impl Into<String>) -> Result<Vec<InstructorInfo>, String> {
    let postgres_pool = POSTGRES.read().await;
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
            let email: String = f.get("email");
            InstructorInfo::new(first_name, last_name, email)
        }).collect::<Vec<InstructorInfo>>();

        return Ok(user_info);
    }

    Err("Could not acquire database lock".into())
}

pub async fn add_join_code(join_code: String, class_number: String) -> Result<(), String> {
    postgres_lock!(transaction, {
        sqlx::query("INSERT INTO class_join_code (join_code, class_number, expiration)
        VALUES ($1, $2, NOW() + INTERVAL '1 hour')
        ON CONFLICT (join_code) DO UPDATE SET
            class_number = EXCLUDED.class_number,
            expiration = EXCLUDED.expiration;")
            .bind(join_code)
            .bind(class_number)
            .execute(&mut *transaction)
            .await
            .unwrap();

        transaction.commit().await.unwrap();
        return Ok(());
    });

    return Err("Failed to acquire transaction lock".into());
}

pub async fn join_class(user_id: i32, join_code: String) -> Result<bool, String> {
    postgres_lock!(transaction, {
        let row = match sqlx::query("SELECT class_number FROM class_join_code WHERE join_code = $1 AND expiration > NOW();")
            .bind(join_code)
            .fetch_one(&mut *transaction)
            .await
        {
            Ok(r) => r,
            Err(sqlx::Error::RowNotFound) => {
                return Ok(false);
            }
            Err(e) => {
                return Err(format!("Database error: {e}"));
            }
        };

        let class_number: String = row.get("class_number");

        if let Err(e) = sqlx::query(
            "INSERT INTO user_class (user_id, class_number, is_instructor)
            VALUES ($1, $2, FALSE);",
        )
        .bind(user_id)
        .bind(&class_number)
        .execute(&mut *transaction)
        .await {
            return Err(format!("Unable to add to user_class table: {e}"));
        }

        transaction.commit().await.unwrap();
        return Ok(true);
    });

    return Err("Failed to acquire transaction lock".into());
}
