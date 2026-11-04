use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;
use uetl_compiler::api::routes::router;

async fn post_raw(uri: &str, body: &Value) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap();

    let response = router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, bytes.to_vec())
}

async fn post(uri: &str, body: Value) -> (StatusCode, Value) {
    let (status, bytes) = post_raw(uri, &body).await;
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    (status, json)
}

const VALID_DOC: &str = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://example.com">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;

#[tokio::test]
async fn health_returns_ok() {
    let request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let response = router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn profiles_lists_seven_clients() {
    let request = Request::builder()
        .method("GET")
        .uri("/profiles")
        .body(Body::empty())
        .unwrap();
    let response = router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["profiles"].as_array().unwrap().len(), 7);
}

#[tokio::test]
async fn compile_returns_html_for_known_client() {
    let (status, body) = post("/compile", json!({ "uetl": VALID_DOC, "client": "gmail" })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["html"].as_str().unwrap().contains("<!DOCTYPE html>"));
    assert!(body["warnings"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn compile_rejects_unknown_client() {
    let (status, body) = post(
        "/compile",
        json!({ "uetl": VALID_DOC, "client": "does-not-exist" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "unknown_client");
}

#[tokio::test]
async fn compile_rejects_invalid_uetl_with_422() {
    let (status, body) = post(
        "/compile",
        json!({ "uetl": "<ue-layout></ue-layout>", "client": "gmail" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Le `code` est maintenant celui du diagnostic structuré (voir
    // `ParseError::to_diagnostic`), plus précis que le générique
    // "parse_error" d'avant — `message` reste présent, ce qui est tout ce
    // que le backend Elixir (`extract_compiler_message/1`) lit.
    assert_eq!(body["error"]["code"], "root_must_be_email");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("ue-email"));
    assert_eq!(body["error"]["line"], 1);
}

#[tokio::test]
async fn compile_rejects_a_source_over_the_size_limit() {
    let huge = format!(
        "<ue-email><ue-layout><ue-row><ue-col><ue-text>{}</ue-text></ue-col></ue-row></ue-layout></ue-email>",
        "a".repeat(300 * 1024)
    );
    let (status, body) = post("/compile", json!({ "uetl": huge, "client": "gmail" })).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["error"]["code"], "source_too_large");
}

#[tokio::test]
async fn compile_all_returns_results_for_every_profile() {
    let (status, body) = post("/compile/all", json!({ "uetl": VALID_DOC })).await;
    assert_eq!(status, StatusCode::OK);
    let results = body["results"].as_object().unwrap();
    assert_eq!(results.len(), 7);
    assert!(results["outlook_desktop"]["html"]
        .as_str()
        .unwrap()
        .contains("v:roundrect"));
    assert!(!results["gmail"]["html"]
        .as_str()
        .unwrap()
        .contains("v:roundrect"));
}

#[tokio::test]
async fn compile_all_response_bytes_are_identical_across_repeated_calls() {
    // Comparaison sur les octets bruts de la réponse, avant tout
    // reparsing JSON : `serde_json::Value`/`Map` retrie ses clés par défaut
    // (pas de feature `preserve_order`), ce qui masquerait un vrai problème
    // d'ordre côté `IndexMap`/`HashMap` si on comparait des `Value` au lieu
    // des octets tels qu'envoyés sur le fil.
    let body = json!({ "uetl": VALID_DOC });
    let (status, first) = post_raw("/compile/all", &body).await;
    assert_eq!(status, StatusCode::OK);

    for _ in 0..10 {
        let (status, bytes) = post_raw("/compile/all", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            bytes, first,
            "identical /compile/all requests must produce byte-identical responses"
        );
    }
}

#[tokio::test]
async fn validate_accepts_a_valid_document() {
    let (status, body) = post("/validate", json!({ "uetl": VALID_DOC })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], true);
    assert!(body["errors"].as_array().unwrap().is_empty());
    assert!(body["diagnostics"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn validate_rejects_an_invalid_document_without_erroring() {
    let (status, body) = post("/validate", json!({ "uetl": "<ue-layout></ue-layout>" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], false);
    // `errors` (Vec<String>, ancien format) reste peuplé à l'identique...
    assert!(!body["errors"].as_array().unwrap().is_empty());
    // ...et `diagnostics` (nouveau, structuré) porte la même information
    // de façon exploitable par un éditeur sans regex sur le message.
    let diagnostics = body["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "root_must_be_email");
    assert_eq!(diagnostics[0]["line"], 1);
    assert_eq!(diagnostics[0]["column"], 1);
}

#[tokio::test]
async fn validate_reports_every_local_error_in_one_pass() {
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-button>Missing href #1</ue-button></ue-col>
        <ue-col><ue-button>Missing href #2</ue-button></ue-col>
    </ue-row></ue-layout></ue-email>"#;
    let (status, body) = post("/validate", json!({ "uetl": src })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], false);
    let diagnostics = body["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    for d in diagnostics {
        assert_eq!(d["code"], "missing_required_attr");
    }
}

#[tokio::test]
async fn compile_still_stops_at_the_first_error_unlike_validate() {
    // /compile ne doit jamais generer un HTML partiel en ignorant des
    // elements fautifs - contrairement a /validate, il reste strict.
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-button>Missing href #1</ue-button></ue-col>
        <ue-col><ue-button>Missing href #2</ue-button></ue-col>
    </ue-row></ue-layout></ue-email>"#;
    let (status, body) = post("/compile", json!({ "uetl": src, "client": "gmail" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "missing_required_attr");
}

#[tokio::test]
async fn validate_rejects_a_source_over_the_size_limit() {
    let huge = format!(
        "<ue-email><ue-layout><ue-row><ue-col><ue-text>{}</ue-text></ue-col></ue-row></ue-layout></ue-email>",
        "a".repeat(300 * 1024)
    );
    let (status, body) = post("/validate", json!({ "uetl": huge })).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["error"]["code"], "source_too_large");
}

#[tokio::test]
async fn delivery_is_common_responsive_and_has_mso_hero_fallback_without_fixed_button() {
    let source = r##"<ue-email lang="fr" dark-mode="auto"><ue-layout background="#ffffff" max-width="640px"><ue-row stack-on="mobile"><ue-col><ue-button href="https://example.com" padding="18px 32px">Un libellé très long {{first_name}} qui doit rester visible</ue-button></ue-col><ue-col><ue-text color-dark="#ffffff">Texte</ue-text></ue-col></ue-row><ue-hero src="https://example.com/bg.jpg" height="240px"><ue-text>Hero</ue-text></ue-hero></ue-layout></ue-email>"##;
    let (status, body) = post("/compile/send", json!({"uetl": source})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let html = body["html"].as_str().unwrap();
    assert!(html.contains("<v:rect"));
    assert!(html.contains("<!--[if mso]><table"));
    assert!(html.contains("width=\"640\""));
    assert!(html.contains("@media"));
    assert!(html.contains("prefers-color-scheme"));
    assert!(html.contains("bgcolor=\"#ffffff\""));
    assert!(html.contains("padding:18px 32px;"));
    assert!(html.contains("{{first_name}}"));
    assert!(!html.contains("<v:roundrect"));
    assert!(!html.contains("display:flex"));
    let (_, again) = post("/compile/send", json!({"uetl": source})).await;
    assert_eq!(body, again);
}

#[tokio::test]
async fn delivery_rejects_invalid_source_and_preserves_legacy_preview_contract() {
    let (status, _) = post("/compile/send", json!({"uetl": "<div>Wrong</div>"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, gmail) = post("/compile", json!({"uetl": VALID_DOC, "client": "gmail"})).await;
    assert!(!gmail["html"].as_str().unwrap().contains("<!--[if mso]>"));
    let (_, outlook) = post(
        "/compile",
        json!({"uetl": VALID_DOC, "client": "outlook_desktop"}),
    )
    .await;
    assert!(outlook["html"].as_str().unwrap().contains("<v:roundrect"));
}

#[tokio::test]
async fn delivery_escapes_language_as_an_attribute() {
    let source = r#"<ue-email lang='en" onload="alert(1)'><ue-layout><ue-row><ue-col><ue-text>Safe</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let (status, body) = post("/compile/send", json!({"uetl": source})).await;
    assert_eq!(status, StatusCode::OK);
    let html = body["html"].as_str().unwrap();
    assert!(html.contains("&quot;"));
    assert!(!html.contains("lang=\"en\" onload="));
}
