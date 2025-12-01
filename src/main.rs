use std::env::var;
use std::net::SocketAddr;
use std::sync::OnceLock;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, Method};
use axum::middleware::from_fn;
use axum::routing::{get, post, put};
use axum_server::tls_rustls::RustlsConfig;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use crate::container::ContainerEntry;
use crate::model::supplementary_material::SupplementaryMaterial;

mod container;
mod database;
mod endpoints;
mod model;
mod security;

/// Basic nondescript OK request body, in case the client is looking for a JSON response.
const OK_JSON: &str = r#"{ "message": "OK" }"#;

/// Static, global mpsc channel Sender. Sends ContainerEntries to the container processing queue.
static TX: OnceLock<tokio::sync::mpsc::Sender<ContainerEntry>> = OnceLock::new();

#[tokio::main]
async fn main() {
    // Begin logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Create the CORS layer, which essentially sets a guideline that requests must follow
    // Allow GET, POST, PUT, and OPTIONS methods
    // Allow Auth, content-type, and "language" headers
    // Allow requests from any origin
    // Expose internal headers content-type, admin, instructor, and student (of which are used to let the frontend know what to display)
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_lowercase(b"language").unwrap(),
        ])
        .allow_origin(AllowOrigin::any())
        .expose_headers([
            CONTENT_TYPE,
            HeaderName::from_lowercase(b"admin").unwrap(),
            HeaderName::from_lowercase(b"instructor").unwrap(),
            HeaderName::from_lowercase(b"student").unwrap(),
        ]);

    // Create application
    // Each layer acts as a layer of an onion, with the ones added first
    // acting as the centre of the onion, and the ones added last acting
    // as the outer layers
    // let app: Router = Router::new();

    // Add admin layer
    let admin_routes: Router = Router::new()
        .route("/create_class", post(endpoints::admin::create_class));

    // The instructor layer
    // All endpoints in this layer require a class_number path parameter.
    // Endpoints in this layer are accessible by instructors of the provided class number.
    // Admins are excluded.
    let instructor_routes: Router = Router::new()
        .route(
            "/{class_number}/add_instructor",
            put(endpoints::instructor::add_instructor),
        )
        .route(
            "/{class_number}/{assignment_number}/download/{username}",
            get(endpoints::instructor::download_submission),
        )
        .route(
            "/{class_number}/{assignment_number}/retrieve_scores",
            get(endpoints::instructor::retrieve_scores),
        )
        .route(
            "/{class_number}/add_assignment",
            post(endpoints::instructor::add_assignment),
        )
        .route(
            "/{class_number}/{assignment_id}/update_assignment",
            put(endpoints::instructor::update_assignment),
        )
        .route(
            "/{class_number}/{assignment_id}/retrieve_full_assignment",
            get(endpoints::instructor::retrieve_full_assignment_info),
        )
        .route(
            "/{class_number}/generate_join_code",
            get(endpoints::instructor::generate_join_code),
        )
        .route(
            "/{class_number}/add_student",
            put(endpoints::instructor::add_student),
        )
        .route(
            "/{class_number}/list_all_students",
            get(endpoints::list_all_students),
        );

    // The student layer
    // These endpoints all require a class_number path parameter. They are accessible
    // by both students and instructors of that class. Admins are excluded.
    let student_routes: Router = Router::new()
        .route(
            "/{class_number}/{assignment_id}/{task_id}/download_material",
            get(endpoints::student::download_material),
        )
        .route(
            "/{class_number}/{assignment_id}/{task_id}/submit",
            post(endpoints::student::handle_submission),
        )
        .route(
            "/{class_number}/{assignment_id}/{task_id}/retrieve_score",
            get(endpoints::student::retrieve_task_score),
        )
        .route(
            "/{class_number}/{assignment_id}",
            get(endpoints::student::get_assignment),
        )
        .route("/{class_number}", get(endpoints::student::get_class_info));

    // The general User layer
    // These endpoints are accessible by all authenticated users
    let general_routes: Router = Router::new()
        .route("/join_class", put(endpoints::join_class))
        .route("/get_classes", get(endpoints::get_classes))
        .route("/list_all_students", get(endpoints::list_all_students))
        .route(
            "/get_supported_languages",
            get(endpoints::supported_languages),
        );

    // The CORS and Max Body Limit layers
    // These endpoints are public
    let public_routes: Router = Router::new()
        .route("/login", post(endpoints::login))
        .route("/signup", post(endpoints::signup));

    // Define the app, merging the routers
    let app = Router::new()
        .nest("/admin", admin_routes)
        .layer(from_fn(security::handle_admin_auth))
        .nest("/instructor", instructor_routes)
        .layer(from_fn(security::handle_instructor_auth))
        .nest("/student", student_routes)
        .layer(from_fn(security::handle_student_auth))
        .merge(general_routes)
        .layer(from_fn(security::handle_basic_auth))
        .merge(public_routes)
        .layer(cors)
        .layer(DefaultBodyLimit::max(usize::MAX));


    // Load the certificate for HTTPS
    let config =
        RustlsConfig::from_pem_file("aeskul.net_certificate.cer", "aeskul.net_private_key.key")
            .await
            .unwrap();

    // Initialize the database, aborting start-up if an error occurs
    if let Err(e) = database::init_database().await {
        tracing::error!("{}", e);
        return;
    };

    info!("Database initialized");

    // Initialize an mpsc channel so submissions can be processed
    let (tx, rx) = tokio::sync::mpsc::channel::<ContainerEntry>(i32::MAX as usize);

    let n_threads = var("NTHREADS").ok().and_then(|f| f.parse::<usize>().ok());

    // Spawn the persistent container-processing queue thread
    tokio::spawn(async move {
        container::container_queue(rx, n_threads).await;
    });

    // Make the sender portion of the channel global, so it can be accessed across all threads
    TX.set(tx).unwrap();

    // Serve the application on port 9090
    let server = axum_server::bind_rustls("0.0.0.0:9090".parse::<SocketAddr>().unwrap(), config);
    server.serve(app.into_make_service()).await.unwrap();
}
