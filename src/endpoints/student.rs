use axum::{
    body::Body,
    extract::Path,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::Response,
};
use chrono::Utc;

use crate::{OK_JSON, SupplementaryMaterial, TX, container::ContainerEntry, database, model::class_info::ClassInfo};

pub async fn download_material(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, _, task_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let Ok(task_id) = task_id.parse::<i32>() else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap();
    };

    let material = database::assignment::download_material(task_id)
        .await
        .unwrap();

    let Some((material, filename)) = material else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("No material found.".into())
            .unwrap();
    };

    let material_resp = SupplementaryMaterial { material, filename };
    let material_resp_json = serde_json::to_string(&material_resp).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .body(material_resp_json.into())
        .unwrap()
}

pub async fn handle_submission(
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
            .body("Not Authorized".into())
            .unwrap();
    };

    let Some(lang) = parts
        .headers
        .get("Language")
        .and_then(|f| f.to_str().map(|f| f.to_owned()).ok())
    else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Language Header Missing".into())
            .unwrap();
    };

    let token = auth_header.to_str().unwrap().to_owned();
    let user_id = database::user::get_user_from_session(token).await.unwrap();

    if database::assignment::submission_in_progress(user_id, assignment_id).await {
        return Response::builder()
            .status(StatusCode::TOO_EARLY)
            .body("Previous submission still in queue. Check for results later.".into())
            .unwrap();
    }

    if let Err(e) = database::assignment::remove_old_grade(user_id, task_id).await {
        tracing::error!(e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(e.into())
            .unwrap();
    }

    let was_late = match database::assignment::mark_as_submitted(
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

    let container_entry = ContainerEntry::new(zip_file, user_id, task_id, was_late, lang);

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

    Response::builder()
        .status(StatusCode::OK)
        .body(OK_JSON.into())
        .unwrap()
}

pub async fn retrieve_task_score(
    Path(path_params): Path<Vec<String>>,
    parts: Parts,
) -> Response<Body> {
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

    if database::assignment::submission_in_progress(user_id, task_id).await {
        return Response::builder()
            .status(StatusCode::TOO_EARLY)
            .body("Submission in progress".into())
            .unwrap();
    }

    match database::assignment::get_task_score(user_id, task_id).await {
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

pub async fn get_assignment(Path(path_params): Path<Vec<String>>) -> Response<Body> {
    let [_, assignment_id] = &path_params[..] else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request".into())
            .unwrap();
    };
    let assignment_id = assignment_id.parse::<i32>().unwrap();
    let ass = database::assignment::get_assignment_info(assignment_id)
        .await
        .unwrap();

    let ass_json = serde_json::to_string(&ass).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .body(ass_json.into())
        .unwrap()
}

pub async fn get_class_info(Path(path_params): Path<Vec<String>>, parts: Parts) -> Response<Body> {
    let token = parts
        .headers
        .get("Authorization")
        .unwrap()
        .to_str()
        .unwrap();

    let user_id = database::user::get_user_from_session(token).await.unwrap();

    if let Some(class_number) = path_params.first() {
        let assignments = database::assignment::get_assignments_for_class(
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

        Response::builder()
            .status(StatusCode::OK)
            .body(class_json.into())
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Bad Request.".into())
            .unwrap()
    }
}
