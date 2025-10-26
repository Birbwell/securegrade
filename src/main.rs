use std::fs::read_dir;
use std::net::SocketAddr;
use std::sync::{LazyLock, Mutex, RwLock};

use axum::routing::{get, post, put};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use serde_json::Value;
use tracing::{Level, debug};
use tracing_subscriber::FmtSubscriber;

use crate::assignment::Assignment;
use crate::container::run_container;
use crate::response_object::ResponseObject;
use crate::submission_object::SubmissionObject;

mod assignment;
mod container;
mod database;
mod image;
mod response_object;
mod submission_object;

static NEXT_ID: Mutex<u32> = Mutex::new(0);

// TODO: RESEARCH DDoS RESILIENCE
// TODO: IMPLEMENT USER AUTHENTICATION

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let app: Router = Router::new()
        .route("/submit", get(receive_submission))
        .route("/add_assignment", post(add_assignment))
        .route("/update_assignment", put(update_assignment))
        .route("/get_assignments", get(get_assignments))
        .route("/add_user", post(add_student));

    let config =
        RustlsConfig::from_pem_file("aeskul.net_certificate.cer", "aeskul.net_private_key.key")
            .await
            .unwrap();

    database::init_database().await.unwrap();

    let server = axum_server::bind_rustls("0.0.0.0:443".parse::<SocketAddr>().unwrap(), config);
    server.serve(app.into_make_service()).await.unwrap()
}

async fn receive_submission(json_submission: String) -> Result<Json<ResponseObject>, String> {
    let sub_ob = serde_json::from_str::<SubmissionObject>(&json_submission).unwrap();
    Ok(Json(run_container(sub_ob).await?))
}

async fn add_assignment(toml: String) -> Result<String, String> {
    // let id = NEXT_ID.lock().unwrap().clone();
    let id = if let Ok(mut next_id) = NEXT_ID.lock() {
        *next_id += 1;
        next_id.clone()
    } else {
        return Err("Unable to create new assignment".into());
    };
    tracing::info!("Creating new assignment: {id}");

    std::fs::create_dir_all(format!("assignments/{}", id)).unwrap();
    std::fs::write(format!("assignments/{}/assignment.toml", id), toml).unwrap();

    Ok("true".into())
}

async fn get_assignments() -> Result<String, String> {
    let mut assignments = vec![];

    let read_dir = std::fs::read_dir("assignments").unwrap();
    for dir in read_dir {
        let _dir = dir.unwrap();
        let id = _dir.file_name().into_string().unwrap();

        let assignment_raw =
            std::fs::read_to_string(format!("assignments/{}/assignment.toml", id)).unwrap();
        let assignment = toml::from_str::<Assignment>(&assignment_raw).unwrap();

        assignments.push(format!(
            "{} -- {}",
            id,
            assignment
                .get_description()
                .unwrap_or("No Description.".into())
        ));
    }

    Ok(assignments.join("\n"))
}

async fn update_assignment(assignment_json: String) -> Result<String, String> {
    let assignment = serde_json::from_str::<Value>(&assignment_json).unwrap();

    let id = assignment.get("id").unwrap().as_u64().unwrap();
    let toml = assignment
        .get("toml")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    tracing::info!("Updating assignment: {}", id);
    std::fs::create_dir_all(format!("assignments/{}", id)).unwrap();
    std::fs::write(format!("assignments/{}/assignment.toml", id), toml).unwrap();

    Ok("true".into())
}

async fn add_student(student_json: String) {

}
