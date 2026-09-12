use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::compiler::{collect_warnings, HtmlGenerator, ProfileRegistry};
use crate::parser::{Diagnostic, DocumentNode, ParseError, Parser};

/// Taille maximale d'un document UETL accepté. Bien au-delà de tout
/// template email réel (quelques dizaines de Ko) ; sert uniquement à
/// rejeter tôt un envoi hostile ou buggé avant de le faire parser, plutôt
/// que de laisser un document de plusieurs Mo occuper un thread bloquant
/// pour rien.
const MAX_SOURCE_BYTES: usize = 256 * 1024;

fn check_source_size(uetl: &str) -> Result<(), ApiError> {
    if uetl.len() > MAX_SOURCE_BYTES {
        Err(ApiError::SourceTooLarge {
            actual: uetl.len(),
            max: MAX_SOURCE_BYTES,
        })
    } else {
        Ok(())
    }
}

/// Parse en dehors de la boucle d'événements tokio : `Parser::parse_document`
/// est un calcul CPU synchrone (récursion sur l'AST, pas d'I/O) ; l'exécuter
/// directement dans un handler async bloque le thread qui le sert et, en
/// mono-thread ou sous forte charge, retarde toutes les autres requêtes
/// pendant la durée du parsing. `spawn_blocking` le déplace sur le pool
/// dédié de tokio pour ça.
async fn parse_document_blocking(uetl: String) -> Result<DocumentNode, ParseError> {
    tokio::task::spawn_blocking(move || Parser::parse_document(&uetl))
        .await
        .expect("parse_document panicked")
}

pub async fn health() -> Json<Value> {
    // Force le chargement (et donc la validation de schéma, voir
    // `ProfileRegistry::load`) des profils dès le premier health check,
    // plutôt qu'au premier `/compile` réel : un profil JSON cassé au
    // déploiement échoue maintenant la sonde de liveness/readiness du
    // conteneur, au lieu de laisser le service démarrer "en bonne santé"
    // et de casser silencieusement la première requête venue d'un client.
    let profile_count = ProfileRegistry::shared().list_profiles().len();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "profiles_loaded": profile_count,
    }))
}

pub async fn profiles() -> Json<Value> {
    let profiles: Vec<Value> = ProfileRegistry::shared()
        .list_profiles()
        .iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "version": p.version }))
        .collect();
    Json(json!({ "profiles": profiles }))
}

#[derive(Debug, Deserialize)]
pub struct CompileRequest {
    pub uetl: String,
    pub client: String,
}

#[derive(Debug, Serialize)]
pub struct CompileResult {
    pub html: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompileAllRequest {
    pub uetl: String,
}

#[derive(Debug, Serialize)]
pub struct CompileAllResponse {
    /// `IndexMap` peuplée depuis `ProfileRegistry::list_profiles()` (trié
    /// par id) plutôt que `HashMap` : sans ça, l'ordre des clés dans le JSON
    /// de `/compile/all` variait d'un process à l'autre pour un input
    /// strictement identique, rendant tout diff/snapshot de test inutilisable.
    pub results: indexmap::IndexMap<String, CompileResult>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub uetl: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// Forme structurée des mêmes erreurs (code/ligne/colonne/détails),
    /// ajoutée aux côtés de `errors` sans le remplacer — un consommateur
    /// existant qui ne lit que `errors` (ex: `mcp_controller.ex` côté
    /// backend) continue de fonctionner à l'identique.
    pub diagnostics: Vec<Diagnostic>,
}

pub enum ApiError {
    UnknownClient(String),
    ParseFailed(ParseError),
    SourceTooLarge { actual: usize, max: usize },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Chaque branche pose explicitement `message` : `extract_compiler_message/1`
        // côté Elixir ne lit que `.error.message` et doit le trouver quelle que
        // soit la variante, sans avoir besoin de connaître `code`/les autres champs.
        let (status, body) = match self {
            ApiError::UnknownClient(client) => (
                StatusCode::BAD_REQUEST,
                json!({ "error": { "code": "unknown_client", "message": format!("unknown email client '{client}'") } }),
            ),
            ApiError::ParseFailed(err) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({ "error": err.to_diagnostic() }),
            ),
            ApiError::SourceTooLarge { actual, max } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({ "error": {
                    "code": "source_too_large",
                    "message": format!("UETL source is {actual} bytes, exceeding the {max} byte limit"),
                    "actual": actual,
                    "max": max,
                } }),
            ),
        };
        (status, Json(body)).into_response()
    }
}

pub async fn compile(Json(req): Json<CompileRequest>) -> Result<Json<CompileResult>, ApiError> {
    let profile = ProfileRegistry::shared()
        .get_profile(&req.client)
        .ok_or_else(|| ApiError::UnknownClient(req.client.clone()))?;
    check_source_size(&req.uetl)?;

    let document = parse_document_blocking(req.uetl)
        .await
        .map_err(ApiError::ParseFailed)?;
    let warnings = collect_warnings(&document);
    let html = HtmlGenerator::generate(&document, profile);

    Ok(Json(CompileResult { html, warnings }))
}

pub async fn compile_all(
    Json(req): Json<CompileAllRequest>,
) -> Result<Json<CompileAllResponse>, ApiError> {
    check_source_size(&req.uetl)?;
    let document = parse_document_blocking(req.uetl)
        .await
        .map_err(ApiError::ParseFailed)?;
    // Les memes pour chaque profil : un attribut inconnu ne depend pas du
    // client de messagerie, pas la peine de recalculer par profil.
    let warnings = collect_warnings(&document);

    let results = ProfileRegistry::shared()
        .list_profiles()
        .into_iter()
        .map(|profile| {
            let html = HtmlGenerator::generate(&document, profile);
            (
                profile.id.clone(),
                CompileResult {
                    html,
                    warnings: warnings.clone(),
                },
            )
        })
        .collect();

    Ok(Json(CompileAllResponse { results }))
}

pub async fn validate(
    Json(req): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, ApiError> {
    check_source_size(&req.uetl)?;
    Ok(match parse_document_blocking(req.uetl).await {
        Ok(document) => Json(ValidateResponse {
            valid: true,
            errors: Vec::new(),
            warnings: collect_warnings(&document),
            diagnostics: Vec::new(),
        }),
        Err(e) => Json(ValidateResponse {
            valid: false,
            errors: vec![e.to_string()],
            warnings: Vec::new(),
            diagnostics: vec![e.to_diagnostic()],
        }),
    })
}
