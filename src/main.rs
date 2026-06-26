use std::time::Duration;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use uetl_compiler::api;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app: Router = api::routes::router()
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(5)));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001")
        .await
        .expect("failed to bind to port 4001");

    tracing::info!("UETL compiler listening on :4001");
    axum::serve(listener, app).await.expect("server error");
}
