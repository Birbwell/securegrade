use axum::Router;
use axum::routing::{get, post};
use tracing::{info, subscriber, Level};
use tracing_subscriber::FmtSubscriber;

use crate::container::run_container;
use crate::submission_object::SubmissionObject;

mod assignment;
mod container;
mod response_object;
mod submission_object;
mod docker;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::TRACE).finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let app = Router::new().route("/submit", post(receive_submission));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn receive_submission(json_submission: String) -> String {
    let json_val = serde_json::from_str::<SubmissionObject>(&json_submission).unwrap();

    let out = match run_container(json_val) {
        Ok(out) => out,
        Err(e) => format!("Error: {e}"),
    };
    
    out
}
