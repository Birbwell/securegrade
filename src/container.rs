use std::{
    fs::{copy, create_dir, create_dir_all, read_dir, read_to_string, remove_dir_all, write},
    path::PathBuf,
};

use tracing::{error, info, warn};

use crate::{
    assignment::Assignment, database::auth, image::ImageBuilder, model::{response_object::ResponseObject, submission_object::SubmissionObject}
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

pub async fn run_container(sub_ob: SubmissionObject) -> Result<ResponseObject, String> {
    if !auth::validate::validate_student(sub_ob.clone().into()).await {
        return Err("Unauthorized".into());
    }

    let Some(container) = get_container_for_language(&sub_ob.lang) else {
        error!("No container found for language: {}", sub_ob.lang);
        return Err("Language not supported.".into());
    };

    let workdir = format!("/tmp/{}-{}", sub_ob.banner_id, sub_ob.assignment_id);

    // Delete and recreate working directory
    let _ = remove_dir_all(&workdir);
    create_dir(&workdir).unwrap();

    copy(
        container.join("Dockerfile"),
        format!("{}/Dockerfile", workdir),
    )
    .unwrap();

    for file in sub_ob.files {
        info!(
            "Writing {}/{}{}{}",
            workdir,
            file.parent_path,
            if file.parent_path.len() == 0 { "" } else { "/" },
            file.name
        );

        create_dir_all(format!("{}/{}", workdir, file.parent_path)).unwrap();
        write(
            format!(
                "{}/{}{}{}",
                workdir,
                file.parent_path,
                if file.parent_path.len() == 0 { "" } else { "/" },
                file.name
            ),
            file.data,
        )
        .unwrap();
    }

    let assignment_dir = format!("assignments/{}", sub_ob.assignment_id);
    let toml_assignment = read_to_string(format!("{}/assignment.toml", assignment_dir)).unwrap();
    let assignment = toml::from_str::<Assignment>(&toml_assignment).unwrap();

    let image = ImageBuilder::new(&workdir).build().unwrap();
    info!("Removing working directory {workdir}");
    remove_dir_all(&workdir).unwrap();

    let mut test_results = ResponseObject::default();

    for (test_name, test) in &assignment.tests {
        let input = if let Some(input_file) = &test.input_file {
            if test.input.is_some() {
                warn!(
                    "Assignment {}, {}: Both input and input_file defined. Defaulting to input_file.",
                    sub_ob.assignment_id, test_name
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
                    sub_ob.assignment_id, test_name
                );
            }

            read_to_string(format!("{}/{}", assignment_dir, output_file)).unwrap()
        } else {
            test.output.clone().unwrap()
        };

        // let Ok(container_output) = image.exec(input).unwrap();
        let container_output = match image.exec(input.clone(), assignment.get_timeout()).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                test_results.time_out(test_name);
                continue;
            }
            Err(e) => {
                test_results.err(test_name, e);
                continue;
            }
        };

        if container_output.trim() == output.trim() {
            info!("Assignment {}, {} :: OK", sub_ob.assignment_id, test_name);
            test_results.pass(test_name);
        } else {
            info!(
                "Assignment {}, {} :: Expected {:?}, found {:?}",
                sub_ob.assignment_id,
                test_name,
                output.trim(),
                container_output.trim()
            );
            if test.public {
                test_results.pub_fail(
                    test_name,
                    input.trim(),
                    output.trim(),
                    container_output.trim(),
                );
            } else {
                test_results.fail(test_name);
            }
        }
    }

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
