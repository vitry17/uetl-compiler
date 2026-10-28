use uetl_compiler::parser::Parser;

#[test]
fn a_valid_document_has_no_errors_and_produces_a_document() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-text>Hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert!(errors.is_empty());
    assert!(document.is_some());
}

#[test]
fn two_sibling_buttons_missing_href_are_both_reported_in_one_pass() {
    // Sans recuperation, seule la premiere erreur remonterait et il
    // faudrait corriger-puis-relancer /validate pour decouvrir la seconde.
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-button>Go</ue-button></ue-col>
        <ue-col><ue-button>Also go</ue-button></ue-col>
    </ue-row></ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 2, "{errors:?}");
    for e in &errors {
        assert_eq!(e.to_diagnostic().code, "missing_required_attr");
    }
    // Le document reste construit (racine valide) malgre les deux enfants
    // fautifs, qui sont simplement absents de l'arbre.
    assert!(document.is_some());
}

#[test]
fn two_unknown_sibling_tags_are_both_reported_and_skipped() {
    let src = r#"<ue-email><ue-layout>
        <ue-row><ue-col><ue-not-a-real-tag>oops</ue-not-a-real-tag></ue-col></ue-row>
        <ue-row><ue-col><ue-also-fake>oops again</ue-also-fake></ue-col></ue-row>
    </ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 2, "{errors:?}");
    for e in &errors {
        assert_eq!(e.to_diagnostic().code, "unknown_tag");
    }
    assert!(document.is_some());
}

#[test]
fn recovery_after_an_unknown_tag_does_not_swallow_the_next_valid_sibling() {
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-not-a-real-tag>oops</ue-not-a-real-tag></ue-col>
        <ue-col><ue-text>I am fine</ue-text></ue-col>
    </ue-row></ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 1);
    let doc = document.expect("root should still parse");
    let html = format!("{doc:?}");
    assert!(
        html.contains("I am fine"),
        "the valid sibling must survive: {html}"
    );
    assert!(
        !html.contains("oops"),
        "the skipped element must not leak into the tree: {html}"
    );
}

#[test]
fn an_invalid_child_placement_is_recovered_the_same_way() {
    // <ue-col> n'est valide que sous <ue-row> : en placer un directement
    // sous <ue-layout> a cote d'une ligne valide doit signaler l'erreur
    // sans empecher le reste du document d'etre analyse.
    let src = r#"<ue-email><ue-layout>
        <ue-col><ue-text>misplaced</ue-text></ue-col>
        <ue-row><ue-col><ue-text>fine</ue-text></ue-col></ue-row>
    </ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].to_diagnostic().code, "invalid_child");
    assert!(document.is_some());
}

#[test]
fn an_invalid_heading_level_is_recovered_without_a_skip() {
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-heading level="9">Too big</ue-heading></ue-col>
        <ue-col><ue-heading level="2">Fine</ue-heading></ue-col>
    </ue-row></ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].to_diagnostic().code, "invalid_heading_level");
    let doc = document.expect("root should still parse");
    assert!(format!("{doc:?}").contains("Fine"));
}

#[test]
fn errors_are_recovered_at_any_nesting_depth_not_just_top_level_siblings() {
    let src = r#"<ue-email><ue-layout>
        <ue-row><ue-col><ue-button>Missing href</ue-button></ue-col></ue-row>
        <ue-row><ue-col><ue-text>Deeply fine</ue-text></ue-col></ue-row>
    </ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert_eq!(errors.len(), 1);
    assert!(document.is_some());
}

#[test]
fn a_structural_error_is_still_fatal_but_keeps_earlier_recovered_diagnostics() {
    // Le premier ue-col a une erreur locale recuperable ; le second n'est
    // jamais ferme (erreur structurelle) - la balise ouvrante racine n'est
    // donc jamais consommee jusqu'a EOF et le document ne peut pas etre
    // construit, mais l'erreur locale deja trouvee doit rester dans la liste.
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-button>Missing href</ue-button></ue-col>
        <ue-col><ue-text>Never closed
    </ue-row></ue-layout></ue-email>"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert!(document.is_none());
    assert!(errors.len() >= 2, "{errors:?}");
    assert_eq!(errors[0].to_diagnostic().code, "missing_required_attr");
    // La derniere erreur est la cause fatale de l'echec du document : ici,
    // `<ue-text>` n'est jamais refermee, donc la prochaine fermeture
    // rencontree (`</ue-row>`) ne correspond pas au nom attendu.
    let last_code = errors.last().unwrap().to_diagnostic().code;
    assert_eq!(last_code, "mismatched_closing_tag", "{errors:?}");
}

#[test]
fn a_document_truncated_at_eof_is_reported_as_unclosed_after_earlier_recovery() {
    let src = r#"<ue-email><ue-layout><ue-row>
        <ue-col><ue-button>Missing href</ue-button></ue-col>
        <ue-col><ue-text>Truncated here, no closing tags at all"#;
    let (document, errors) = Parser::parse_document_tolerant(src);
    assert!(document.is_none());
    assert!(errors.len() >= 2, "{errors:?}");
    assert_eq!(errors[0].to_diagnostic().code, "missing_required_attr");
    assert_eq!(errors.last().unwrap().to_diagnostic().code, "unclosed_tag");
}

#[test]
fn tolerant_parsing_of_a_valid_document_produces_the_same_warnings_as_strict() {
    use uetl_compiler::compiler::collect_warnings;

    let src = r#"<ue-email><ue-layout><ue-row><ue-col unknown-attr="x"><ue-text>Hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let strict = Parser::parse_document(src).unwrap();
    let (tolerant, errors) = Parser::parse_document_tolerant(src);
    assert!(errors.is_empty());
    assert_eq!(
        collect_warnings(&strict),
        collect_warnings(&tolerant.unwrap())
    );
}
