use std::{
    collections::HashMap,
    fs::{copy, create_dir, create_dir_all, read_dir, read_to_string, remove_dir_all, write},
    path::PathBuf,
};

use tracing::{error, info, warn};

use crate::{assignment::Assignment, docker::DockerBuilder, submission_object::SubmissionObject};

// Supported Languages
// pub enum Language {
//     Python311,
//     Python313,
//     Rust,
//     Java,
//     C,
//     Cpp,
// }

pub fn run_container(sub_ob: SubmissionObject) -> Result<String, String> {
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
        let (file_path, file_name, file_data) = (file.0, file.1, file.2);
        info!("Writing {}/{}/{}", workdir, file_path, file_name);

        // create_dir_all(&file_path).unwrap();
        create_dir_all(format!("{}/{}", workdir, file_path)).unwrap();
        write(
            format!("{}/{}/{}", workdir, file_path, file_name),
            file_data,
        )
        .unwrap();
    }

    let assignment_dir = format!("assignments/{}", sub_ob.assignment_id);
    let toml_assignment =
        read_to_string(format!("{}/assignment.toml", assignment_dir)).unwrap();
    let assignment = toml::from_str::<Assignment>(&toml_assignment).unwrap();

    let image = DockerBuilder::new(&workdir).build().unwrap();
    info!("Removing working directory {workdir}");
    remove_dir_all(&workdir).unwrap();

    for (test_name, test) in &assignment.tests {
        let input = if let Some(input_file) = &test.input_file {
            if test.input.is_some() {
                warn!("Assignment {}, {}: Both input and input_file defined. Defaulting to input_file.", sub_ob.assignment_id, test_name);
            }

            read_to_string(format!("{}/{}", assignment_dir, input_file)).unwrap()
        } else {
            test.input.clone().unwrap()
        };

        let output = if let Some(output_file) = &test.output_file {
            if test.output.is_some() {
                warn!("Assignment {}, {}: Both output and output_file defined. Defaulting to output_file.", sub_ob.assignment_id, test_name);
            }

            read_to_string(format!("{}/{}", assignment_dir, output_file)).unwrap()
        } else {
            test.output.clone().unwrap()
        };

        let container_output = image.exec(input).unwrap();
        if container_output.trim() == output {
            info!("Assignment {}, {} :: OK", sub_ob.assignment_id, test_name);
        } else {
            info!(
                "Assignment {}, {} :: Expected {:?}, found {:?}",
                sub_ob.assignment_id, test_name, output, container_output
            );
        }
    }

    todo!("Need to implement return value")
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
