//! `width` sur `ue-col` — silencieusement ignoré jusqu'ici (voir
//! Obsidian_Unimail/audit-uetl-compiler.md, écart confirmé étape 4) :
//! LANGUAGE.md ne listait pour `ue-col` que background/padding/border/
//! border-radius/align, si bien qu'un template avec des colonnes 62/36
//! sortait en 50/50 sans le moindre avertissement.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str, client: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile(client).unwrap())
}

const TWO_COLS_PCT: &str = r##"<ue-email><ue-layout><ue-row>
<ue-col width="62%"><ue-text>Large</ue-text></ue-col>
<ue-col width="36%"><ue-text>Small</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;

#[test]
fn table_path_carries_a_percentage_width_as_both_html_attribute_and_css() {
    // outlook_desktop ne supporte pas flexbox : chemin table.
    let html = render(TWO_COLS_PCT, "outlook_desktop");

    assert!(
        html.contains(r#"width="62%""#),
        "attribut HTML absent : {html}"
    );
    assert!(
        html.contains(r#"width="36%""#),
        "attribut HTML absent : {html}"
    );
    assert!(
        html.contains("width:62%;"),
        "declaration CSS absente : {html}"
    );
    assert!(
        html.contains("width:36%;"),
        "declaration CSS absente : {html}"
    );
}

#[test]
fn flexbox_path_uses_a_fixed_flex_basis_instead_of_equal_distribution() {
    // gmail/outlook_desktop n'ont pas flexbox (css_flexbox: false dans leurs
    // profils) : apple_mail est un profil a support complet.
    let html = render(TWO_COLS_PCT, "apple_mail");

    assert!(
        html.contains("flex:0 0 62%;"),
        "flex-basis 62% absente : {html}"
    );
    assert!(
        html.contains("flex:0 0 36%;"),
        "flex-basis 36% absente : {html}"
    );
    // Le defaut (repartition egale) ne doit plus apparaitre pour ces colonnes.
    assert!(!html.contains("flex:1;"));
}

#[test]
fn a_column_without_width_keeps_the_previous_behaviour_on_both_paths() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>A</ue-text></ue-col><ue-col><ue-text>B</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;

    let flex_html = render(src, "apple_mail");
    assert!(flex_html.contains("flex:1;"));

    let table_html = render(src, "outlook_desktop");
    // Les tables englobantes portent deja `width="100%"` par ailleurs : seule
    // la cellule de la colonne elle-meme doit rester sans attribut width.
    let col_cell = table_html
        .split("class=\"ue-col\"")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    assert!(
        !col_cell.contains("width="),
        "attribut width inattendu sur la colonne : {col_cell}"
    );
}

#[test]
fn pixel_width_becomes_a_bare_integer_html_attribute_but_keeps_its_unit_in_css() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col width="200px"><ue-text>A</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = render(src, "outlook_desktop");

    assert!(html.contains(r#"width="200""#), "attribut non nu : {html}");
    assert!(html.contains("width:200px;"), "unite CSS perdue : {html}");
}

#[test]
fn a_unitless_number_is_treated_as_pixels_on_both_the_attribute_and_the_style() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col width="200"><ue-text>A</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = render(src, "outlook_desktop");

    assert!(html.contains(r#"width="200""#));
    assert!(html.contains("width:200px;"));
}

#[test]
fn width_is_no_longer_an_unknown_attribute_warning_on_ue_col() {
    use uetl_compiler::compiler::collect_warnings;

    let doc = Parser::parse_document(TWO_COLS_PCT).unwrap();
    let warnings = collect_warnings(&doc);

    assert!(
        warnings.is_empty(),
        "width ne devrait plus etre signale comme inconnu : {warnings:?}"
    );
}
