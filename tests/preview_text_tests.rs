//! `preview-text` sur `<ue-email>` — le preheader affiché par la plupart
//! des clients a cote de l'objet (voir Obsidian_Unimail/audit-uetl-compiler.md,
//! ecart confirme etape 2 : le champ etait totalement absent de `DocumentNode`).

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn compile(src: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap())
}

#[test]
fn preview_text_is_rendered_as_a_hidden_block_right_after_body_opens() {
    let src = r#"<ue-email preview-text="Your order ships tomorrow."><ue-layout><ue-row><ue-col><ue-text>Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    let body_pos = html.find("<body").unwrap();
    let preview_pos = html.find("Your order ships tomorrow.").unwrap();
    let content_pos = html.find("Body").unwrap();

    assert!(
        body_pos < preview_pos,
        "le preheader doit venir apres <body>"
    );
    assert!(
        preview_pos < content_pos,
        "le preheader doit venir avant le contenu visible"
    );
    assert!(
        html.contains("display:none"),
        "le bloc doit rester invisible"
    );
}

#[test]
fn preview_text_is_padded_with_zero_width_spaces() {
    let src = r#"<ue-email preview-text="Short preview"><ue-layout><ue-row><ue-col><ue-text>Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(
        html.contains("&#8203;"),
        "remplissage absent, le snippet auto-genere deborderait sur le corps"
    );
}

#[test]
fn preview_text_is_html_escaped() {
    let src = r#"<ue-email preview-text="Save & win <big> prizes"><ue-layout><ue-row><ue-col><ue-text>Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(html.contains("Save &amp; win &lt;big&gt; prizes"));
    assert!(!html.contains("Save & win <big> prizes"));
}

#[test]
fn no_preview_text_means_no_hidden_block_at_all() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = compile(src);

    assert!(!html.contains("&#8203;"));
    assert!(!html.contains("display:none;max-height:0"));
}

#[test]
fn preview_text_is_not_flagged_as_an_unknown_attribute() {
    use uetl_compiler::compiler::collect_warnings;

    let src = r#"<ue-email preview-text="hi"><ue-layout><ue-row><ue-col><ue-text>Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();

    assert!(collect_warnings(&doc).is_empty());
}
