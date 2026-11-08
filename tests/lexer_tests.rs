use uetl_compiler::lexer::{Scanner, Token};

fn tokenize(src: &str) -> Result<Vec<Token>, uetl_compiler::lexer::LexError> {
    let mut scanner = Scanner::new(src);
    let mut tokens = Vec::new();
    loop {
        let token = scanner.next_token()?;
        let is_eof = token == Token::Eof;
        tokens.push(token);
        if is_eof {
            break;
        }
    }
    Ok(tokens)
}

#[test]
fn tokenizes_simple_tag() {
    let tokens = tokenize("<ue-text></ue-text>").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::TagOpen("ue-text".into()),
            Token::TagClose("ue-text".into()),
            Token::Eof,
        ]
    );
}

#[test]
fn tokenizes_attribute_with_value() {
    let tokens = tokenize(r#"<ue-button href="https://example.com"></ue-button>"#).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::TagOpen("ue-button".into()),
            Token::AttrName("href".into()),
            Token::AttrValue("https://example.com".into()),
            Token::TagClose("ue-button".into()),
            Token::Eof,
        ]
    );
}

#[test]
fn tokenizes_self_closing_tag() {
    let tokens = tokenize(r#"<ue-image src="logo.png" />"#).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::TagOpen("ue-image".into()),
            Token::AttrName("src".into()),
            Token::AttrValue("logo.png".into()),
            Token::SelfClose,
            Token::Eof,
        ]
    );
}

#[test]
fn tokenizes_template_variable() {
    let tokens = tokenize("{{prenom}}").unwrap();
    assert_eq!(tokens, vec![Token::Template("prenom".into()), Token::Eof]);
}

#[test]
fn tokenizes_text_mixed_with_template() {
    let tokens = tokenize("Bonjour {{prenom}},").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Text("Bonjour ".into()),
            Token::Template("prenom".into()),
            Token::Text(",".into()),
            Token::Eof,
        ]
    );
}

#[test]
fn tokenizes_nested_tags() {
    let tokens = tokenize("<ue-row><ue-col></ue-col></ue-row>").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::TagOpen("ue-row".into()),
            Token::TagOpen("ue-col".into()),
            Token::TagClose("ue-col".into()),
            Token::TagClose("ue-row".into()),
            Token::Eof,
        ]
    );
}

#[test]
fn tokenizes_comment() {
    let tokens = tokenize("<!-- hello -->").unwrap();
    assert_eq!(tokens, vec![Token::Comment, Token::Eof]);
}

#[test]
fn errors_on_unclosed_tag() {
    let err = tokenize("<ue-text").unwrap_err();
    assert!(err.message.contains("end of input"));
}

#[test]
fn errors_on_attribute_without_value() {
    let err = tokenize(r#"<ue-button href></ue-button>"#).unwrap_err();
    assert!(err.message.contains("missing a value"));
}

#[test]
fn errors_on_unterminated_comment() {
    let err = tokenize("<!-- never closed").unwrap_err();
    assert!(err.message.contains("unterminated comment"));
}

#[test]
fn errors_on_unterminated_template() {
    let err = tokenize("{{prenom").unwrap_err();
    assert!(err.message.contains("unterminated template"));
}

#[test]
fn errors_on_unterminated_attribute_value() {
    let err = tokenize(r#"<ue-button href="https://example.com"#).unwrap_err();
    assert!(err.message.contains("unterminated attribute value"));
}

// Une balise HTML parasite (non `ue-`) doit être tokenisée comme une vraie
// balise, pas avalée comme du texte : le parser la rejette ensuite en
// `unknown_tag`. Sans ça, un fragment `<div>x</div>` passait pour valide
// (voir `Unimail.Templates.BlocksTest`).
#[test]
fn tokenizes_a_bare_html_tag_as_a_tag_not_text() {
    let tokens = tokenize("<div>x</div>").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::TagOpen("div".into()),
            Token::Text("x".into()),
            Token::TagClose("div".into()),
            Token::Eof,
        ]
    );
}

// À l'inverse, un `<` qui n'est pas suivi d'une lettre reste du texte
// littéral : inégalités mathématiques, `<=`, etc. ne doivent pas déclencher
// « expected tag name after '<' ».
#[test]
fn keeps_a_non_tag_left_angle_as_text() {
    let tokens = tokenize("2 < 3 and a <= b").unwrap();
    assert_eq!(
        tokens,
        vec![Token::Text("2 < 3 and a <= b".into()), Token::Eof]
    );
}
