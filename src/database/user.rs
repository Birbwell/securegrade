use base64::{Engine, prelude::BASE64_STANDARD};
use sha2::{Digest, Sha512};
use sqlx::Row;

use crate::model::request::ClientRequest;

use super::POSTGRES;

fn create_hash(user_name: impl Into<Vec<u8>>, pass: impl Into<Vec<u8>>) -> Vec<u8> {
    let user_name = user_name.into();
    let pass = pass.into();

    let name_len = user_name.len();
    let first_half_user_name = &user_name[0..name_len / 2];
    let last_half_user_name = &user_name[name_len / 2..];

    let secret_sauce = vec![first_half_user_name, &pass, last_half_user_name].concat();
    Sha512::digest(secret_sauce).to_vec()
}

pub async fn get_user_from_session(session_base: impl AsRef<[u8]>) -> Option<i32> {
    let session_id = BASE64_STANDARD.decode(session_base).unwrap();
    let session_hash = Sha512::digest(session_id).to_vec();

    let postgres_pool = POSTGRES.read().await;
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let mut transaction = transaction_future.await.unwrap();

        let row = sqlx::query("SELECT user_id FROM user_session WHERE session_hash = $1;")
            .bind(session_hash)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        let id: i32 = row.get("user_id");
        return Some(id);
    }
    None
}

pub async fn register_user(new_user: ClientRequest) -> Result<[u8; 16], String> {
    let Some((user_name, pass)) = new_user.get_login() else {
        return Err(format!("Missing fields user_name or pass in request"));
    };

    let hash = create_hash(user_name, pass);

    {
        let postgres_pool = POSTGRES.read().await;
        if let Some(transaction) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
            let Ok(mut transaction) = transaction.await else {
                return Err("Unable to lock database transaction".into());
            };

            let id: i32 = match sqlx::query(
            "INSERT INTO users (first_name, last_name, user_name, email) VALUES ($1, $2, $3, $4) RETURNING id;",
            )
            .bind(new_user.first_name.clone())
            .bind(new_user.last_name.clone())
            .bind(new_user.user_name.clone())
            .bind(new_user.email.clone())
            .fetch_one(&mut *transaction)
            .await {
                Ok(id) => id.get("id"),
                Err(e) => return Err(format!("Could not insert into database: {e}")),
            };

            if sqlx::query("INSERT INTO user_auth (hash, user_id) VALUES ($1, $2);")
                .bind(hash)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .is_err()
            {
                return Err("Could not add to authentication table".into());
            }

            if let Err(e) = transaction.commit().await {
                return Err(format!("Could not commit database transaction: {e}"));
            }
        } else {
            return Err("Could not create user".into());
        }
    }

    tracing::info!("User Created");
    Ok(login_user(new_user).await?)
}

pub async fn login_user(user: ClientRequest) -> Result<[u8; 16], String> {
    let Some((user_name, pass)) = user.get_login() else {
        return Err(format!("Missing fields user_name or pass"));
    };

    let hash = create_hash(user_name, pass);
    let postgres_pool = POSTGRES.read().await;
    let mut session_id = [0u8; 16];
    if let Some(transaction_future) = postgres_pool.as_ref().and_then(|f| Some(f.begin())) {
        let Ok(mut transaction) = transaction_future.await else {
            panic!();
        };

        let Ok(Some(out)) = sqlx::query("SELECT * FROM user_auth WHERE hash = $1;")
            .bind(hash)
            .fetch_optional(&mut *transaction)
            .await
        else {
            return Err("Incorrect password or account does not exist.".into());
        };

        let id: i32 = out.get("user_id");

        rand::fill(&mut session_id);

        let session_hash = Sha512::digest(session_id).to_vec();

        let current_time = chrono::Utc::now();
        let one_hour = chrono::TimeDelta::hours(1);

        // Clear previous sessions
        if let Err(e) = sqlx::query("DELETE FROM user_session WHERE user_id = $1;")
            .bind(id)
            .execute(&mut *transaction)
            .await
        {
            return Err(format!("Could not clear prior sessions: {e}"));
        }

        if let Err(e) = sqlx::query(
            "INSERT INTO user_session (session_hash, user_id, expiration) VALUES ($1, $2, $3);",
        )
        .bind(session_hash)
        .bind(id)
        .bind(current_time + one_hour)
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create login session: {e}"));
        }

        if let Err(e) = transaction.commit().await {
            return Err(format!("Failed to commit database transaction: {e}"));
        }

        tracing::info!("Logged in user {}", id);
    } else {
        return Err("Could not begin transaction".into());
    }

    Ok(session_id)
}
