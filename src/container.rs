use std::{
    fs::{copy, create_dir_all, read_dir, remove_dir_all},
    path::PathBuf,
    process::Command,
};

use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::{
    database::{self, assignment::Test},
    model::submission_response::SubmissionResponse,
};

use image::ImageBuilder;

mod image;

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
    task_id: i32,
    was_late: bool,
    lang: String,
}

impl ContainerEntry {
    pub fn new(
        zip_file: axum::body::Bytes,
        user_id: i32,
        task_id: i32,
        was_late: bool,
        lang: impl Into<String>,
    ) -> Self {
        Self {
            zip_file,
            user_id,
            task_id,
            was_late,
            lang: lang.into(),
        }
    }
}

pub async fn container_queue(
    mut rx: tokio::sync::mpsc::Receiver<ContainerEntry>,
    n_threads: Option<usize>,
) -> ! {
    static SEMAPHORE: Semaphore = Semaphore::const_new(20);

    if let Some(n) = n_threads {
        let cur_n = SEMAPHORE.available_permits();
        let diff = n as i32 - cur_n as i32;

        match diff {
            ..0 => _ = SEMAPHORE.forget_permits(-diff as usize),
            1.. => SEMAPHORE.add_permits(diff as usize),
            0 => (),
        };
    }

    warn!("MAX THREADS: {}", SEMAPHORE.available_permits());

    loop {
        if let Ok(perm) = SEMAPHORE.acquire().await
            && let Some(container) = rx.recv().await
        {
            tokio::spawn(async move {
                let user_id = container.user_id;
                let task_id = container.task_id;
                let Ok(results) = run_container(container).await else {
                    drop(perm);
                    tracing::error!("Unable to run container");

                    // Log error in psql

                    return;
                };
                drop(perm);

                let json_results = serde_json::to_vec(&results).unwrap();

                database::assignment::operations::container_add_task_grade(
                    user_id,
                    task_id,
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
        task_id,
        was_late,
        lang,
    }: ContainerEntry,
) -> Result<SubmissionResponse, String> {
    let Some(container) = get_container_for_language(&lang) else {
        error!("No container found for language: {}", lang);
        // Log error in database
        return Err("Language not supported".into());
    };

    let workdir = format!("/tmp/securegrade/{}-{}", user_id, task_id);

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

    let task = match database::assignment::operations::container_get_task_details(task_id).await {
        Ok(r) => r,
        Err(e) => return Err(e),
    };

    let image = ImageBuilder::new(&workdir).build().unwrap();
    info!("Removing working directory {workdir}");
    remove_dir_all(&workdir).unwrap();

    // let mut test_results = ResponseObject::default();
    let mut test_results = SubmissionResponse::default();

    for Test {
        test_name,
        input,
        output,
        public,
        timeout,
    } in &task
    {
        let container_output = match image.exec(&input, *timeout).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                if *public {
                    test_results.pub_time_out(test_name.clone(), input, output);
                } else {
                    test_results.time_out(test_name.clone());
                }
                continue;
            }
            Err(e) => {
                if *public {
                    test_results.pub_err(test_name.clone(), input, output, e);
                } else {
                    test_results.err(test_name.clone());
                }
                continue;
            }
        };

        if container_output.trim() == output.trim() {
            if *public {
                test_results.pub_pass(
                    test_name.clone(),
                    was_late,
                    input.trim(),
                    output.trim(),
                    container_output.trim(),
                );
            } else {
                test_results.pass(test_name.clone(), was_late);
            }
        } else {
            if *public {
                test_results.pub_fail(
                    test_name.clone(),
                    input.trim(),
                    output.trim(),
                    container_output.trim(),
                );
            } else {
                test_results.fail(test_name.clone());
            }
        }
    }

    // Store test_results in database
    Ok(test_results)
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
