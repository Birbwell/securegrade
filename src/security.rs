//! Contains the middleware security functions. Each layer checks for a different level of security, as denoted by the function

use axum::{
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use crate::database::auth::{
    session_exists_and_valid, session_is_admin, session_is_instructor, session_is_student,
};

/// Checks to see if the user is authenticated.
pub async fn handle_basic_auth(
    Path(path_params): Path<Vec<String>>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response<Body> {
    let (parts, body) = request.into_parts();

    let Some(auth_header) = parts.headers.get(&AUTHORIZATION) else {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
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

            let is_admin = session_is_admin(token.clone())
                .await
                .unwrap();
            let (is_instructor, is_student) = if let Some(class_number) = path_params.first() {
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

            resp
        }
        Ok(false) => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
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

/// Checks if the user is a authorized as a student (or an instructor) for the provided class.
/// If no class parameter is provided, fall through (for admin-related endpoints).
pub async fn handle_student_auth(
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

    if let Some(class_number) = path_params.first() {
        let is_auth =
            match session_is_student(class_number.clone(), token.clone()).await {
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

/// Check if the user is authorized as an instructor for the class.
/// If no class number is provided, fall through (for admin-related endpoints).
pub async fn handle_instructor_auth(
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

    if let Some(class_number) = path_params.first() {
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

/// Check if the user is authorized as an admin.
pub async fn handle_admin_auth(request: axum::http::Request<Body>, next: Next) -> Response<Body> {
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
