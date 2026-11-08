//! Lisibilité du document envoyé pour les lecteurs non humains : clients
//! texte, filtres anti-spam et assistants IA des boîtes mail (résumés Apple
//! Mail, Gemini dans Gmail). Voir `Obsidian_Unimail/fonctionnalité IA/`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;
use uetl_compiler::api::routes::router;
use uetl_compiler::compiler::html_gen::text_direction;
use uetl_compiler::compiler::{
    collect_structure_warnings, to_plain_text, HtmlGenerator, ProfileRegistry,
};
use uetl_compiler::parser::Parser;

const PROMO: &str = r#"<ue-email lang="fr" preview-text="Aperçu caché">
  <ue-layout>
    <ue-row>
      <ue-col>
        <ue-heading level="1">-20 % jusqu'à dimanche</ue-heading>
        <ue-text>Bonjour {{ first_name }}, <ue-bold>toute la sélection</ue-bold> est à -20 %&nbsp;!</ue-text>
        <ue-spacer height="12px" />
        <ue-button href="https://shop.example/promo">J'en profite</ue-button>
        <ue-image src="https://cdn.example/p.png" alt="Sac en cuir camel" />
        <ue-image src="https://cdn.example/deco.png" alt="" />
        <ue-divider />
        <ue-text>Offre valable jusqu'au 16 novembre.</ue-text>
      </ue-col>
    </ue-row>
  </ue-layout>
</ue-email>"#;

fn parse(src: &str) -> uetl_compiler::parser::DocumentNode {
    Parser::parse_document(src).unwrap()
}

fn compile(src: &str, profile: &str) -> String {
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&parse(src), registry.get_profile(profile).unwrap())
}

#[test]
fn plain_text_follows_the_reading_order_with_buttons_as_label_and_url() {
    let text = to_plain_text(&parse(PROMO));

    assert_eq!(
        text,
        "-20 % jusqu'à dimanche\n\n\
         Bonjour {{first_name}}, toute la sélection est à -20 % !\n\n\
         J'en profite : https://shop.example/promo\n\n\
         [Sac en cuir camel]\n\n\
         ———\n\n\
         Offre valable jusqu'au 16 novembre."
    );
}

#[test]
fn plain_text_never_contains_the_hidden_preheader_nor_markup() {
    let text = to_plain_text(&parse(PROMO));

    assert!(
        !text.contains("Aperçu caché"),
        "le préheader est caché, il ne doit pas réapparaître"
    );
    assert!(!text.contains('<'));
    assert!(!text.contains("&nbsp;"));
}

#[test]
fn plain_text_keeps_dynamic_blocks_for_personalisation_at_send_time() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>{% if vip %}Accès VIP{% endif %}</ue-text><ue-button href="{{ order_url }}">Suivre</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let text = to_plain_text(&parse(src));

    assert!(text.contains("{% if vip %}Accès VIP{% endif %}"));
    assert!(text.contains("Suivre : {{order_url}}") || text.contains("Suivre : {{ order_url }}"));
}

#[test]
fn a_preview_text_made_of_a_single_variable_is_kept() {
    let src = r#"<ue-email preview-text="{{ accroche }}"><ue-layout><ue-row><ue-col><ue-text>Corps</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src, "gmail");

    assert!(html.contains("mso-hide:all;\">{{accroche}}</div>"));
}

#[test]
fn lang_and_dir_are_declared_on_html_body_and_main_container() {
    let html = compile(PROMO, "gmail");

    assert!(html.contains("<html lang=\"fr\" dir=\"ltr\">"));
    assert!(html.contains("<body lang=\"fr\" dir=\"ltr\""));
    // Gmail retire <html> et <body> : le conteneur principal porte aussi la langue.
    assert!(html.contains("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" lang=\"fr\" dir=\"ltr\""));
}

#[test]
fn right_to_left_languages_get_dir_rtl() {
    assert_eq!(text_direction("ar"), "rtl");
    assert_eq!(text_direction("he-IL"), "rtl");
    assert_eq!(text_direction("fa_IR"), "rtl");
    assert_eq!(text_direction("fr"), "ltr");
    assert_eq!(text_direction("en-US"), "ltr");
    assert_eq!(text_direction(""), "ltr");

    let src = r#"<ue-email lang="ar"><ue-layout><ue-row><ue-col><ue-text>مرحبا</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    assert!(compile(src, "apple_mail").contains("<html lang=\"ar\" dir=\"rtl\">"));
}

#[test]
fn exactly_one_level_one_heading_is_expected() {
    assert!(collect_structure_warnings(&parse(PROMO)).is_empty());

    let none = r#"<ue-email><ue-layout><ue-row><ue-col><ue-heading level="2">Titre</ue-heading></ue-col></ue-row></ue-layout></ue-email>"#;
    let warnings = collect_structure_warnings(&parse(none));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("heading_structure: no"));

    let two = r#"<ue-email><ue-layout><ue-row><ue-col><ue-heading level="1">A</ue-heading><ue-heading level="1">B</ue-heading></ue-col></ue-row></ue-layout></ue-email>"#;
    let warnings = collect_structure_warnings(&parse(two));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("heading_structure: 2"));
}

async fn post(uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn compile_send_returns_the_plain_text_part_and_compile_does_not() {
    let (status, body) = post(
        "/compile/send",
        json!({ "uetl": PROMO, "title": "-20 % jusqu'à dimanche" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = body["text"].as_str().unwrap();
    assert!(text.starts_with("-20 % jusqu'à dimanche"));
    assert!(text.contains("J'en profite : https://shop.example/promo"));
    assert!(body["html"].as_str().unwrap().contains("<title>"));

    let (status, body) = post("/compile", json!({ "uetl": PROMO, "client": "gmail" })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.get("text").is_none(),
        "les aperçus par client restent inchangés"
    );
}

#[tokio::test]
async fn validate_reports_heading_structure_without_invalidating() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>Pas de titre</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let (status, body) = post("/validate", json!({ "uetl": src })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], true);
    let warnings = body["warnings"].as_array().unwrap();
    assert!(warnings
        .iter()
        .any(|w| w.as_str().unwrap().starts_with("heading_structure:")));
}
