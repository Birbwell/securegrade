//! Contains all endpoint-associated function. These are grouped depending on the security level required to access them
//! 
//! The endpoints requiring no authentication and no authorization are here, and the endpoints requiring higher levels of authorization are in the `student`, `instructor`, and `admin` submodules respectively.

use axum::{
    Json,
    body::Body,
    extract::Path,
    http::{Response, StatusCode, header::AUTHORIZATION, request::Parts},
};

use crate::{
    OK_JSON,
    database::{self, auth::Session},
    model::request::ClientRequest,
};

pub mod admin;
pub mod instructor;
pub mod student;

/// Adds the user to a class as a student, using the provided join code
/// 
/// Uses the Authorization header to determine the submitter's user id, so it also accepts a `Parts` parameter
pub async fn join_class(parts: Parts, Json(client_req): Json<ClientRequest>) -> Response<Body> {
    let ClientRequest {
        join_code: Some(join_code),
        ..
    } = client_req
    else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let join_code = join_code.to_uppercase();

    let session_base = parts
        .headers
        .get(&AUTHORIZATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    let user_id = database::user::get_user_from_session(session_base)
        .await
        .unwrap();

    match database::operations::join_class(user_id, join_code).await {
        Ok(true) => Response::builder()
            .status(StatusCode::OK)
            .body(OK_JSON.into())
            .unwrap(),
        Ok(false) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Invalid Join Code.".into())
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

/// Gets all classes associated with a user
/// 
/// Determines the user from the Authorization header, so it accepts a `Parts` parameter
pub async fn get_classes(parts: Parts) -> Response<Body> {
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

/// Lists all the students using the platform. Instructors use this to facilitate with auto completion.
/// 
/// A class_number can be optionally provided to exclude students from that class (as they do not need to be in the auto complete)
pub async fn list_all_students(class_number: Option<Path<String>>) -> Response<Body> {
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

/// Returns a list of languages the backend supports
/// 
/// This way the frontend does not need to be statically updated with languages when new ones are added
pub async fn supported_languages() -> Response<Body> {
    let Ok(dir) = std::fs::read_dir("dockerfiles") else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Server Error.".into())
            .unwrap();
    };

    let items = dir
        .filter_map(|f| f.ok())
        .filter_map(|f| f.file_name().into_string().ok())
        .collect::<Vec<String>>();

    let item_json = serde_json::to_string(&items).unwrap();

    return Response::builder()
        .status(StatusCode::OK)
        .body(item_json.into())
        .unwrap();
}

/// Logins a user provided their username and password
/// 
/// Returns a session token to be used for subsequent operations. By default, this token expires after an hour.
pub async fn login(Json(login_req): Json<ClientRequest>) -> Response<Body> {
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


/// Signs up a new user with the provided credentials
/// 
/// Returns a session token to be used for subsequent operations. By default, it expires after an hour.
pub async fn signup(Json(signup_req): Json<ClientRequest>) -> Response<Body> {
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
