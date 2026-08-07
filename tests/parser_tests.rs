use uetl_compiler::parser::{AttrValue, Node, ParseError, Parser, UetlTag};

fn find_element(node: &Node, tag: UetlTag) -> Option<&uetl_compiler::parser::ElementNode> {
    match node {
        Node::Element(el) if el.tag == tag => Some(el),
        Node::Element(el) => el.children.iter().find_map(|c| find_element(c, tag)),
        _ => None,
    }
}

#[test]
fn parses_minimal_valid_email() {
    let src = r#"<ue-email lang="fr"><ue-layout><ue-row><ue-col><ue-text>Bonjour</ue-text></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();

    assert_eq!(doc.lang, "fr");
    assert_eq!(doc.children.len(), 1);

    match &doc.children[0] {
        Node::Element(layout) => assert_eq!(layout.tag, UetlTag::Layout),
        other => panic!("expected layout element, got {other:?}"),
    }
}

#[test]
fn errors_when_col_is_outside_row() {
    let src = r#"<ue-email><ue-layout><ue-col></ue-col></ue-layout></ue-email>"#;
    let err = Parser::parse_document(src).unwrap_err();
    assert!(matches!(err, ParseError::InvalidChild { ref child, .. } if child == "ue-col"));
}

#[test]
fn errors_when_button_is_missing_href() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button>Clique</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let err = Parser::parse_document(src).unwrap_err();
    assert!(matches!(err, ParseError::MissingRequiredAttr { ref attr, .. } if attr == "href"));
}

#[test]
fn errors_when_image_is_missing_alt() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-image src="logo.png" /></ue-col></ue-row></ue-layout></ue-email>"#;
    let err = Parser::parse_document(src).unwrap_err();
    assert!(matches!(err, ParseError::MissingRequiredAttr { ref attr, .. } if attr == "alt"));
}

#[test]
fn errors_when_heading_level_is_out_of_range() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-heading level="9">Titre</ue-heading></ue-col></ue-row></ue-layout></ue-email>"#;
    let err = Parser::parse_document(src).unwrap_err();
    assert!(matches!(err, ParseError::InvalidHeadingLevel { .. }));
}

#[test]
fn parses_dynamic_template_attribute() {
    let src = r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="{{cta_url}}">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#;
    let doc = Parser::parse_document(src).unwrap();

    let layout_node = &doc.children[0];
    let button = find_element(layout_node, UetlTag::Button).expect("button not found");

    match button.attrs.get("href") {
        Some(AttrValue::Template(expr)) => assert_eq!(expr, "cta_url"),
        other => panic!("expected template attribute, got {other:?}"),
    }
}

#[test]
fn parses_deeply_nested_rows_and_cols() {
    // layout > row > col > row > col > row > col > text  (7 levels of element nesting)
    let src = r#"<ue-email>
        <ue-layout>
            <ue-row><ue-col>
                <ue-row><ue-col>
                    <ue-row><ue-col>
                        <ue-text>fond</ue-text>
                    </ue-col></ue-row>
                </ue-col></ue-row>
            </ue-col></ue-row>
        </ue-layout>
    </ue-email>"#;

    let doc = Parser::parse_document(src).unwrap();

    fn depth(node: &Node) -> usize {
        match node {
            Node::Element(el) => 1 + el.children.iter().map(depth).max().unwrap_or(0),
            _ => 0,
        }
    }

    let max_depth = doc.children.iter().map(depth).max().unwrap_or(0);
    assert!(max_depth >= 5, "expected at least 5 levels of nesting, got {max_depth}");
}

#[test]
fn divider_and_spacer_are_valid_directly_under_layout() {
    // Cas réel : un séparateur pleine largeur entre deux sections, hors
    // colonne — la place la plus naturelle pour ce composant.
    let src = r##"<ue-email>
        <ue-layout>
            <ue-row><ue-col><ue-text>Haut</ue-text></ue-col></ue-row>
            <ue-divider color="#eeeeee" thickness="1px" margin="20px 0" />
            <ue-spacer height="10px" />
            <ue-row><ue-col><ue-text>Bas</ue-text></ue-col></ue-row>
        </ue-layout>
    </ue-email>"##;

    let doc = Parser::parse_document(src).unwrap();

    assert!(doc.children.iter().any(|c| find_element(c, UetlTag::Divider).is_some()));
    assert!(doc.children.iter().any(|c| find_element(c, UetlTag::Spacer).is_some()));
}

#[test]
fn tolerates_a_trailing_newline_after_the_root_closes() {
    // Coller un document dans un éditeur de texte (Monaco y compris) laisse
    // quasi-systématiquement un saut de ligne final après `</ue-email>` —
    // un vrai document utilisateur a déclenché "expected end of input" sur
    // ce seul caractère avant ce correctif.
    let src = "<ue-email><ue-layout><ue-row><ue-col><ue-text>Bonjour</ue-text></ue-col></ue-row></ue-layout></ue-email>\n";
    Parser::parse_document(src).unwrap();

    let with_blank_lines = "<ue-email><ue-layout><ue-row><ue-col><ue-text>Bonjour</ue-text></ue-col></ue-row></ue-layout></ue-email>\n\n  \n";
    Parser::parse_document(with_blank_lines).unwrap();
}

#[test]
fn errors_when_root_is_not_ue_email() {
    let src = r#"<ue-layout></ue-layout>"#;
    let err = Parser::parse_document(src).unwrap_err();
    assert!(matches!(err, ParseError::RootMustBeEmail { .. }));
}
