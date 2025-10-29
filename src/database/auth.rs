use base64::{Engine, prelude::BASE64_STANDARD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use sqlx::Row;

use crate::database::POSTGRES;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    session_base: String,
}

impl Session {
    pub fn new(token: impl AsRef<[u8]>) -> Self {
        let base = BASE64_STANDARD.encode(token);
        Self {
            session_base: base
        }
    }
}

pub async fn session_exists_and_valid(token: String) -> Result<bool, String> {
    let session_id = BASE64_STANDARD.decode(token).unwrap();
    let session_hash = Sha512::digest(session_id).to_vec();
    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let row = match sqlx::query(
            "SELECT user_id, expiration FROM user_session WHERE session_hash = $1;",
        )
        .bind(session_hash)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("An error occured querying the database: {e}"));
            }
        };

        let Some(row) = row else {
            return Ok(false);
        };

        let now = chrono::Utc::now();
        let expiration: DateTime<Utc> = row.get("expiration");


        if now > expiration {
            return Ok(false);
        }

        transaction.commit().await.unwrap();
        return Ok(true);
    }

    Ok(false)
}

pub async fn session_is_student(
    class_number: String,
    token: impl AsRef<[u8]>,
) -> Result<bool, String> {
    let session_hash = BASE64_STANDARD.decode(token).unwrap();
    let session_id = Sha512::digest(session_hash).to_vec();

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let row = match sqlx::query(
            "SELECT user_id, expiration FROM user_session WHERE session_hash = $1;",
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("An error occured querying the database: {e}"));
            }
        };

        let Some(row) = row else {
            return Ok(false);
        };

        let now = chrono::Utc::now();
        let expiration: DateTime<Utc> = row.get("expiration");

        if now > expiration {
            return Ok(false);
        }

        let user_id: i32 = row.get("user_id");

        // let Ok(row) = sqlx::query("SELECT is_admin FROM users WHERE id = $1;")
        //     .bind(user_id)
        //     .fetch_one(&mut *transaction)
        //     .await
        // else {
        //     return Err(format!("User ID missing from users table: {user_id}"));
        // };

        // UNCOMMENT IF YOU WANT ADMINS TO HAVE STUDENT PERMS
        // let is_admin: bool = row.get("is_admin");
        // if is_admin {
        //     return Ok(true);
        // }

        match sqlx::query(
            "SELECT is_instructor FROM user_class WHERE class_number = $1 AND user_id = $2;",
        )
        .bind(class_number)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(None) => return Ok(false),
            // Ok(Some(_)) => return Ok(true),
            Ok(Some(r)) => {
                let is_instructor: bool = r.get("is_instructor");
                return Ok(!is_instructor);  // Invert, cause an entry was found and theyre *NOT* an instructor
            }
            Err(e) => return Err(format!("An unexpected error occured: {e}")),
        };
    }
    Ok(false)
}

pub async fn session_is_instructor(
    class_number: String,
    token: impl AsRef<[u8]>,
) -> Result<bool, String> {
    let session_hash = BASE64_STANDARD.decode(token).unwrap();
    let session_id = Sha512::digest(session_hash).to_vec();

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let row = match sqlx::query(
            "SELECT user_id, expiration FROM user_session WHERE session_hash = $1;",
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("An error occured querying the database: {e}"));
            }
        };

        let Some(row) = row else {
            return Ok(false);
        };

        let now = chrono::Utc::now();
        let expiration: DateTime<Utc> = row.get("expiration");

        if now > expiration {
            return Ok(false);
        }

        let user_id: i32 = row.get("user_id");

        let Ok(row) = sqlx::query("SELECT is_admin FROM users WHERE id = $1;")
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
        else {
            return Err(format!("User ID missing from users table: {user_id}"));
        };

        // UNCOMMENT IF YOU WANT ADMINS TO HAVE INSTRUCTOR PRIVELEGES
        // let is_admin: bool = row.get("is_admin");
        // if is_admin {
        //     return Ok(true);
        // }

        let row = match sqlx::query(
            "SELECT is_instructor FROM user_class WHERE class_number = $1 AND user_id = $2;",
        )
        .bind(class_number)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(None) => return Ok(false),
            Ok(Some(r)) => r,
            Err(e) => return Err(format!("An unexpected error occured: {e}")),
        };

        let is_instructor: bool = row.get("is_instructor");
        transaction.commit().await.unwrap();
        return Ok(is_instructor);
    }
    Ok(false)
}

pub async fn session_is_admin(token: impl AsRef<[u8]>) -> Result<bool, String> {
    let session_hash = BASE64_STANDARD.decode(token).unwrap();
    let session_id = Sha512::digest(session_hash).to_vec();

    let postgres_pool = POSTGRES.lock().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let row = match sqlx::query(
            "SELECT user_id, expiration FROM user_session WHERE session_hash = $1;",
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("An error occured querying the database: {e}"));
            }
        };

        let Some(row) = row else {
            return Ok(false);
        };

        let now = chrono::Utc::now();
        let expiration: DateTime<Utc> = row.get("expiration");

        if now > expiration {
            return Ok(false);
        }

        let user_id: i32 = row.get("user_id");

        let Ok(row) = sqlx::query("SELECT is_admin FROM users WHERE id = $1;")
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
        else {
            return Err(format!("User ID missing from users table: {user_id}"));
        };

        let is_admin: bool = row.get("is_admin");
        if is_admin {
            return Ok(true);
        }

        transaction.commit().await.unwrap();
    }
    Ok(false)
}
