use axum::{
    Json,
    body::Body,
    extract::Path,
    http::{Response, StatusCode, header::CONTENT_TYPE},
};

use crate::{OK_JSON, database, model::request::ClientRequest};

pub async fn add_instructor(Json(client_req): Json<ClientRequest>) -> Response<Body> {
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

pub async fn download_submission(Path(path_params): Path<Vec<String>>) -> Response<Body> {
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

    let zip = database::assignment::download_submission(username.clone(), assignment_id)
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

pub async fn generate_join_code(Path(class_number): Path<String>) -> Response<Body> {
    let join_code = rand::random_iter::<u8>()
        .take(6)
        .map(|b| format!("{:X}", b % 16))
        .collect::<String>();

    database::operations::add_join_code(join_code.clone(), class_number)
        .await
        .unwrap();

    return Response::builder()
        .status(StatusCode::OK)
        .body(format!(r#"{{ "join_code": "{join_code}" }}"#).into())
        .unwrap();
}

pub async fn add_student(Json(client_req): Json<ClientRequest>) -> Response<Body> {
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

pub async fn retrieve_scores(Path(path_params): Path<Vec<String>>) -> Response<Body> {
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

    let scores = database::assignment::get_assignment_scores(assignment_id)
        .await
        .unwrap();

    let scores_json = serde_json::to_string(&scores).unwrap();
    return Response::builder()
        .status(StatusCode::OK)
        .body(scores_json.into())
        .unwrap();
}

pub async fn retrieve_full_assignment_info(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, assignment_id, ..] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid URL parameters.".into())
            .unwrap();
    };

    let Ok(assignment_id) = assignment_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid URL parameters.".into())
            .unwrap();
    };

    let full_assignment_info =
        match database::assignment::retrieve_full_assignment_info(assignment_id).await {
            Ok(fai) => serde_json::to_string(&fai).unwrap(),
            Err(e) => {
                tracing::error!(e);
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Internal Error".into())
                    .unwrap();
            }
        };

    return Response::builder()
        .status(StatusCode::OK)
        .body(full_assignment_info.into())
        .unwrap();
}

pub async fn add_assignment(
    Path(path_params): Path<Vec<String>>,
    Json(client_req): Json<ClientRequest>,
) -> Response<Body> {
    let [class_number, ..] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let ClientRequest {
        assignment_name: Some(assignment_name),
        assignment_description,
        deadline: Some(deadline),
        tasks: Some(tasks),
        ..
    } = client_req
    else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Missing required fields assignment_name or deadline.".into())
            .unwrap();
    };

    if let Err(e) = database::assignment::add_assignment(
        class_number.into(),
        assignment_name,
        assignment_description,
        deadline,
        tasks,
    )
    .await
    {
        tracing::error!("Could not add assignment: {e}");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Error.".into())
            .unwrap();
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap()
}

pub async fn update_assignment(
    Path(path_params): Path<Vec<String>>,
    Json(client_req): Json<ClientRequest>,
) -> Response<Body> {
    let [_, assignment_id, ..] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Missing assignment_id URL parameter.".into())
            .unwrap();
    };

    let Ok(assignment_id) = assignment_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid assignment_id parameter.".into())
            .unwrap();
    };

    let ClientRequest {
        assignment_name: Some(assignment_name),
        assignment_description,
        deadline: Some(deadline),
        tasks: Some(tasks),
        ..
    } = client_req
    else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    if let Err(e) = database::assignment::update_assignment(
        assignment_id,
        assignment_name,
        assignment_description,
        deadline,
        tasks,
    )
    .await {
        tracing::error!(e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Internal Error.".into())
            .unwrap();
    };

    Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap()
}
