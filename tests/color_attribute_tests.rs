//! `color` et `color-light` doivent etre interchangeables.
//!
//! Le generateur ne lisait que `color-light` sur `ue-heading` et `ue-text`,
//! alors que la reference du langage annonce `color`. Un titre ecrit
//! `<ue-heading color="#05073B">` sortait donc en couleur par defaut, sans le
//! moindre message : la charte graphique etait silencieusement ignoree, et
//! rien ne permettait de comprendre pourquoi.

use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

fn render(src: &str) -> String {
    let doc = Parser::parse_document(src).unwrap();
    let registry = ProfileRegistry::load();
    HtmlGenerator::generate(&doc, registry.get_profile("gmail").unwrap())
}

#[test]
fn heading_honours_plain_color() {
    let src = r##"<ue-email><ue-layout><ue-row><ue-col>
<ue-heading level="1" color="#05073B">Titre</ue-heading>
</ue-col></ue-row></ue-layout></ue-email>"##;

    assert!(render(src).contains("color:#05073B"), "couleur de titre ignoree");
}

#[test]
fn text_honours_plain_color() {
    let src = r##"<ue-email><ue-layout><ue-row><ue-col>
<ue-text color="#1F2937">Texte</ue-text>
</ue-col></ue-row></ue-layout></ue-email>"##;

    assert!(render(src).contains("color:#1F2937"), "couleur de texte ignoree");
}

#[test]
fn color_light_still_wins_when_both_are_present() {
    // `-light` est le plus specifique : il decrit explicitement le theme clair.
    let src = r##"<ue-email><ue-layout><ue-row><ue-col>
<ue-text color="#000000" color-light="#1F2937">Texte</ue-text>
</ue-col></ue-row></ue-layout></ue-email>"##;

    let html = render(src);
    assert!(html.contains("color:#1F2937"), "color-light ignore");
    assert!(!html.contains("color:#000000"), "color a pris le dessus sur color-light");
}
