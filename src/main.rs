use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::usize;

use axum::body::{self, Body};
use axum::extract::Path;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use crate::container::ContainerEntry;
// use crate::container::run_container;
use crate::database::auth::{
    Session, session_exists_and_valid, session_is_admin, session_is_instructor, session_is_student,
};
use crate::database::operations::{submission_in_progress, container_retrieve_grade};
use crate::model::assignment_item::AssignmentItem;
use crate::model::class_item::ClassItem;
use crate::model::request::Request;
use crate::model::simple_response::SimpleResponse;

mod assignment;
mod container;
mod database;
mod image;
mod model;

static TX: OnceLock<tokio::sync::mpsc::Sender<ContainerEntry>> = OnceLock::new();

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(AllowOrigin::any());

    let app: Router = Router::new()
        .route("/api/admin/create_class", post(create_class))
        .layer(from_fn(handle_admin_auth)) //^^ Admin Layer
        // .route("/api/instructor/{class_number}/add_instructor", post(add_instructor))
        // .route("/api/instructor/{class_number}/add_student", post(add_student))
        .route(
            "/api/instructor/{class_number}/add_assignment",
            post(add_assignment),
        )
        // .route("/api/update_assignment", put(update_assignment))
        .layer(from_fn(handle_instructor_auth)) //^^ Instructor Layer
        .route(
            "/api/student/{class_number}/{assignment_id}/submit",
            post(handle_submission),
        )
        .route(
            "/api/student/{class_number}/{assignment_id}/retrieve_score",
            get(retrieve_score)
        )
        .route(
            "/api/student/{class_number}/{assignment_id}",
            get(get_assignment),
        )
        .route("/api/student/{class_number}", get(get_assignments))
        .layer(from_fn(handle_student_auth)) //^^ Student Layer
        .route("/api/get_classes", get(get_classes))
        // .route("/api/validate", get(validate))
        .layer(from_fn(handle_basic_auth)) //^^ User Layer
        .route("/api/login", post(login))
        .route("/api/signup", post(signup))
        .layer(cors);

    let config =
        RustlsConfig::from_pem_file("aeskul.net_certificate.cer", "aeskul.net_private_key.key")
            .await
            .unwrap();

    if let Err(e) = database::init_database().await {
        tracing::error!("{}", e);
        return;
    };

    info!("Database initialized");

    let (tx, rx) = tokio::sync::mpsc::channel::<ContainerEntry>(i32::MAX as usize);

    tokio::spawn(async move {
        container::container_queue(rx).await;
    });

    let _ = TX.set(tx).unwrap();

    let server = axum_server::bind_rustls("0.0.0.0:9090".parse::<SocketAddr>().unwrap(), config);
    server.serve(app.into_make_service()).await.unwrap();
}

async fn handle_basic_auth(
    // class_number: Option<Path<(String, Option<String>)>>,
    Path(path_params): Path<Vec<String>>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header
        .as_bytes()
        .iter()
        .map(|c| *c as char)
        .collect::<String>();

    match session_exists_and_valid(token.clone()).await {
        Ok(true) => {
            let req = axum::http::Request::from_parts(parts, body);
            let resp = next.run(req).await;

            let is_admin = session_is_admin(token.clone()).await.unwrap();
            let (is_instructor, is_student) = if let Some(class_number) = path_params.get(0) {
                (
                    session_is_instructor(class_number.clone(), token.clone())
                        .await
                        .unwrap(),
                    session_is_student(class_number.clone(), token.clone())
                        .await
                        .unwrap(),
                )
            } else {
                (false, false)
            };

            let (resp_parts, resp_body) = resp.into_parts();

            let resp_body_str = axum::body::to_bytes(resp_body, usize::MAX)
                .await
                .unwrap()
                .iter()
                .map(|u| *u as char)
                .collect::<String>();

            let new_resp =
                SimpleResponse::new(resp_body_str, is_admin, is_instructor, is_student, true);

            let new_resp_body = serde_json::to_string(&new_resp).unwrap();
            return Response::from_parts(resp_parts, Body::new(new_resp_body));
        }
        Ok(false) => Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Not Authorized.".into())
            .unwrap(),
        Err(e) => {
            tracing::error!("{e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error.".into())
                .unwrap()
        }
    }
}

