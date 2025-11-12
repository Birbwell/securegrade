use std::env::var;
use std::net::SocketAddr;
use std::sync::OnceLock;

use axum::body::Body;
use axum::extract::Path;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, Method, Response, StatusCode};
use axum::middleware::{Next, from_fn};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use chrono::Utc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use crate::container::ContainerEntry;
use crate::database::auth::{
    Session, session_exists_and_valid, session_is_admin, session_is_instructor, session_is_student,
};
use crate::model::class_info::ClassInfo;
use crate::model::request::ClientRequest;

mod container;
mod database;
mod image;
mod model;

const OK_JSON: &'static str = r#"{ "message": "OK" }"#;

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
        .allow_origin(AllowOrigin::any())
        .expose_headers([
            CONTENT_TYPE,
            HeaderName::from_lowercase(b"admin").unwrap(),
            HeaderName::from_lowercase(b"instructor").unwrap(),
            HeaderName::from_lowercase(b"student").unwrap(),
        ]);

    let app: Router = Router::new()
        .route("/api/admin/create_class", post(create_class))
        .layer(from_fn(handle_admin_auth)) //^^ Admin Layer
        .route(
            "/api/instructor/{class_number}/add_instructor",
            put(add_instructor),
        )
        .route(
            "/api/instructor/{class_number}/{assignment_number}/download/{username}",
            get(download_submission),
        )
        .route(
            "/api/instructor/{class_number}/{assignment_number}/retrieve_scores",
            get(retrieve_scores),
        )
        .route(
            "/api/instructor/{class_number}/add_assignment",
            post(add_assignment),
        )
        // .route("/api/update_assignment", put(update_assignment))
        .route(
            "/api/instructor/{class_number}/add_student",
            put(add_student),
        )
        .route(
            "/api/instructor/{class_number}/list_all_students",
            get(list_all_students),
        )
        .layer(from_fn(handle_instructor_auth)) //^^ Instructor Layer
        .route(
            "/api/student/{class_number}/{assignment_id}/{task_id}/submit",
            post(handle_submission),
        )
        .route(
            "/api/student/{class_number}/{assignment_id}/{task_id}/retrieve_score",
            get(retrieve_task_score),
        )
        .route(
            "/api/student/{class_number}/{assignment_id}",
            get(get_assignment),
        )
        .route("/api/student/{class_number}", get(get_class_info))
        .layer(from_fn(handle_student_auth)) //^^ Student Layer
        .route("/api/get_classes", get(get_classes))
        .route("/api/list_all_students", get(list_all_students))
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

    let n_threads = var("NTHREADS").ok().and_then(|f| f.parse::<usize>().ok());

    tokio::spawn(async move {
        container::container_queue(rx, n_threads).await;
    });

    let _ = TX.set(tx).unwrap();

    let server = axum_server::bind_rustls("0.0.0.0:9090".parse::<SocketAddr>().unwrap(), config);
    server.serve(app.into_make_service()).await.unwrap();
}

