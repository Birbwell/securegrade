use chrono::Utc;
use sqlx::Row;

use crate::{database::POSTGRES, model::validation_object::ValidationObject};

pub async fn validate_token(validation: &ValidationObject) -> bool {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(Some(res)) = sqlx::query("SELECT * FROM user_session WHERE session_hash = $1;")
            .bind(&validation.session_hash)
            .fetch_optional(&mut *transaction)
            .await
        else {
            return false;
        };

        let expiration: chrono::DateTime<Utc> = res.get("expiration");
        if expiration > chrono::Utc::now() {
            return true;
        }

        transaction.commit().await.unwrap();
    }

    false
}

pub async fn validate_student(validation: &ValidationObject) -> bool {
    let Some(class) = validation.class.clone() else {
        return false;
    };

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(Some(res)) = sqlx::query("SELECT * FROM user_session WHERE session_hash = $1;")
            .bind(&validation.session_hash)
            .fetch_optional(&mut *transaction)
            .await
        else {
            return false;
        };

        let user_id: i32 = res.get("user_id");
        let expiration: chrono::DateTime<Utc> = res.get("expiration");

        let is_instructor: bool = sqlx::query(
            "SELECT is_instructor FROM user_class WHERE user_id = $1 AND class_number = $2;",
        )
        .bind(user_id)
        .bind(class)
        .fetch_one(&mut *transaction)
        .await
        .unwrap()
        .get("is_instructor");

        if expiration >= chrono::Utc::now() {
            return !is_instructor;
        }
    }

    false
}

pub async fn validate_instructor(validation: &ValidationObject) -> bool {
    let Some(class) = validation.class.clone() else {
        return false;
    };

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(Some(res)) = sqlx::query("SELECT * FROM user_session WHERE session_hash = $1;")
            .bind(&validation.session_hash)
            .fetch_optional(&mut *transaction)
            .await
        else {
            return false;
        };

        let user_id: i32 = res.get("user_id");
        let expiration: chrono::DateTime<Utc> = res.get("expiration");

        let is_instructor: bool = sqlx::query(
            "SELECT is_instructor FROM user_class WHERE user_id = $1 AND class_number = $2;",
        )
        .bind(user_id)
        .bind(class)
        .fetch_one(&mut *transaction)
        .await
        .unwrap()
        .get("is_instructor");

        if expiration >= chrono::Utc::now() {
            return is_instructor;
        }
    }

    false
}

pub async fn validate_admin(validation: &ValidationObject) -> bool {
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let Ok(Some(res)) = sqlx::query("SELECT * FROM user_session WHERE session_hash = $1;")
            .bind(&validation.session_hash)
            .fetch_optional(&mut *transaction)
            .await
        else {
            return false;
        };

        let user_id: i32 = res.get("user_id");
        let expiration: chrono::DateTime<Utc> = res.get("expiration");

        let is_admin: bool = sqlx::query("SELECT is_admin FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
            .unwrap()
            .get("is_admin");

        if chrono::Utc::now() <= expiration {
            return is_admin;
        }
    }
    false
}
