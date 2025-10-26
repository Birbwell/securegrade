use crate::database::POSTGRES;
use crate::database::auth;
use crate::model::add_to_class_object::AddToClassObject;
use crate::model::new_class_object::NewClassObject;

pub async fn new_class(obj: NewClassObject) -> Result<(), String> {
    let validation = obj.clone().into();
    if auth::validate::validate_admin(validation).await {
        let postgres_pool = POSTGRES.lock().await;
        if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
            let mut transaction = transaction_future.await.unwrap();
            if let Err(e) = sqlx::query("INSERT INTO classes (class_number) VALUES ($1);")
                .bind(obj.class_name)
                .execute(&mut *transaction)
                .await
            {
                return Err(format!("Unable to add new class: {e}"));
            }

            transaction.commit().await.unwrap();
        }
    } else {
        return Err("Not authorized".into());
    }
    Ok(())
}

pub async fn add_student(obj: AddToClassObject) -> Result<(), String> {
    let validation = obj.clone().into();
    if auth::validate::validate_instructor(validation).await {
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
    if auth::validate::validate_instructor(validation).await {
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
