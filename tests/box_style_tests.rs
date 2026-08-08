//! Styles de boite : `ue-col`, `ue-row`, forme du bouton, alignements.
//!
//! Avant ces attributs, `gen_col` ne lisait rien et se contentait de rendre
//! ses enfants. Or les elements les plus courants d'un email reel — tuiles
//! d'arguments, encadres colores, blocs partenaires — sont des colonnes a
//! fond, marge interieure et angles arrondis. Ils etaient donc inexprimables,
//! quelle que soit l'habilete de l'auteur ou du modele qui redige l'UETL.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

const CARTE: &str = r##"<ue-email><ue-layout><ue-row gap="16px">
<ue-col background="#F0FDF4" padding="16px" border-radius="12px" border="1px solid #BBF7D0" align="center">
<ue-text>Reduisez le cout</ue-text>
</ue-col>
</ue-row></ue-layout></ue-email>"##;

#[test]
fn column_carries_its_own_background_padding_radius_and_border() {
    let doc = Parser::parse_document(CARTE).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains("background:#F0FDF4"), "fond absent : {html}");
    assert!(html.contains("padding:16px"), "marge interieure absente");
    assert!(html.contains("border-radius:12px"), "arrondi absent");
    assert!(html.contains("border:1px solid #BBF7D0"), "bordure absente");
    assert!(html.contains("text-align:center"), "alignement absent");
}

#[test]
fn column_background_is_also_a_bgcolor_attribute_for_outlook() {
    let doc = Parser::parse_document(CARTE).unwrap();
    let registry = ProfileRegistry::load();

    // Le moteur Word honore bgcolor bien plus fiablement qu'une declaration
    // CSS background : on emet les deux.
    let html = HtmlGenerator::generate(&doc, registry.get_profile("outlook_desktop").unwrap());

    assert!(html.contains(r##"bgcolor="#F0FDF4""##), "bgcolor absent : {html}");
}

#[test]
fn column_styles_survive_the_flexbox_layout_too() {
    let doc = Parser::parse_document(CARTE).unwrap();
    let registry = ProfileRegistry::load();

    // apple_mail passe par un <div style="display:flex"> et non par un
    // tableau : les styles de colonne doivent suivre les deux chemins.
    let html = HtmlGenerator::generate(&doc, registry.get_profile("apple_mail").unwrap());

    assert!(html.contains("display:flex"), "chemin flexbox attendu");
    assert!(html.contains("background:#F0FDF4"), "fond perdu en flexbox");
    assert!(html.contains("border-radius:12px"), "arrondi perdu en flexbox");
    // bgcolor n'a aucun sens sur un div : il ne doit pas y apparaitre.
    assert!(!html.contains("bgcolor"), "bgcolor emis sur un div");
}

#[test]
fn column_background_has_a_dark_variant() {
    let src = r##"<ue-email dark-mode="auto"><ue-layout><ue-row>
<ue-col background-light="#FFFFFF" background-dark="#0F1B33"><ue-text>x</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("apple_mail").unwrap());

    assert!(html.contains("background:#FFFFFF"), "fond clair absent");
    assert!(html.contains("prefers-color-scheme:dark"), "media query absente");
    assert!(html.contains("background:#0F1B33 !important"), "fond sombre absent");
}

#[test]
fn background_works_with_or_without_the_light_suffix() {
    // ue-layout n'acceptait que background-light, ue-row que background : le
    // langage etait imprevisible selon la balise.
    let src = r##"<ue-email><ue-layout background="#111111"><ue-row background-light="#222222">
<ue-col background="#333333"><ue-text>x</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());

    assert!(html.contains("#111111"), "background sur layout ignore");
    assert!(html.contains("#222222"), "background-light sur row ignore");
    assert!(html.contains("#333333"), "background sur col ignore");
}

#[test]
fn button_radius_is_configurable_and_defaults_to_four_pixels() {
    let registry = ProfileRegistry::load();
    let gmail = registry.get_profile("gmail").unwrap();

    let defaut = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://x.fr">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = HtmlGenerator::generate(&Parser::parse_document(defaut).unwrap(), gmail);
    assert!(html.contains("border-radius:4px"), "defaut modifie");

    let pilule = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://x.fr" border-radius="24px">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = HtmlGenerator::generate(&Parser::parse_document(pilule).unwrap(), gmail);
    assert!(html.contains("border-radius:24px"), "rayon personnalise ignore");
}

#[test]
fn outlook_translates_the_radius_into_a_vml_arcsize() {
    let registry = ProfileRegistry::load();
    let outlook = registry.get_profile("outlook_desktop").unwrap();

    // VML ignore border-radius : l'arrondi passe par arcsize, exprime en
    // pourcentage de la moitie du plus petit cote. Le bouton fait 44px de
    // haut, donc 50 % vaut un rayon de 22px — la pilule.
    let pilule = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://x.fr" border-radius="24px">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = HtmlGenerator::generate(&Parser::parse_document(pilule).unwrap(), outlook);
    assert!(html.contains(r#"arcsize="50%""#), "pilule non transmise a VML : {html}");

    let carre = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://x.fr" border-radius="0px">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let html = HtmlGenerator::generate(&Parser::parse_document(carre).unwrap(), outlook);
    assert!(html.contains(r#"arcsize="0%""#), "angles droits non transmis a VML");
}

#[test]
fn button_is_centered_by_a_table_align_attribute() {
    // Un <table> est de niveau bloc : text-align:center sur son parent ne le
    // centre pas. L'attribut HTML align, lui, est honore partout.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://x.fr" align="center">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    for id in ["gmail", "outlook_desktop", "apple_mail"] {
        let html = HtmlGenerator::generate(&doc, registry.get_profile(id).unwrap());
        assert!(html.contains(r#"align="center""#), "bouton non centre sur {id}");
    }
}

#[test]
fn text_accepts_an_alignment() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text align="center">Legende</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());
    assert!(html.contains("text-align:center"), "alignement du texte ignore");
}

#[test]
fn image_accepts_a_border_radius() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="hero.png" alt="Hero" border-radius="16px" /></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    let html = HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap());
    assert!(html.contains("border-radius:16px"), "arrondi de l'image ignore");
}

#[test]
fn a_styled_card_layout_renders_on_every_profile() {
    // Le motif « trois tuiles » d'un email reel, celui-la meme qui etait
    // inexprimable avant ces attributs.
    let src = r##"<ue-email><ue-layout background="#F8FAFC" padding="24px">
<ue-row gap="16px" stack-on="mobile">
<ue-col background="#FFFFFF" padding="20px" border-radius="12px" align="center">
<ue-image src="coins.png" alt="Economies" width="64px" />
<ue-text align="center">Reduisez encore le cout</ue-text>
</ue-col>
<ue-col background="#FFFFFF" padding="20px" border-radius="12px" align="center">
<ue-image src="rocket.png" alt="Rapide" width="64px" />
<ue-text align="center">Creez votre partage gratuitement</ue-text>
</ue-col>
</ue-row>
<ue-row><ue-col align="center">
<ue-button href="https://x.fr" background="#22C55E" color="#052E16" border-radius="24px" align="center">Creer mon partage</ue-button>
</ue-col></ue-row>
</ue-layout></ue-email>"##;

    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);

        assert!(html.contains("#FFFFFF"), "fond de tuile perdu sur {}", profile.id);
        assert!(html.contains("padding:20px"), "marge de tuile perdue sur {}", profile.id);
        assert!(html.contains("#22C55E"), "couleur de bouton perdue sur {}", profile.id);
        assert!(html.contains("coins.png"), "image perdue sur {}", profile.id);
    }
}
