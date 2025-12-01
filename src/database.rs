//! Contains all functions associated with accessing the database
//! 
//! Functions are grouped into submodules depending on what the operation affects. For example, operations primarily affecting the `users` table will be in the `users` module.
//! 
//! Submodules are:
//! - assignment
//! - auth
//! - user
//! - operations (for generic operations, will be refactored out)

use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env::var;
use std::sync::LazyLock;
use tokio::sync::RwLock;

pub mod assignment;
pub mod auth;
pub mod operations;
pub mod user;

/// Static, global postgres connection pool
static POSTGRES: LazyLock<RwLock<Option<Pool<Postgres>>>> = LazyLock::new(|| RwLock::new(None));

/// Simplifies the syntax of acquiring the postgres lock, so to avoid reusing the same unnecessarily complex lines of code.
/// 
/// Acquires the postgres lock, assigns it to the identifier provided in the first parameter, then executes the block provided in the second parameter.
/// 
/// ## Usage:
/// 
/// ```
/// postgres_lock!(transaction, {
///     let user_rows = sqlx::query("SELECT * FROM users;")
///         .fetch_all(&mut *transaction)       // Identifier used here
///         .await
///         .unwrap();
/// });
/// ```
#[macro_export]
macro_rules! postgres_lock {
    ($transaction: ident, $($body: tt)*) => {
        let postgres_pool = POSTGRES.read().await;
        if let Some(transaction_future) = postgres_pool.as_ref().map(|f| f.begin()) {
            let mut $transaction = transaction_future.await.unwrap();
            $($body)*
        }
    };
}

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

        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            user_name TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            is_admin BOOLEAN DEFAULT FALSE
        );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Failed to create user table: {e}"));
        };

        // Create a table for the classes
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS classes (
            class_number TEXT PRIMARY KEY,
            class_description TEXT
        );"#,
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create table classes: {e}"));
        }

        // Create a table for the user-class associations
        if let Err(e) = sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS user_class (
            user_id INTEGER REFERENCES users (id),
            class_number TEXT REFERENCES classes (class_number),
            is_instructor BOOLEAN NOT NULL,
            CONSTRAINT student_class_pkey PRIMARY KEY (user_id, class_number)
        );"#,
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create association table: {e}"));
        }

        // Create the authentication table
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_auth (
            hash BYTEA PRIMARY KEY,
            user_id INTEGER REFERENCES users (id)
        );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create auth table: {e}"));
        }

        // Create the session table
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_session (
            session_hash BYTEA PRIMARY KEY,
            expiration TIMESTAMPTZ NOT NULL,
            user_id INTEGER REFERENCES users (id)
        );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create session table: {e}"));
        }

        // Create assignments
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS assignments (
                id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
                assignment_name TEXT NOT NULL,
                assignment_description TEXT,
                deadline TIMESTAMPTZ NOT NULL,
                visible BOOLEAN NOT NULL DEFAULT FALSE
            );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create assignment table: {e}"));
        }

        // Create task
        // test_method = { 'stdio' | 'http:xxxx' }, where xxxx => port number
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
                assignment_id INTEGER REFERENCES assignments(id) ON UPDATE CASCADE ON DELETE CASCADE,
                task_description TEXT,
                allow_editor BOOLEAN DEFAULT FALSE,
                placement INTEGER NOT NULL,
                template BYTEA,
                supplementary_material BYTEA,
                supplementary_filename TEXT,
                test_method TEXT DEFAULT 'stdio'
            );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create task table: {e}"));
        }

        // Create tests
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS tests (
                id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON UPDATE CASCADE ON DELETE CASCADE,
                test_name TEXT,
                input TEXT NOT NULL,
                output TEXT NOT NULL,
                public BOOLEAN NOT NULL DEFAULT FALSE,
                timeout INTEGER
            );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create test table: {e}"));
        }

        // And assignment-class associations
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS assignment_class (
            assignment_id INTEGER REFERENCES assignments (id),
            class_number TEXT REFERENCES classes (class_number)
        );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create assignment-class table: {e}"));
        }

        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_task_grade (
                user_id INTEGER NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON UPDATE CASCADE ON DELETE CASCADE,
                assignment_id INTEGER NOT NULL REFERENCES assignments(id) ON UPDATE CASCADE ON DELETE CASCADE,
                json_results BYTEA,
                submission_zip BYTEA,
                grade FLOAT4,
                error TEXT,
                was_late BOOLEAN,
                CONSTRAINT user_task_id_pkey PRIMARY KEY (user_id, task_id)
            );",
        )
        .execute(&mut *transaction)
        .await
        {
            return Err(format!("Could not create user_assignment_grade table: {e}"));
        }

        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS class_join_code (
                join_code TEXT PRIMARY KEY,
                class_number TEXT REFERENCES classes (class_number),
                expiration TIMESTAMPTZ NOT NULL
            );"
        ).execute(&mut *transaction)
        .await {
            return Err(format!("Could not create class_join_code table: {e}"));
        }

        if let Err(e) = transaction.commit().await {
            return Err(format!("Could not commit table-creation transaction: {e}"));
        };
    }

    let mut lock = POSTGRES.write().await;
    *lock = Some(pool);

    Ok(())
}
