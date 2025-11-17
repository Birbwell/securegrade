use axum::{Json, body::Body, http::{Response, StatusCode}};

use crate::{database, model::request::ClientRequest};

pub async fn create_class(Json(client_req): Json<ClientRequest>) -> Response<Body> {
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