async fn handle_student_auth(
    Path(path_params): Path<Vec<String>>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header
        .as_bytes()
        .iter()
        .map(|u| *u as char)
        .collect::<String>();

    if let Some(class_number) = path_params.get(0) {
        let is_auth = match session_is_student(class_number.clone(), token.clone()).await {
            Ok(t) => t,
            Err(e) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(e.into())
                    .unwrap();
            }
        };

        let is_auth = is_auth
            || match session_is_instructor(class_number.clone(), token).await {
                Ok(t) => t,
                Err(e) => {
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(e.into())
                        .unwrap();
                }
            };

        let req = axum::http::Request::from_parts(parts, Body::new(body));

        if is_auth {
            next.run(req).await
        } else {
            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("Not Authorized.".into())
                .unwrap()
        }
    } else {
        let is_auth = match session_is_admin(token).await {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("{e}");
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Internal Server Error.".into())
                    .unwrap();
            }
        };

        if is_auth {
            let req = axum::http::Request::from_parts(parts, body);
            next.run(req).await
        } else {
            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("Not Authorized.".into())
                .unwrap()
        }
    }
}

async fn handle_instructor_auth(
    path_params: Path<Vec<String>>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header
        .as_bytes()
        .iter()
        .map(|u| *u as char)
        .collect::<String>();

    if let Some(class_number) = path_params.get(0) {
        let is_auth = match session_is_instructor(class_number.clone(), token).await {
            Ok(t) => t,
            Err(e) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(e.into())
                    .unwrap();
            }
        };

        let req = axum::http::Request::from_parts(parts, Body::new(body));

        if is_auth {
            next.run(req).await
        } else {
            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("Not Authorized.".into())
                .unwrap()
        }
    } else {
        let is_auth = match session_is_admin(token).await {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("{e}");
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Internal Server Error.".into())
                    .unwrap();
            }
        };

        if is_auth {
            let req = axum::http::Request::from_parts(parts, body);
            next.run(req).await
        } else {
            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body("Not Authorized.".into())
                .unwrap()
        }
    }
}

async fn handle_admin_auth(request: axum::http::Request<Body>, next: Next) -> Response {
    let Some(auth_header) = request.headers().get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header
        .as_bytes()
        .iter()
        .map(|c| *c as char)
        .collect::<String>();

    match session_is_admin(token).await {
        Ok(true) => next.run(request).await,
        Ok(false) => Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Not Authorized.".into())
            .unwrap(),
        Err(e) => {
            tracing::error!("{e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error.".into())
                .unwrap()
        }
    }
}

async fn create_class(class_data: String) -> Result<String, String> {
    let class_obj = serde_json::from_str::<Request>(&class_data).unwrap();
    if let Err(e) = database::operations::new_class(class_obj).await {
        return Err(format!("Could not create class: {e}"));
    };
    Ok("OK".into())
}

async fn handle_submission(
    Path(path_params): Path<Vec<String>>,
    req: axum::http::Request<Body>,
) -> Response {
    let [_, assignment_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request".into())
            .unwrap();
    };

    let assignment_id = assignment_id.parse::<i32>().unwrap();

    let (parts, body) = req.into_parts();
    let zip_file = axum::body::to_bytes(body, usize::MAX).await.unwrap();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header.to_str().unwrap().to_owned();

    let user_id = database::user::get_user_from_session(token).await.unwrap();

    if submission_in_progress(user_id, assignment_id).await {
        return Response::builder()
            .status(StatusCode::PROCESSING)
            .body("Previous submission still in queue. Check for results later.".into())
            .unwrap();
    }

    let container_entry = ContainerEntry::new(zip_file, user_id, assignment_id, "python313");

    // Add to container queue
    if let Some(tx) = TX.get() && let Ok(perm) = tx.reserve().await {
        perm.send(container_entry);
    } else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Could not add submission to queue".into())
            .unwrap();
    }

    // let results_json = serde_json::to_string(&results).unwrap();
    // TODO Store results in database

    return Response::builder()
        .status(StatusCode::OK)
        // .body(Body::new(json_out))
        .body("Submission Received.".into())
        .unwrap();
}

