//! Rate limiting basique, par fenêtre glissante d'une seconde, partagé par
//! toutes les requêtes (pas par client/IP — le compilateur n'est appelé
//! que par le backend Elixir, jamais directement par un navigateur, donc
//! une seule fenêtre globale suffit à se protéger d'un emballement
//! (boucle, bug côté appelant) sans la complexité d'un suivi par IP.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

const WINDOW: Duration = Duration::from_secs(1);

struct Window {
    started_at: Instant,
    count: u32,
}

#[derive(Clone)]
pub struct RateLimiter {
    max_per_second: u32,
    window: std::sync::Arc<Mutex<Window>>,
}

impl RateLimiter {
    pub fn new(max_per_second: u32) -> Self {
        Self {
            max_per_second,
            window: std::sync::Arc::new(Mutex::new(Window {
                started_at: Instant::now(),
                count: 0,
            })),
        }
    }

    /// Lit la limite depuis `COMPILER_RATE_LIMIT_PER_SECOND`, ou `default_max` si absente/invalide.
    pub fn from_env(default_max: u32) -> Self {
        let max = std::env::var("COMPILER_RATE_LIMIT_PER_SECOND")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_max);
        Self::new(max)
    }

    /// `true` si la requête est autorisée à cet instant — séparé de `Instant::now()`
    /// pour pouvoir tester la logique de fenêtre sans dépendre de l'horloge réelle.
    fn check_at(&self, now: Instant) -> bool {
        let mut window = self.window.lock().expect("rate limiter mutex poisoned");

        if now.duration_since(window.started_at) >= WINDOW {
            window.started_at = now;
            window.count = 0;
        }

        window.count += 1;
        window.count <= self.max_per_second
    }

    pub fn check(&self) -> bool {
        self.check_at(Instant::now())
    }
}

pub async fn enforce(
    State(limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    if limiter.check() {
        next.run(request).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "code": "rate_limited",
                    "message": "Too many requests — slow down and retry shortly."
                }
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_requests_up_to_the_limit_within_the_same_window() {
        let limiter = RateLimiter::new(3);
        let now = Instant::now();

        assert!(limiter.check_at(now));
        assert!(limiter.check_at(now));
        assert!(limiter.check_at(now));
        assert!(!limiter.check_at(now), "4th request in the same window must be rejected");
    }

    #[test]
    fn resets_the_count_once_the_window_elapses() {
        let limiter = RateLimiter::new(1);
        let now = Instant::now();

        assert!(limiter.check_at(now));
        assert!(!limiter.check_at(now));

        let next_window = now + Duration::from_secs(1);
        assert!(limiter.check_at(next_window), "a new window must reset the count");
    }

    #[test]
    fn from_env_falls_back_to_the_default_when_unset() {
        std::env::remove_var("COMPILER_RATE_LIMIT_PER_SECOND");
        let limiter = RateLimiter::from_env(42);
        assert_eq!(limiter.max_per_second, 42);
    }
}
