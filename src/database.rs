use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env::var;
use std::sync::{Mutex, OnceLock};

mod user;

static POSTGRES: Mutex<OnceLock<Pool<Postgres>>> = Mutex::new(OnceLock::new());

pub async fn init_database() -> Result<(), String> {
    let Ok(name) = var("PSQL_NAME") else {
        return Err("PSQL_NAME environment variable not present".into());
    };
    let Ok(pass) = var("PSQL_PASS") else {
        return Err("PSQL_PASS environment variable not present".into());
    };

    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .connect(&format!("postgres://{}:{}@localhost", name, pass))
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("{e}"));
        }
    };

    // Initiate schema
    if let Ok(mut transaction) = pool.begin().await {
        // Create a schema for the autograder
        if let Err(e) = sqlx::query(r#"CREATE SCHEMA IF NOT EXISTS autograder"#)
            .execute(&mut *transaction)
            .await
        {
            return Err(format!("Could not create schema 'autograder': {e}"));
        }

        // Set the search path to the autograder schema.
        sqlx::query(r#"SET search_path TO autograder;"#)
            .execute(&mut *transaction)
            .await
            .unwrap();

        if let Err(e) = sqlx::query("CREATE TABLE IF NOT EXISTS users(
            id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            is_admin BOOLEAN DEFAULT FALSE
        );").execute(&mut *transaction).await {
            return Err(format!("Failed to create user table: {e}"));
        };

        // Create a table for the classes
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS classes(
            class_number TEXT PRIMARY KEY
        );"#,
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create table classes: {e}"));
        }

        // Create a table for the user-class associations
        if let Err(e) = sqlx::query(r#"CREATE TABLE IF NOT EXISTS user_class (
            user_id INTEGER REFERENCES users (id) ON UPDATE CASCADE ON DELETE CASCADE,
            class_number TEXT REFERENCES classes (class_number) ON UPDATE CASCADE ON DELETE CASCADE,
            is_instructor BOOLEAN NOT NULL,
            CONSTRAINT student_class_pkey PRIMARY KEY (user_id, class_number)
        );"#).execute(&mut *transaction).await {
            return Err(format!("Could not create association table: {e}"));
        }

        // Create the authentication table
        if let Err(e) = sqlx::query("CREATE TABLE IF NOT EXISTS auth_user (
            hash BYTEA PRIMARY KEY,
            user_id INTEGER REFERENCES users (id)
        );").execute(&mut *transaction).await {
            return Err(format!("Could not create auth table: {e}"));
        }

        // Create the session table
        if let Err(e) = sqlx::query("CREATE TABLE IF NOT EXISTS sessions(
            session_hash BYTEA PRIMARY KEY,
            user_id INTEGER REFERENCES users (id),
            expiration TIMESTAMPTZ NOT NULL
        );").execute(&mut *transaction).await {
            return Err(format!("Could not create session table: {e}"));
        }

        if let Err(e) = transaction.commit().await {
            return Err(format!("Could not commit table-creation transaction: {e}"));
        };
    }

    if let Ok(lock) = POSTGRES.lock() {
        lock.get_or_init(|| pool);
    }

    Ok(())
}
