//! Mise en forme en ligne (`ue-bold`, `ue-italic`) et dimensions d'image.
//!
//! Le gras manquait completement. Un attribut sur `ue-text` n'aurait pas
//! suffi : ce qu'un email demande, c'est « votre **prochain abonnement** »,
//! deux mots au milieu d'une phrase. Il fallait donc une balise imbricable
//! dans le contenu, pas une propriete de tout le paragraphe.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str, client: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile(client).unwrap())
}

#[test]
fn bold_applies_to_a_fragment_in_the_middle_of_a_sentence() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col>
<ue-text>Et si votre <ue-bold>prochain abonnement</ue-bold> etait le votre ?</ue-text>
</ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");

    assert!(html.contains("<strong>prochain abonnement</strong>"), "gras absent : {html}");
    // Le reste de la phrase doit rester hors du gras.
    assert!(html.contains("Et si votre "), "debut de phrase perdu");
    assert!(html.contains(" etait le votre ?"), "fin de phrase perdue");
}

#[test]
fn bold_and_italic_render_on_every_profile() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col>
<ue-text>Sans <ue-italic>frais</ue-italic>. <ue-bold>Jamais</ue-bold>.</ue-text>
</ue-col></ue-row></ue-layout></ue-email>"#;

    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);
        // <strong>/<em> plutot que <b>/<i> : meme rendu partout, moteur Word
        // d'Outlook compris, et le sens est porte pour les lecteurs d'ecran.
        assert!(html.contains("<strong>Jamais</strong>"), "gras perdu sur {}", profile.id);
        assert!(html.contains("<em>frais</em>"), "italique perdu sur {}", profile.id);
    }
}

#[test]
fn inline_formatting_works_inside_headings_and_buttons_too() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col>
<ue-heading level="1">Pourquoi ne pas <ue-bold>economiser</ue-bold> ?</ue-heading>
<ue-button href="https://x.fr">Creer <ue-bold>gratuitement</ue-bold></ue-button>
</ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");

    assert!(html.contains("<strong>economiser</strong>"), "gras absent du titre");
    assert!(html.contains("<strong>gratuitement</strong>"), "gras absent du bouton");
}

#[test]
fn bold_and_italic_nest() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col>
<ue-text><ue-bold>tres <ue-italic>important</ue-italic></ue-bold></ue-text>
</ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");
    assert!(html.contains("<strong>tres <em>important</em></strong>"), "imbrication cassee");
}

#[test]
fn inline_tags_are_rejected_where_they_make_no_sense() {
    // Une balise en ligne directement sous une colonne n'a pas de sens : le
    // parseur doit le dire plutot que produire un HTML bancal.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-bold>x</ue-bold></ue-col></ue-row></ue-layout></ue-email>"#;

    let error = Parser::parse_document(src).unwrap_err().to_string();
    assert!(error.contains("ue-bold"), "erreur peu explicite : {error}");
}

#[test]
fn image_width_attribute_is_a_bare_integer() {
    // L'attribut HTML width attend un entier NU. `width="160px"` est invalide
    // et se fait ignorer : Outlook affiche alors l'image a sa taille native,
    // souvent deux fois trop grande.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="logo.png" alt="Logo" width="160px" /></ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "outlook_desktop");

    assert!(html.contains(r#"width="160""#), "attribut width invalide : {html}");
    assert!(!html.contains(r#"width="160px""#), "unite laissee dans l'attribut");
    // La CSS, elle, garde l'unite.
    assert!(html.contains("width:160px"), "largeur CSS perdue");
}

#[test]
fn a_relative_image_width_omits_the_attribute_rather_than_guessing() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="logo.png" alt="Logo" width="100%" /></ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");

    // Cibler la balise <img> : les tables de mise en page portent elles aussi
    // un width="100%", parfaitement legitime.
    let img = html
        .split("<img")
        .nth(1)
        .expect("aucune balise img generee")
        .split('>')
        .next()
        .unwrap()
        .to_string();

    assert!(img.contains("width:100%"), "largeur CSS perdue : {img}");
    assert!(!img.contains(r#"width="100%""#), "pourcentage place dans l'attribut HTML : {img}");
}
