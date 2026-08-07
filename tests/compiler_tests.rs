use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

#[test]
fn button_rendering_differs_between_gmail_and_outlook_desktop() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://example.com">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let gmail_html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());
    let outlook_html = HtmlGenerator::generate(&doc, registry.get_profile("outlook_desktop").unwrap());

    assert_ne!(gmail_html, outlook_html);
    assert!(!gmail_html.contains("v:roundrect"));
    assert!(outlook_html.contains("v:roundrect"));
}

#[test]
fn button_background_and_color_override_the_theme_preset() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://example.com" background="#00AFF5" color="#05073B">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    for profile in ["gmail", "outlook_desktop"] {
        let html = HtmlGenerator::generate(&doc, registry.get_profile(profile).unwrap());

        assert!(html.contains("#00AFF5"), "{profile}: brand background missing");
        assert!(html.contains("#05073B"), "{profile}: brand text colour missing");
        // Le preset ne doit plus apparaitre une fois surcharge.
        assert!(!html.contains("#2E5FAC"), "{profile}: theme preset still applied");
    }
}

#[test]
fn button_falls_back_to_the_theme_preset_without_explicit_colours() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://example.com" theme="danger">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());
    assert!(html.contains("#d9534f"));
}

#[test]
fn row_uses_media_queries_only_when_profile_supports_them() {
    let src = r#"<ue-email><ue-layout><ue-row stack-on="mobile"><ue-col><ue-text>A</ue-text></ue-col><ue-col><ue-text>B</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let gmail_html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());
    let outlook_html = HtmlGenerator::generate(&doc, registry.get_profile("outlook_desktop").unwrap());

    assert!(gmail_html.contains("@media"));
    assert!(!outlook_html.contains("@media"));
}

#[test]
fn dark_mode_image_differs_between_apple_mail_and_gmail() {
    let src = r#"<ue-email dark-mode="auto"><ue-layout><ue-row><ue-col><ue-image src="logo.png" alt="Logo" dark-src="logo-dark.png" /></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let apple_html = HtmlGenerator::generate(&doc, registry.get_profile("apple_mail").unwrap());
    let gmail_html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(apple_html.contains("<picture>"));
    assert!(apple_html.contains("prefers-color-scheme"));
    assert!(!gmail_html.contains("<picture>"));
}

#[test]
fn dark_mode_overrides_heading_text_and_layout_colors_when_supported() {
    let src = r##"<ue-email dark-mode="auto"><ue-layout background-light="#ffffff" background-dark="#1a1a2e"><ue-row><ue-col><ue-heading level="1" color-light="#111111" color-dark="#eeeeee">Titre</ue-heading><ue-text color-light="#333333" color-dark="#cccccc">Texte</ue-text></ue-col></ue-row></ue-layout></ue-email>"##;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let apple_html = HtmlGenerator::generate(&doc, registry.get_profile("apple_mail").unwrap());
    let gmail_html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(apple_html.contains("prefers-color-scheme:dark"));
    assert!(apple_html.contains("#eeeeee"));
    assert!(apple_html.contains("#cccccc"));
    assert!(apple_html.contains("#1a1a2e"));
    assert!(!gmail_html.contains("prefers-color-scheme:dark"));
}

#[test]
fn yahoo_mail_gets_data_ogsc_ogsb_instead_of_a_media_query() {
    // Yahoo/AOL ne supportent pas prefers-color-scheme dans le contenu d'un
    // mail, mais lisent eux-mêmes ces attributs propriétaires pour basculer
    // la couleur affichée en mode sombre — voir Profile::quirk("dark_mode_data_attributes").
    let src = r##"<ue-email dark-mode="auto"><ue-layout background-light="#ffffff" background-dark="#1a1a2e"><ue-row><ue-col><ue-heading level="1" color-light="#111111" color-dark="#eeeeee">Titre</ue-heading></ue-col></ue-row></ue-layout></ue-email>"##;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let yahoo_html = HtmlGenerator::generate(&doc, registry.get_profile("yahoo_mail").unwrap());

    assert!(!yahoo_html.contains("prefers-color-scheme"));
    assert!(yahoo_html.contains("data-ogsb=\"#1a1a2e\""));
    assert!(yahoo_html.contains("data-ogsc=\"#eeeeee\""));
}

#[test]
fn generated_style_rules_live_in_head_not_scattered_in_body() {
    let src = r#"<ue-email><ue-layout><ue-row stack-on="mobile"><ue-col><ue-text>A</ue-text></ue-col><ue-col><ue-text>B</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    let head_end = html.find("</head>").expect("missing </head>");
    let style_pos = html.find("<style>").expect("missing <style> block");
    assert!(style_pos < head_end, "style block must live inside <head>");
}

#[test]
fn template_variable_is_preserved_in_output() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>Bonjour {{prenom}},</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains("{{prenom}}"));
}

#[test]
fn raw_content_is_not_escaped_unlike_text_content() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-raw>5 &amp; 6</ue-raw><ue-text>5 &amp; 6</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains("5 &amp; 6"));
    assert!(html.contains("5 &amp;amp; 6"));
}

#[test]
fn raw_block_passes_through_literal_html_tags() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-raw><div class="custom"><b>bold</b></div></ue-raw><ue-text>after</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains(r#"<div class="custom"><b>bold</b></div>"#));
    assert!(html.contains("<p>after</p>"));
}

#[test]
fn self_closing_raw_block_has_no_content() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-raw /><ue-text>after</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains("<p>after</p>"));
}

const FULL_DOCUMENT: &str = r##"<ue-email lang="fr" dark-mode="auto">
<ue-layout max-width="600px" background-light="#ffffff" background-dark="#1a1a2e">
<ue-row stack-on="mobile">
<ue-col>
<ue-heading level="1" color-light="#111111" color-dark="#eeeeee">Titre</ue-heading>
<ue-text>Bonjour {{prenom}}</ue-text>
<ue-button href="{{cta_url}}" theme="primary">Voir l'offre</ue-button>
<ue-image src="logo.png" alt="Logo" dark-src="logo-dark.png" />
<ue-divider />
<ue-spacer height="20px" />
<ue-interactive fallback-src="static.png"></ue-interactive>
<ue-raw>contenu brut</ue-raw>
</ue-col>
</ue-row>
</ue-layout>
</ue-email>"##;

#[test]
fn renders_every_component_on_every_profile_without_panicking() {
    let doc = Parser::parse_document(FULL_DOCUMENT).unwrap();
    let registry = ProfileRegistry::load();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);
        assert!(html.starts_with("<!DOCTYPE html>"), "profile {}", profile.id);
        assert!(html.contains("{{prenom}}"), "profile {}", profile.id);
        assert!(html.contains("{{cta_url}}"), "profile {}", profile.id);
        assert!(html.contains("Titre"), "profile {}", profile.id);
        assert!(html.contains("contenu brut"), "profile {}", profile.id);
    }
}
