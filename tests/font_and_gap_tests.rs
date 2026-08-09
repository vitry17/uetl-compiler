//! Police du document et espacement entre colonnes.
//!
//! Deux manques reveles en refaisant un vrai email marketing :
//!
//! - aucune `font-family` n'etait jamais emise, donc chaque client appliquait
//!   la sienne — souvent une serif dans Outlook, loin de toute charte ;
//! - `gap` etait documente sur `ue-row` mais n'existait que dans le rendu
//!   flexbox. Gmail et Outlook passent par un tableau, ou `gap` n'a aucun
//!   sens : l'attribut disparaissait et les cartes se touchaient.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str, client: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile(client).unwrap())
}

const DEUX_CARTES: &str = r##"<ue-email font-family="Inter, Helvetica, sans-serif"><ue-layout><ue-row gap="16px">
<ue-col background="#FFFFFF"><ue-text>A</ue-text></ue-col>
<ue-col background="#FFFFFF"><ue-text>B</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;

#[test]
fn document_font_reaches_every_text_block() {
    // Emise sur chaque bloc et pas seulement sur <body> : le moteur Word
    // d'Outlook n'herite pas la police dans les tableaux, et toute la mise en
    // page en est faite.
    let src = r#"<ue-email font-family="Inter, sans-serif"><ue-layout><ue-row><ue-col>
<ue-heading level="1">Titre</ue-heading>
<ue-text>Texte</ue-text>
<ue-button href="https://x.fr">Action</ue-button>
</ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "outlook_desktop");

    assert_eq!(
        html.matches("font-family:Inter, sans-serif").count(),
        4,
        "police absente d'un bloc : {html}"
    );
}

#[test]
fn a_document_without_font_falls_back_to_a_sans_serif_stack() {
    // Sans declaration, Outlook affichait une serif : aucun email de marque
    // ne ressemblait a son maquettage.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>Texte</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");

    assert!(html.contains("sans-serif"), "aucune police par defaut : {html}");
}

#[test]
fn gap_becomes_a_real_spacer_cell_in_the_table_layout() {
    let html = render(DEUX_CARTES, "gmail");

    assert!(html.contains("width:16px"), "espacement absent : {html}");
    // L'attribut width d'une cellule attend un entier nu, comme celui d'une
    // image : `width="16px"` serait invalide et ignore.
    assert!(html.contains(r#"width="16""#), "attribut width invalide");
    assert!(html.contains("font-size:0"), "la cellule vide imposera une hauteur");
}

#[test]
fn there_is_one_spacer_between_columns_not_around_them() {
    let html = render(DEUX_CARTES, "gmail");

    assert_eq!(
        html.matches("font-size:0;line-height:0;").count(),
        1,
        "espaceurs en trop ou en bordure : {html}"
    );
}

#[test]
fn gap_still_uses_the_native_property_where_flexbox_works() {
    let html = render(DEUX_CARTES, "apple_mail");

    assert!(html.contains("gap:16px"), "gap natif perdu");
    // Pas de cellule d'espacement inutile la ou la propriete existe.
    assert!(!html.contains("font-size:0;line-height:0;"), "espaceur superflu");
}

#[test]
fn a_row_without_gap_gets_no_spacer_at_all() {
    let src = r##"<ue-email><ue-layout><ue-row>
<ue-col><ue-text>A</ue-text></ue-col><ue-col><ue-text>B</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;

    assert!(!render(src, "gmail").contains("font-size:0;line-height:0;"));
}

#[test]
fn the_spacer_disappears_when_the_row_stacks_on_mobile() {
    // Empile, un espaceur deviendrait une bande vide entre chaque carte.
    let src = r##"<ue-email><ue-layout><ue-row gap="16px" stack-on="mobile">
<ue-col><ue-text>A</ue-text></ue-col><ue-col><ue-text>B</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;

    let html = render(src, "gmail");

    assert!(html.contains("ue-gap"), "espaceur non identifiable : {html}");
    assert!(
        html.contains("display:none!important"),
        "espaceur non masque a l'empilement"
    );
}
