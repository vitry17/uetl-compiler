use axum::{
    routing::{get, post},
    Router,
};

use super::handlers;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/profiles", get(handlers::profiles))
        .route("/compile", post(handlers::compile))
        .route("/compile/all", post(handlers::compile_all))
        .route("/compile/send", post(handlers::compile_send))
        .route("/validate", post(handlers::validate))
}
