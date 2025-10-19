use std::{
    io::Write,
    process::{Command, Stdio},
};

use tracing::{error, info, warn};

pub struct ImageBuilder {
    tag: Option<String>,
    directory: String,
}

#[derive(Clone)]
pub struct Image {
    image_id: String,
}

impl ImageBuilder {
    pub fn new(directory: impl Into<String>) -> ImageBuilder {
        Self {
            directory: directory.into(),
            tag: None,
        }
    }

    /// Build the docker container object
    pub fn build(self) -> Result<Image, String> {
        let container = Command::new("podman")
            .args(["buildx", "build", "-q", &self.directory])
            .output()
            .unwrap();

        if container.stderr.len() > 0 {
            let err_str = String::from_utf8(container.stderr)
                .unwrap()
                .trim()
                .to_string();
            error!("Error creating container: {}", err_str);
            return Err(err_str);
        }

        let image_id = String::from_utf8(container.stdout)
            .unwrap()
            .trim()
            .to_owned();
        info!("Image {image_id} created");

        Ok(Image { image_id })
    }
}

impl Image {
    /// Runs the docker container with the provided input
    pub fn exec(&self, stdin: String) -> Result<String, String> {
        let mut child = Command::new("podman")
            .args(["run", "-i", &self.image_id])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let child_stdin = child.stdin.as_mut().unwrap();
        child_stdin.write_all(stdin.as_bytes()).unwrap();

        let process_output = child.wait_with_output().unwrap();

        if process_output.stderr.len() > 0 {
            let err_str = String::from_utf8(process_output.stderr)
                .unwrap()
                .trim()
                .to_string();
            warn!("Error running container {}: {}", self.image_id, err_str);

            return Err(err_str);
        }

        let output = String::from_utf8(process_output.stdout).unwrap();

        Ok(output)
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        info!("Pruning containers...");
        Command::new("podman")
            .args(["container", "prune", "-f"])
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap()
            .stdout;

        info!("Removing image {}.", self.image_id);
        let rm_output = Command::new("podman")
            .args(["rmi", &self.image_id])
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap();

        if rm_output.stderr.len() > 0 {
            error!("Error removing container or images. Please review.");
            error!("{}", String::from_utf8(rm_output.stderr).unwrap());
        }
    }
}