async fn handle_basic_auth(
    Path(path_params): Path<Vec<String>>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response<Body> {
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
            let mut resp = next.run(req).await;

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

            resp.headers_mut().insert(
                "admin",
                HeaderValue::from_str(&is_admin.to_string()).unwrap(),
            );
            resp.headers_mut().insert(
                "instructor",
                HeaderValue::from_str(&is_instructor.to_string()).unwrap(),
            );
            resp.headers_mut().insert(
                "student",
                HeaderValue::from_str(&is_student.to_string()).unwrap(),
            );

            return resp;
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
) -> Response<Body> {
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
) -> Response<Body> {
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

async fn handle_admin_auth(request: axum::http::Request<Body>, next: Next) -> Response<Body> {
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

async fn create_class(Json(client_req): Json<ClientRequest>) -> Response<Body> {
    if let Err(e) = database::operations::new_class(client_req).await {
        tracing::error!("Could not create class: {e}");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Error".into())
            .unwrap();
    };
    return Response::builder()
        .status(StatusCode::OK)
        .body("OK".into())
        .unwrap();
}

async fn handle_submission(
    Path(path_params): Path<Vec<String>>,
    parts: Parts,
    zip_file: axum::body::Bytes,
) -> Response<Body> {
    let submission_time = Utc::now();
    let [_, assignment_id, task_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request".into())
            .unwrap();
    };

    let assignment_id = assignment_id.parse::<i32>().unwrap();
    let task_id = task_id.parse::<i32>().unwrap();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Not Authorized".to_string()))
            .unwrap();
    };

    let token = auth_header.to_str().unwrap().to_owned();
    let user_id = database::user::get_user_from_session(token).await.unwrap();

    if database::assignment::operations::submission_in_progress(user_id, assignment_id).await {
        return Response::builder()
            .status(StatusCode::TOO_EARLY)
            .body("Previous submission still in queue. Check for results later.".into())
            .unwrap();
    }

    if let Err(e) = database::assignment::operations::remove_old_grade(user_id, task_id).await {
        tracing::error!(e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(e.into())
            .unwrap();
    }

    let was_late = match database::assignment::operations::mark_as_submitted(
        user_id,
        assignment_id,
        task_id,
        submission_time,
        zip_file.clone(),
    )
    .await
    {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("{e}");
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Error".into())
                .unwrap();
        }
    };

    let container_entry = ContainerEntry::new(zip_file, user_id, task_id, was_late, "python");

    // Add to container queue
    if let Some(tx) = TX.get()
        && let Ok(perm) = tx.reserve().await
    {
        perm.send(container_entry);
    } else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Could not add submission to queue".into())
            .unwrap();
    }

    return Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap();
}

async fn retrieve_task_score(Path(path_params): Path<Vec<String>>, parts: Parts) -> Response<Body> {
    let Some(auth_header) = parts.headers.get(AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body("Access Denied.".into())
            .unwrap();
    };

    let [_, _, task_id] = &path_params[..] else {
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

    let Ok(task_id) = task_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid Request.".into())
            .unwrap();
    };

    if database::assignment::operations::submission_in_progress(user_id, task_id).await {
        return Response::builder()
            .status(StatusCode::TOO_EARLY)
            .body("Submission in progress".into())
            .unwrap();
    }

    match database::assignment::operations::get_task_score(user_id, task_id).await {
        Ok(Some(res)) => {
            let res_json = serde_json::to_string(&res).unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .body(res_json.into())
                .unwrap()
        }
        Ok(None) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not Found.".into())
            .unwrap(),
        Err(e) => {
            tracing::error!("{e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Error.".into())
                .unwrap()
        }
    }
}

async fn add_assignment(
    Path(path_params): Path<Vec<String>>,
    Json(client_req): Json<ClientRequest>,
) -> Response<Body> {
    let [class_number, ..] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    if let Some(assignment_name) = client_req.assignment_name {
        if let Err(e) = database::assignment::operations::add_assignment(
            class_number.into(),
            assignment_name,
            client_req.assignment_description,
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

async fn get_assignment(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, assignment_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request".into())
            .unwrap();
    };
    let assignment_id = assignment_id.parse::<i32>().unwrap();
    let ass = database::assignment::operations::get_assignment_info(assignment_id)
        .await
        .unwrap();

    let ass_json = serde_json::to_string(&ass).unwrap();

    return Response::builder()
        .status(StatusCode::OK)
        .body(ass_json.into())
        .unwrap();
}

async fn get_class_info(Path(path_params): Path<Vec<String>>, parts: Parts) -> Response<Body> {
    let token = parts
        .headers
        .get("Authorization")
        .unwrap()
        .to_str()
        .unwrap();

    let user_id = database::user::get_user_from_session(token).await.unwrap();

    if let Some(class_number) = path_params.get(0) {
        let assignments = database::assignment::operations::get_assignments_for_class(
            class_number.clone(),
            user_id,
        )
        .await
        .unwrap();

        let instructors = database::operations::get_instructors(class_number)
            .await
            .unwrap();

        let class_info = ClassInfo::new(assignments, instructors);

        let class_json = serde_json::to_string(&class_info).unwrap();

        return Response::builder()
            .status(StatusCode::OK)
            .body(class_json.into())
            .unwrap();
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    }
}

async fn retrieve_scores(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, assignment_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let Ok(assignment_id) = assignment_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let scores = database::assignment::operations::get_assignment_scores(assignment_id)
        .await
        .unwrap();

    let scores_json = serde_json::to_string(&scores).unwrap();
    return Response::builder()
        .status(StatusCode::OK)
        .body(scores_json.into())
        .unwrap();
}

async fn download_submission(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, assignment_id, username] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let Ok(assignment_id) = assignment_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let zip =
        database::assignment::operations::download_submission(username.clone(), assignment_id)
            .await
            .unwrap();

    let Some(zip) = zip else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Nothing to download.".into())
            .unwrap();
    };

    return Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/zip")
        .body(zip.into())
        .unwrap();
}

async fn get_classes(parts: Parts) -> Response<Body> {
    let auth_header = parts.headers.get(&AUTHORIZATION).unwrap().to_str().unwrap();
    let user_id = database::user::get_user_from_session(auth_header)
        .await
        .unwrap();

    let class_items = database::operations::get_classes(user_id).await.unwrap();
    let class_items_json = serde_json::to_string(&class_items).unwrap();

    return Response::builder()
        .status(StatusCode::OK)
        .body(class_items_json.into())
        .unwrap();
}

async fn add_student(Json(client_req): Json<ClientRequest>) -> Response<Body> {
    if let Err(e) = database::operations::add_student(client_req).await {
        tracing::error!("Could not add instructor: {e}");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Error.".into())
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap()
}

async fn list_all_students(class_number: Option<Path<String>>) -> Response<Body> {
    let class_number = class_number.and_then(|f| Some(f.0));

    let user_info = match database::operations::list_all_students(class_number).await {
        Ok(user_info) => user_info,
        Err(e) => {
            tracing::error!(e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error.".into())
                .unwrap();
        }
    };

    let users_json = serde_json::to_string(&user_info).unwrap();

    return Response::builder()
        .status(StatusCode::OK)
        .body(users_json.into())
        .unwrap();
}

async fn add_instructor(Json(client_req): Json<ClientRequest>) -> Response<Body> {
    if let Err(e) = database::operations::add_instructor(client_req).await {
        tracing::error!("Could not add instructor: {e}");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Error.".into())
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap()
}

async fn login(Json(login_req): Json<ClientRequest>) -> Response<Body> {
    match database::user::login_user(login_req).await {
        Ok(s) => {
            let session = Session::new(s);
            let session_json = serde_json::to_string(&session).unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .body(session_json.into())
                .unwrap()
        }
        Err(e) => {
            tracing::error!("{e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Error".into())
                .unwrap()
        }
    }
}

async fn signup(Json(signup_req): Json<ClientRequest>) -> Response<Body> {
    match database::user::register_user(signup_req).await {
        Ok(s) => {
            let session = Session::new(s);
            let session_json = serde_json::to_string(&session).unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .body(session_json.into())
                .unwrap()
        }
        Err(e) => {
            tracing::error!("{e}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Error".into())
                .unwrap()
        }
    }
}
