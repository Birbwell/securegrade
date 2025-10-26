use sha2::{Digest, Sha512};
use sqlx::Row;

use crate::database;

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

async fn register_user(
    user_name: impl Into<Vec<u8>>,
    pass: impl Into<Vec<u8>>,
    first_name: impl Into<String>,
    last_name: impl Into<String>,
    is_instructor: Option<bool>,
) {
    let b = is_instructor.unwrap_or_default();

    let hash = create_hash(user_name, pass);

    if let Ok(postgres_pool) = POSTGRES.lock() {
        if let Ok(mut transaction) = postgres_pool.get().unwrap().begin().await {
            let id = sqlx::query(
                "INSERT INTO users (first_name, last_name, is_admin) VALUES ($1, $2, $3) RETURNING id;",
            )
            .bind(first_name.into())
            .bind(last_name.into())
            .bind(b)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

            let id: i32 = id.get("id");

            sqlx::query("INSERT INTO auth_user (hash, user_id) VALUES ($1, $2);")
                .bind(hash)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .unwrap();

            transaction.commit().await.unwrap();
        }
    }
}

async fn login_user(user_name: impl Into<Vec<u8>>, pass: impl Into<Vec<u8>>) -> Option<[u8; 16]> {
    let hash = create_hash(user_name, pass);

    if let Ok(postgres_pool) = POSTGRES.lock() {
        if let Ok(mut transaction) = postgres_pool.get().unwrap().begin().await {
            let Some(out) = sqlx::query("SELECT * FROM auth_user WHERE hash = $1;")
                .bind(hash)
                .fetch_optional(&mut *transaction)
                .await
                .unwrap()
            else {
                panic!("User not found");
            };

            let id: i32 = out.get("user_id");

            let mut session_id = [0u8; 16];
            rand::fill(&mut session_id);

            let session_hash = Sha512::digest(session_id).to_vec();

            let current_time = chrono::Utc::now();
            let one_hour = chrono::TimeDelta::hours(1);

            sqlx::query("INSERT INTO sessions (session_hash, user_id, expiration) VALUES ($1, $2, $3);")
                .bind(session_hash)
                .bind(id)
                .bind(current_time + one_hour)
                .execute(&mut *transaction)
                .await
                .unwrap();

            transaction.commit().await.unwrap();

            return Some(session_id);
        }
    }

    None
}

// #[tokio::test]
// async fn register_test() {
//     database::init_database().await.unwrap();
//     register_user("aeskul", "Hopi0104", "John", "Birdwell", Some(false)).await;
// }

#[tokio::test]
async fn login_test() {
    database::init_database().await.unwrap();
    login_user("aeskul", "Hopi0104").await;
}
