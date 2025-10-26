use std::net::SocketAddr;
use std::sync::Mutex;

use axum::routing::{get, post, put};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use serde_json::Value;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::assignment::Assignment;
use crate::container::run_container;
use crate::database::auth::session::Session;
use crate::model::add_to_class_object::AddToClassObject;
use crate::model::login_object::LoginObject;
use crate::model::new_class_object::NewClassObject;
use crate::model::new_user_object::NewUserObject;
use crate::model::response_object::ResponseObject;
use crate::model::submission_object::SubmissionObject;

mod assignment;
mod container;
mod database;
mod image;
mod model;

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
        .route("/login", get(login))
        .route("/signup", post(signup))
        .route("/instructor/add_instructor", post(add_instructor))
        .route("/instructor/add_student", post(add_student))
        .route("/admin/create_class", post(create_class));

    let config =
        RustlsConfig::from_pem_file("aeskul.net_certificate.cer", "aeskul.net_private_key.key")
            .await
            .unwrap();

    if let Err(e) = database::init_database().await {
        tracing::error!("{}", e);
        return;
    };

    info!("Database initialized");

    let server = axum_server::bind_rustls("0.0.0.0:443".parse::<SocketAddr>().unwrap(), config);
    server.serve(app.into_make_service()).await.unwrap()
}

async fn receive_submission(json_submission: String) -> Result<Json<ResponseObject>, String> {
    let sub_ob = serde_json::from_str::<SubmissionObject>(&json_submission).unwrap();
    Ok(Json(run_container(sub_ob).await?))
}

async fn add_assignment(toml: String) -> Result<String, String> {
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

async fn create_class(class_data: String) -> Result<String, String> {
    let class_obj = serde_json::from_str::<NewClassObject>(&class_data).unwrap();
    if let Err(e) = database::operations::new_class(class_obj).await {
        return Err(format!("Could not create new class: {e}"));
    };
    Ok("true".into())
}

async fn add_student(instructor_data: String) -> Result<String, String> {
    let student_obj = serde_json::from_str::<AddToClassObject>(&instructor_data).unwrap();
    if let Err(e) = database::operations::add_student(student_obj).await {
        return Err(format!("Could not add instructor: {e}"));
    }
    Ok("true".into())
}

async fn add_instructor(instructor_data: String) -> Result<String, String> {
    let instructor_obj = serde_json::from_str::<AddToClassObject>(&instructor_data).unwrap();
    if let Err(e) = database::operations::add_instructor(instructor_obj).await {
        return Err(format!("Could not add instructor: {e}"));
    }
    Ok("true".into())
}

async fn login(login_data: String) -> Result<Json<Session>, String> {
    let login_obj = serde_json::from_str::<LoginObject>(&login_data).unwrap();
    let session_token = database::user::login_user(login_obj).await.unwrap();
    Ok(Json(Session::new(session_token)))
}

async fn signup(signup_data: String) -> Result<Json<Session>, String> {
    info!("Signup Request Received");
    let signin_obj = serde_json::from_str::<NewUserObject>(&signup_data).unwrap();
    let session_token = database::user::register_user(signin_obj).await.unwrap();
    Ok(Json(Session::new(session_token)))
}
