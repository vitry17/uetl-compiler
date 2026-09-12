use std::time::Duration;

use axum::http::HeaderValue;
use axum::middleware;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use uetl_compiler::api;
use uetl_compiler::api::rate_limit::RateLimiter;
use uetl_compiler::compiler::ProfileRegistry;

// ~1000 appels/jour/client en prod (voir CLAUDE.md §1.1) reste bien sous ce
// plafond par seconde — il n'existe ici que comme garde-fou contre un
// emballement (bug d'appelant, boucle), pas comme limite de capacité réelle.
const DEFAULT_RATE_LIMIT_PER_SECOND: u32 = 50;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:4001";

/// Construit la couche CORS depuis `COMPILER_CORS_ORIGIN` (une origine, ou
/// `*` pour tout autoriser). Par défaut, aucune origine n'est autorisée :
/// ce service n'est appelé que server-to-server par le backend Elixir,
/// jamais depuis un navigateur, donc `CorsLayer::permissive()` n'apportait
/// aucun bénéfice et n'exposait qu'un risque si ça changeait un jour sans
/// qu'on y repense.
fn cors_layer() -> CorsLayer {
    match std::env::var("COMPILER_CORS_ORIGIN") {
        Ok(origin) if origin == "*" => CorsLayer::permissive(),
        Ok(origin) => match HeaderValue::from_str(&origin) {
            Ok(value) => CorsLayer::new().allow_origin(value),
            Err(_) => {
                tracing::warn!(
                    origin,
                    "invalid COMPILER_CORS_ORIGIN, defaulting to no CORS"
                );
                CorsLayer::new()
            }
        },
        Err(_) => CorsLayer::new(),
    }
}

#[tokio::main]
async fn main() {
    // JSON plutôt que texte : ce service tourne en conteneur, dont les logs
    // sont collectés puis indexés par un agrégateur (voir `docker-compose`
    // du monorepo) — du texte libre y est bien moins exploitable qu'un
    // objet avec des champs filtrables (niveau, cible, timestamp).
    tracing_subscriber::fmt().json().init();

    // Échoue au démarrage plutôt qu'à la première requête si un profil
    // embarqué est invalide (voir `ProfileRegistry::load`) — un conteneur
    // qui crash-loop immédiatement est plus visible qu'un service qui
    // démarre "en bonne santé" et casse la première compilation venue.
    ProfileRegistry::shared();

    let rate_limiter = RateLimiter::from_env(DEFAULT_RATE_LIMIT_PER_SECOND);

    let app: Router = api::routes::router()
        .layer(CompressionLayer::new())
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(middleware::from_fn_with_state(
            rate_limiter,
            api::rate_limit::enforce,
        ));

    let bind_addr =
        std::env::var("COMPILER_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind to {bind_addr}: {e}"));

    tracing::info!(bind_addr, "UETL compiler listening");
    axum::serve(listener, app).await.expect("server error");
}
