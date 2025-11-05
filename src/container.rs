use std::{
    collections::VecDeque,
    fs::{copy, create_dir_all, read_dir, read_to_string, remove_dir_all, write},
    path::PathBuf,
    process::Command,
    sync::LazyLock,
};

use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::{
    assignment::Assignment, database, image::ImageBuilder,
    model::submission_response::SubmissionResponse,
};

// Supported Languages
// pub enum Language {
//     Python311,
//     Python313,
//     Rust,
//     Java,
//     C,
//     Cpp,
// }

pub struct ContainerEntry {
    zip_file: axum::body::Bytes,
    user_id: i32,
    assignment_id: i32,
    lang: String,
}

impl ContainerEntry {
    pub fn new(
        zip_file: axum::body::Bytes,
        user_id: i32,
        assignment_id: i32,
        lang: impl Into<String>,
    ) -> Self {
        Self {
            zip_file,
            user_id,
            assignment_id,
            lang: lang.into(),
        }
    }
}

pub async fn add_to_container_queue(container_entry: ContainerEntry) {
    // let mut container_queue = CONTAINER_QUEUE.write().await;
    // container_queue.push_front(container_entry);
}

pub async fn container_queue(mut rx: tokio::sync::mpsc::Receiver<ContainerEntry>) -> ! {
    // let mut semaphore = Semaphore::new(5);
    static SEMAPHORE: Semaphore = Semaphore::const_new(10);
    loop {
        if let Ok(perm) = SEMAPHORE.acquire().await
            && let Some(container) = rx.recv().await
        {
            tokio::spawn(async move {
                let user_id = container.user_id;
                let assignment_id = container.assignment_id;
                let Ok(results) = run_container(container).await else {
                    drop(perm);
                    tracing::error!("Unable to run container");

                    // Log error in psql

                    return;
                };
                drop(perm);

                let json_results = serde_json::to_vec(&results).unwrap();

                database::operations::container_add_grade(
                    user_id,
                    assignment_id,
                    &json_results,
                    results.score(),
                )
                .await
                .unwrap();
            });
        } else {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }
}

async fn run_container(
    ContainerEntry {
        zip_file,
        user_id,
        assignment_id,
        lang,
    }: ContainerEntry,
) -> Result<SubmissionResponse, String> {
    let Some(container) = get_container_for_language(&lang) else {
        error!("No container found for language: {}", lang);
        // Log error in database
        return Err("Language not supported".into());
    };

    let workdir = format!("/tmp/securegrade/{}-{}", user_id, assignment_id);

    // Delete and recreate working directory
    let _ = remove_dir_all(&workdir);
    create_dir_all(&workdir).unwrap();

    copy(
        container.join("Dockerfile"),
        format!("{}/Dockerfile", workdir),
    )
    .unwrap();

    std::fs::write(&format!("{workdir}/submission.zip"), zip_file).unwrap();
    Command::new("unzip")
        .args([
            &format!("{workdir}/submission.zip"),
            "-d",
            &format!("{workdir}/submission"),
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let assignment_dir = format!("assignments/{}", assignment_id);
    let toml_assignment = read_to_string(format!("{}/assignment.toml", assignment_dir)).unwrap();
    let assignment = toml::from_str::<Assignment>(&toml_assignment).unwrap();

    let image = ImageBuilder::new(&workdir).build().unwrap();
    info!("Removing working directory {workdir}");
    remove_dir_all(&workdir).unwrap();

    // let mut test_results = ResponseObject::default();
    let mut test_results = SubmissionResponse::default();

    for (test_name, test) in &assignment.tests {
        let input = if let Some(input_file) = &test.input_file {
            if test.input.is_some() {
                warn!(
                    "Assignment {}, {}: Both input and input_file defined. Defaulting to input_file.",
                    assignment_id, test_name
                );
            }

            read_to_string(format!("{}/{}", assignment_dir, input_file)).unwrap()
        } else {
            test.input.clone().unwrap()
        };

        let output = if let Some(output_file) = &test.output_file {
            if test.output.is_some() {
                warn!(
                    "Assignment {}, {}: Both output and output_file defined. Defaulting to output_file.",
                    assignment_id, test_name
                );
            }

            read_to_string(format!("{}/{}", assignment_dir, output_file)).unwrap()
        } else {
            test.output.clone().unwrap()
        };

        let container_output = match image.exec(input, assignment.get_timeout()).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                // test_results.time_out(test_name);
                // warn!("Timed Out");
                test_results.time_out(test_name);
                continue;
            }
            Err(e) => {
                // test_results.err(test_name, e);
                // warn!("Container Error: {e}");
                test_results.err(test_name, e);
                continue;
            }
        };

        if container_output.trim() == output.trim() {
            // info!("Assignment {}, {} :: OK", sub_ob.assignment_id, test_name);
            test_results.pass(test_name);
        } else {
            // info!(
            //     "Assignment {}, {} :: Expected {:?}, found {:?}",
            //     sub_ob.assignment_id,
            //     test_name,
            //     output.trim(),
            //     container_output.trim()
            // );
            // if test.public {
            //     test_results.pub_fail(
            //         test_name,
            //         input.trim(),
            //         output.trim(),
            //         container_output.trim(),
            //     );
            // } else {
            //     test_results.fail(test_name);
            // }
            test_results.fail(test_name);
        }
    }

    Ok(test_results)
    // Store test_results in database
}

fn get_container_for_language(lang: impl AsRef<str>) -> Option<PathBuf> {
    let containers = read_dir("dockerfiles").unwrap();
    for container_dir in containers.filter_map(|f| f.ok()) {
        if container_dir.file_name() == lang.as_ref() {
            return Some(container_dir.path());
        }
    }

    None
}
