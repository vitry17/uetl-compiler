use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::compiler::{HtmlGenerator, ProfileRegistry};
use crate::parser::Parser;

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn profiles() -> Json<Value> {
    let ids: Vec<&str> = ProfileRegistry::shared()
        .list_profiles()
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    Json(json!({ "profiles": ids }))
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
    pub results: HashMap<String, CompileResult>,
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
}

pub enum ApiError {
    UnknownClient(String),
    ParseFailed(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::UnknownClient(client) => (
                StatusCode::BAD_REQUEST,
                "unknown_client",
                format!("unknown email client '{client}'"),
            ),
            ApiError::ParseFailed(message) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "parse_error", message)
            }
        };
        let body = Json(json!({ "error": { "code": code, "message": message } }));
        (status, body).into_response()
    }
}

pub async fn compile(Json(req): Json<CompileRequest>) -> Result<Json<CompileResult>, ApiError> {
    let profile = ProfileRegistry::shared()
        .get_profile(&req.client)
        .ok_or_else(|| ApiError::UnknownClient(req.client.clone()))?;

    let document = Parser::parse_document(&req.uetl).map_err(|e| ApiError::ParseFailed(e.to_string()))?;
    let html = HtmlGenerator::generate(&document, profile);

    Ok(Json(CompileResult {
        html,
        warnings: Vec::new(),
    }))
}

pub async fn compile_all(
    Json(req): Json<CompileAllRequest>,
) -> Result<Json<CompileAllResponse>, ApiError> {
    let document = Parser::parse_document(&req.uetl).map_err(|e| ApiError::ParseFailed(e.to_string()))?;

    let results = ProfileRegistry::shared()
        .list_profiles()
        .into_iter()
        .map(|profile| {
            let html = HtmlGenerator::generate(&document, profile);
            (
                profile.id.clone(),
                CompileResult {
                    html,
                    warnings: Vec::new(),
                },
            )
        })
        .collect();

    Ok(Json(CompileAllResponse { results }))
}

pub async fn validate(Json(req): Json<ValidateRequest>) -> Json<ValidateResponse> {
    match Parser::parse_document(&req.uetl) {
        Ok(_) => Json(ValidateResponse {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }),
        Err(e) => Json(ValidateResponse {
            valid: false,
            errors: vec![e.to_string()],
            warnings: Vec::new(),
        }),
    }
}
