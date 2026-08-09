//! `ue-hero` : du texte par-dessus une image de fond.
//!
//! Une image de fond en email n'est pas une propriete CSS qu'on pose et qui
//! marche. Outlook ignore `background-image`, et une bonne part des
//! destinataires bloquent le chargement des images. Trois mecanismes se
//! superposent donc, et ces tests verifient qu'aucun ne manque : chacun
//! couvre ce que le precedent laisse decouvert, et il n'en existe pas de
//! quatrieme.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str, client: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile(client).unwrap())
}

const HERO: &str = r##"<ue-email><ue-layout>
<ue-hero src="https://cdn.exemple.fr/hero.jpg" background="#0F1B33" height="420px" align="center">
<ue-heading level="1" color="#FFFFFF" align="center">Un titre par-dessus l'image</ue-heading>
<ue-button href="https://exemple.fr" align="center">Decouvrir</ue-button>
</ue-hero>
</ue-layout></ue-email>"##;

#[test]
fn outlook_gets_a_vml_rectangle_with_the_content_inside_it() {
    // Le moteur Word ignore background-image : le rectangle VML est la SEULE
    // technique qui fasse tenir du texte par-dessus une image chez lui.
    let html = render(HERO, "outlook_desktop");

    assert!(html.contains("<v:rect"), "rectangle VML absent : {html}");
    assert!(html.contains(r#"type="frame""#), "remplissage VML mal declare");
    assert!(html.contains("<v:textbox"), "textbox VML absente");

    // Le contenu doit se trouver DANS la textbox, sinon il s'affiche sous
    // l'image au lieu d'etre par-dessus.
    let inside = html
        .split("<v:textbox")
        .nth(1)
        .expect("textbox introuvable")
        .split("</v:textbox>")
        .next()
        .unwrap();

    assert!(inside.contains("Un titre par-dessus l'image"), "titre hors de la textbox");
    assert!(inside.contains("Decouvrir"), "bouton hors de la textbox");
}

#[test]
fn the_vml_rectangle_carries_explicit_pixel_dimensions() {
    // VML ne connait ni pourcentage ni dimension automatique : sans taille en
    // pixels, le rectangle ne s'affiche pas du tout.
    let html = render(HERO, "outlook_desktop");

    assert!(html.contains("height:420px"), "hauteur absente du rectangle VML");
    assert!(html.contains("width:600px"), "largeur par defaut absente");
}

#[test]
fn clients_without_vml_do_not_receive_the_conditional_markup() {
    let html = render(HERO, "gmail");

    assert!(!html.contains("<v:rect"), "VML emis pour un client qui l'ignore");
    assert!(html.contains("background-image:url("), "image de fond CSS absente");
    assert!(html.contains("background-size:cover"), "cadrage de l'image absent");
}

#[test]
fn the_fallback_colour_is_always_present() {
    // C'est la seule chose visible quand le destinataire bloque les images —
    // et c'est elle qui decide si le texte reste lisible.
    let registry = ProfileRegistry::load();
    let doc = Parser::parse_document(HERO).unwrap();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);

        assert!(
            html.contains(r##"bgcolor="#0F1B33""##),
            "couleur de repli absente sur {}",
            profile.id
        );
    }
}

#[test]
fn the_background_attribute_doubles_the_css_declaration() {
    // Certains clients honorent l'attribut HTML, d'autres la declaration CSS :
    // n'en emettre qu'un laisse une partie du parc sans image de fond.
    let html = render(HERO, "apple_mail");

    assert!(html.contains(r#"background="https://cdn.exemple.fr/hero.jpg""#));
    assert!(html.contains("background-image:url('https://cdn.exemple.fr/hero.jpg')"));
}

#[test]
fn a_hero_without_a_source_is_rejected() {
    // Sans image de fond, un hero n'est qu'une ligne : autant utiliser
    // `ue-row`, dont le rendu est plus simple et mieux supporte.
    let src = r#"<ue-email><ue-layout><ue-hero><ue-text>x</ue-text></ue-hero></ue-layout></ue-email>"#;

    let error = Parser::parse_document(src).unwrap_err().to_string();
    assert!(error.contains("src"), "erreur peu explicite : {error}");
}

#[test]
fn a_hero_is_only_valid_directly_under_the_layout() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-hero src="x.jpg"></ue-hero></ue-col></ue-row></ue-layout></ue-email>"#;

    assert!(Parser::parse_document(src).is_err(), "hero accepte dans une colonne");
}

#[test]
fn every_profile_renders_a_hero_without_losing_its_content() {
    let registry = ProfileRegistry::load();
    let doc = Parser::parse_document(HERO).unwrap();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);

        assert!(
            html.contains("Un titre par-dessus l'image"),
            "titre perdu sur {}",
            profile.id
        );
        assert!(html.contains("Decouvrir"), "bouton perdu sur {}", profile.id);
    }
}
