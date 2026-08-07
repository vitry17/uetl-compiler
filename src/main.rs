use std::time::Duration;

use axum::middleware;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use uetl_compiler::api;
use uetl_compiler::api::rate_limit::RateLimiter;

// ~1000 appels/jour/client en prod (voir CLAUDE.md §1.1) reste bien sous ce
// plafond par seconde — il n'existe ici que comme garde-fou contre un
// emballement (bug d'appelant, boucle), pas comme limite de capacité réelle.
const DEFAULT_RATE_LIMIT_PER_SECOND: u32 = 50;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let rate_limiter = RateLimiter::from_env(DEFAULT_RATE_LIMIT_PER_SECOND);

    let app: Router = api::routes::router()
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(middleware::from_fn_with_state(rate_limiter, api::rate_limit::enforce));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001")
        .await
        .expect("failed to bind to port 4001");

    tracing::info!("UETL compiler listening on :4001");
    axum::serve(listener, app).await.expect("server error");
}
