//! Tests de securite — voir Obsidian_Unimail/audit-uetl-compiler.md,
//! section "SECURITE — injection via les attributs".
//!
//! La plateforme est multi-tenant, les templates sont ecrits par les
//! utilisateurs et previsualises dans une iframe avant envoi : une valeur
//! d'attribut non echappee est un XSS stocke cote preview et un vecteur de
//! phishing cote envoi.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn compile(src: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap())
}

#[test]
fn a_single_quoted_attribute_value_cannot_smuggle_a_literal_double_quote() {
    // UETL accepte '...' ou "..." pour delimiter une valeur : un attribut
    // delimite par des simples quotes peut donc contenir un guillemet
    // double litteral, qui se retrouvait recopie tel quel dans le HTML —
    // suffisant pour fermer prematurement l'attribut `href` genere et
    // injecter un nouvel attribut (`onmouseover`, ici).
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href='" onmouseover="alert(1)'>Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains(r#"" onmouseover="alert(1)"#));
    assert!(html.contains("&quot;"));
}

#[test]
fn javascript_scheme_in_button_href_is_rejected() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="javascript:alert(1)">Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("javascript:"));
    assert!(html.contains("href=\"#\""));
}

#[test]
fn vbscript_scheme_in_href_is_rejected() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="vbscript:msgbox(1)">Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("vbscript:"));
}

#[test]
fn data_scheme_in_image_src_is_rejected_even_for_images() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="data:text/html,<script>alert(1)</script>" alt="x" /></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("data:text/html"));
}

#[test]
fn safe_schemes_and_template_placeholders_survive_compilation() {
    for href in [
        "https://example.com",
        "http://example.com",
        "mailto:a@example.com",
        "tel:+33100000000",
    ] {
        let src = format!(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="{href}">Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#
        );
        let html = compile(&src);
        assert!(
            html.contains(href),
            "expected {href} to survive compilation, got: {html}"
        );
    }
}

#[test]
fn a_template_placeholder_url_is_never_scheme_checked() {
    // Resolu plus tard par la plateforme, pas par le compilateur — impossible
    // d'en valider le schema au moment de la compilation.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="{{unsubscribe_url}}">Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(html.contains("{{unsubscribe_url}}"));
}

#[test]
fn a_relative_path_with_no_scheme_is_preserved() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="/assets/logo.png" alt="logo" /></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(html.contains("/assets/logo.png"));
}

#[test]
fn a_heading_level_out_of_range_is_already_rejected_at_parse_time() {
    // Le parseur valide deja `level` pour une valeur litterale
    // (validate_heading_level, parser.rs) : ce payload n'atteint jamais
    // html_gen. Verifie juste que ce garde-fou existant n'a pas regresse.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-heading level="1 onmouseover=alert(1) x">Title</ue-heading></ue-col></ue-row></ue-layout></ue-email>"#;
    assert!(Parser::parse_document(src).is_err());
}

#[test]
fn a_templated_heading_level_cannot_break_out_of_the_tag_name_position() {
    // Le parseur n'exige la plage 1-6 que pour une valeur litterale : un
    // niveau exprime par un placeholder de template est accepte sans
    // validation, impossible de connaitre sa valeur reelle avant l'envoi.
    // `gen_heading` doit donc refuser lui-meme tout ce qui n'est pas
    // exactement "1".."6", quelle que soit sa provenance — avant correctif,
    // ce payload produisait litteralement <h{{x onmouseover=alert(1) y}}>.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-heading level="{{x onmouseover=alert(1) y}}">Title</ue-heading></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("onmouseover"));
    assert!(html.contains("<h1"));
}

#[test]
fn html_tags_in_a_color_attribute_are_escaped_not_passed_through() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text color='"><script>alert(1)</script>'>hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("<script>alert(1)</script>"));
}

#[test]
fn accessible_label_with_a_quote_cannot_break_out_of_the_aria_label_attribute() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://a.example" accessible-label='" onmouseover="alert(1)'>Click</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains(r#"" onmouseover="alert(1)"#));
}