async fn retrieve_score(Path(path_params): Path<Vec<String>>, req: axum::http::Request<Body>) -> Response {
    let (parts, _) = req.into_parts();
    let Some(auth_header) = parts.headers.get(AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Access Denied.".into())
            .unwrap();
    };
    
    let [_, assignment_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid URL".into())
            .unwrap();
    };

    let token = auth_header.to_str().unwrap().to_string();
    let Some(user_id) = database::user::get_user_from_session(token).await else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Access Denied.".into())
            .unwrap();
    };

    let Ok(assignment_id) = assignment_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid URL".into())
            .unwrap();
    };

    return container_retrieve_grade(user_id, assignment_id).await;
}

async fn add_assignment(
    Path(path_params): Path<Vec<String>>,
    req: axum::http::Request<Body>,
) -> Response {
    let body = req.into_body();
    let body_str = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap()
        .iter()
        .map(|u| *u as char)
        .collect::<String>();

    let [class_number, ..] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let req = serde_json::from_str::<Request>(&body_str).unwrap();
    if let (Some(assignment_name), Some(assignment_description)) =
        (req.assignment_name, req.assignment_description)
    {
        if let Err(e) = database::operations::add_assignment(
            class_number.into(),
            assignment_name,
            assignment_description,
        )
        .await
        {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(e.into())
                .unwrap();
        };

        return Response::builder()
            .status(StatusCode::OK)
            .body("OK".into())
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Resource not Found".into())
        .unwrap()
}

async fn get_assignment(
    Path(path_params): Path<Vec<String>>,
) -> Result<Json<AssignmentItem>, String> {
    // let [class_number, assignment_id] = &path_params.get(0..2)[0..2] else {
    //     return Err("Bad Request".into());
    // };

    let [class_number, assignment_id] = &path_params[..] else {
        return Err("Bad Request".into());
    };
    let assignment_id = assignment_id.parse::<i32>().unwrap();
    let ass = database::operations::get_assignment(class_number.clone(), assignment_id)
        .await
        .unwrap();

    Ok(Json(ass))
}

async fn get_assignments(
    Path(path_params): Path<Vec<String>>,
    req: axum::http::Request<Body>,
) -> Result<Json<Vec<AssignmentItem>>, String> {
    if let Some(class_number) = path_params.get(0) {
        let assignments = database::operations::get_assignments(class_number.clone())
            .await
            .unwrap();

        Ok(Json(assignments))
    } else {
        Err("Bad Request".into())
    }
}

async fn get_classes(req: axum::http::Request<Body>) -> Result<Json<Vec<ClassItem>>, String> {
    let (parts, _) = req.into_parts();

    let auth_header = parts.headers.get(&AUTHORIZATION).unwrap().to_str().unwrap();
    let user_id = database::user::get_user_from_session(auth_header)
        .await
        .unwrap();

    let class_items = database::operations::get_classes(user_id).await.unwrap();

    return Ok(Json(class_items));
}

async fn add_student(instructor_data: String) -> Result<String, String> {
    let student_obj = serde_json::from_str::<Request>(&instructor_data).unwrap();
    if let Err(e) = database::operations::add_student(student_obj).await {
        return Err(format!("Could not add instructor: {e}"));
    }
    Ok("OK".into())
}

async fn add_instructor(instructor_data: String) -> Result<String, String> {
    let instructor_obj = serde_json::from_str::<Request>(&instructor_data).unwrap();
    if let Err(e) = database::operations::add_instructor(instructor_obj).await {
        return Err(format!("Could not add instructor: {e}"));
    }
    Ok("OK".into())
}

async fn login(login_data: String) -> Result<Json<Session>, Json<String>> {
    let login_obj = serde_json::from_str::<Request>(&login_data).unwrap();
    match database::user::login_user(login_obj).await {
        Ok(s) => Ok(Json(Session::new(s))),
        Err(e) => Err(Json(e)),
    }
}

async fn signup(signup_data: String) -> Result<Json<Session>, Json<String>> {
    let Ok(signup_obj) = serde_json::from_str::<Request>(&signup_data) else {
        return Err(Json("Improperly formatted data".into()));
    };

    match database::user::register_user(signup_obj).await {
        Ok(s) => Ok(Json(Session::new(s))),
        Err(e) => Err(Json(e)),
    }
}
