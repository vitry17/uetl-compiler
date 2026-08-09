//! Colonnes cote a cote sur Outlook, et images qui ne debordent pas.
//!
//! Un client sans media queries recevait un empilement PERMANENT : sur Outlook
//! Desktop, deux colonnes cote a cote se retrouvaient l'une sous l'autre, en
//! `<tr>` separes. Le raisonnement etait inverse — « pas de media queries,
//! donc on ne saura pas empiler sur mobile, donc on empile toujours ». Outlook
//! Desktop est un client de BUREAU : il n'est jamais le cas mobile, et il rend
//! parfaitement des colonnes en tableau.
//!
//! C'est le cas le plus vendeur du produit — grille produits, bloc image a
//! gauche et texte a droite — et c'etait precisement celui qui cassait.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str, client: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile(client).unwrap())
}

/// Nombre de lignes de tableau : deux colonnes cote a cote n'en font qu'une.
fn row_count(html: &str) -> usize {
    html.matches("<tr>").count()
}

const DEUX_COLONNES: &str = r##"<ue-email><ue-layout><ue-row gap="16px" stack-on="mobile">
<ue-col><ue-image src="visuel.png" alt="Visuel" width="260px" /></ue-col>
<ue-col><ue-text>Le texte a droite du visuel.</ue-text></ue-col>
</ue-row></ue-layout></ue-email>"##;

#[test]
fn outlook_keeps_columns_side_by_side() {
    let html = render(DEUX_COLONNES, "outlook_desktop");

    // Deux <td> dans la meme <tr> : cote a cote. Deux <tr> : empiles.
    let row = html
        .split("<tr>")
        .find(|chunk| chunk.contains("ue-col"))
        .expect("aucune ligne de colonnes generee");

    assert_eq!(
        row.matches("class=\"ue-col\"").count(),
        2,
        "les colonnes ne sont pas dans la meme ligne : {html}"
    );
}

#[test]
fn every_profile_lays_two_columns_out_the_same_way() {
    // Aucun client ne doit empiler de son propre chef : c'est la media query
    // qui decide, a la largeur reelle de la fenetre.
    let registry = ProfileRegistry::load();
    let doc = Parser::parse_document(DEUX_COLONNES).unwrap();

    for profile in registry.list_profiles() {
        let html = HtmlGenerator::generate(&doc, profile);

        assert!(
            html.contains("Le texte a droite du visuel."),
            "contenu perdu sur {}",
            profile.id
        );

        // Le rendu flexbox n'utilise pas de tableau : on ne compte les lignes
        // que la ou la mise en page en emploie un.
        if !html.contains("display:flex") {
            assert!(
                row_count(html.as_str()) <= 3,
                "colonnes empilees sur {} : {} lignes de tableau",
                profile.id,
                row_count(html.as_str())
            );
        }
    }
}

#[test]
fn stacking_still_happens_through_the_media_query_where_supported() {
    // `stack-on="mobile"` reste honore : simplement par la media query, a la
    // largeur reelle de la fenetre, et non par une decision prise au build.
    let html = render(DEUX_COLONNES, "gmail");

    assert!(html.contains("@media (max-width:600px)"), "media query absente");
    assert!(html.contains("display:block!important"), "regle d'empilement absente");
}

#[test]
fn an_image_never_overflows_its_container() {
    // Une largeur fixe en pixels debordait de l'ecran sur mobile : le contenu
    // se retrouvait coupe, ou l'email force en zoom arriere.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="hero.png" alt="Hero" width="600px" /></ue-col></ue-row></ue-layout></ue-email>"#;

    for client in ["gmail", "outlook_desktop", "apple_mail"] {
        let html = render(src, client);
        assert!(html.contains("max-width:100%"), "image non contrainte sur {client}");
        assert!(html.contains("height:auto"), "hauteur non proportionnelle sur {client}");
    }
}

#[test]
fn an_explicit_height_wins_over_the_automatic_one() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="h.png" alt="H" width="200px" height="120px" /></ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");

    assert!(html.contains("height:120px"), "hauteur explicite perdue");
    assert!(!html.contains("height:auto"), "hauteur automatique imposee malgre le choix de l'auteur");
}

#[test]
fn a_percentage_width_stays_out_of_the_html_attribute() {
    // L'attribut HTML width attend un entier nu : un pourcentage y serait
    // invalide et ignore. La CSS, elle, le porte tres bien.
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="p.png" alt="P" width="100%" /></ue-col></ue-row></ue-layout></ue-email>"#;

    let html = render(src, "gmail");
    let img = html.split("<img").nth(1).unwrap().split('>').next().unwrap();

    assert!(img.contains("width:100%"), "largeur en pourcentage perdue : {img}");
    assert!(!img.contains(r#"width="100%""#), "pourcentage place dans l'attribut HTML");
}
