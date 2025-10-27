use crate::database::POSTGRES;
use crate::database::auth;
use crate::model::add_to_class_object::AddToClassObject;
use crate::model::new_class_object::NewClassObject;

use sqlx::Row;

pub async fn new_class(obj: NewClassObject) -> Result<(), String> {
    let validation = obj.clone().into();
    if auth::validate::validate_admin(&validation).await {
        let postgres_pool = POSTGRES.lock().await;
        if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
            let mut transaction = transaction_future.await.unwrap();
            if let Err(e) = sqlx::query("INSERT INTO classes (class_number) VALUES ($1);")
                .bind(&obj.class_number)
                .execute(&mut *transaction)
                .await
            {
                return Err(format!("Unable to add new class: {e}"));
            }

            let id_row = match sqlx::query("SELECT id FROM users WHERE user_name = $1;")
                .bind(&obj.instructor_user_name)
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
    } else {
        return Err("Not authorized".into());
    }
    Ok(())
}

pub async fn add_student(obj: AddToClassObject) -> Result<(), String> {
    let validation = obj.clone().into();
    if auth::validate::validate_instructor(&validation).await {
        // Add instructor
        let postgres_pool = POSTGRES.lock().await;
        if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
            let mut transaction = transaction_future.await.unwrap();
            if let Err(e) = sqlx::query(
                "INSERT INTO user_class (user_id, class_number, is_instructor)
                SELECT id, $1, FALSE FROM users
                WHERE user_name = $2;",
            )
            .bind(obj.class_name)
            .bind(obj.user_name)
            .execute(&mut *transaction)
            .await
            {
                return Err(format!("Unable to add to user_class table: {e}"));
            }
            transaction.commit().await.unwrap();
        }
    } else {
        return Err("Not authorized".into());
    }

    Ok(())
}

pub async fn add_instructor(obj: AddToClassObject) -> Result<(), String> {
    let validation = obj.clone().into();
    if auth::validate::validate_instructor(&validation).await {
        // Add instructor
        let postgres_pool = POSTGRES.lock().await;
        if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
            let mut transaction = transaction_future.await.unwrap();
            if let Err(e) = sqlx::query(
                "INSERT INTO user_class (user_id, class_number, is_instructor)
                SELECT id, $1, TRUE FROM users
                WHERE user_name = $2;",
            )
            .bind(obj.class_name)
            .bind(obj.user_name)
            .execute(&mut *transaction)
            .await
            {
                return Err(format!("Unable to add to user_class table: {e}"));
            }
            transaction.commit().await.unwrap();
        }
    } else {
        return Err("Not authorized".into());
    }

    Ok(())
}
