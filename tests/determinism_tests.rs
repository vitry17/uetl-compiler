use uetl_compiler::compiler::{collect_warnings, HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

/// Deux attributs inconnus, dans un ordre fixe d'écriture : avant le passage
/// de `ElementNode.attrs` à `IndexMap` (voir `parser::ast`), l'ordre de
/// `collect_warnings` dépendait du hachage aléatoire du process — deux
/// compilations du même source pouvaient produire ces deux avertissements
/// dans un ordre différent.
const SRC_WITH_TWO_UNKNOWN_ATTRS: &str = r#"<ue-email><ue-layout><ue-row><ue-col foo="1" bar="2"><ue-text>Hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;

#[test]
fn compiling_the_same_document_twenty_times_yields_byte_identical_html() {
    let registry = ProfileRegistry::load();
    let profile = registry.get_profile("gmail").unwrap();
    let doc = Parser::parse_document(SRC_WITH_TWO_UNKNOWN_ATTRS).unwrap();
    let first = HtmlGenerator::generate(&doc, profile);

    for _ in 0..20 {
        let doc = Parser::parse_document(SRC_WITH_TWO_UNKNOWN_ATTRS).unwrap();
        let html = HtmlGenerator::generate(&doc, profile);
        assert_eq!(
            html, first,
            "compiling the same source must always produce the same bytes"
        );
    }
}

#[test]
fn warnings_keep_the_source_order_of_attributes_across_twenty_runs() {
    let first_warnings = {
        let doc = Parser::parse_document(SRC_WITH_TWO_UNKNOWN_ATTRS).unwrap();
        collect_warnings(&doc)
    };
    assert_eq!(
        first_warnings.len(),
        2,
        "expected exactly two unknown-attribute warnings"
    );

    for _ in 0..20 {
        let doc = Parser::parse_document(SRC_WITH_TWO_UNKNOWN_ATTRS).unwrap();
        let warnings = collect_warnings(&doc);
        assert_eq!(
            warnings, first_warnings,
            "warning order must not depend on hash-map iteration order"
        );
    }
}

#[test]
fn list_profiles_is_sorted_by_id_and_stable_across_calls() {
    let registry = ProfileRegistry::load();
    let ids: Vec<&str> = registry
        .list_profiles()
        .iter()
        .map(|p| p.id.as_str())
        .collect();

    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "list_profiles() must already be sorted by id");

    for _ in 0..20 {
        let again: Vec<&str> = registry
            .list_profiles()
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            again, ids,
            "list_profiles() order must be stable across calls"
        );
    }
}
