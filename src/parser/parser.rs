use indexmap::IndexMap;

use serde::Serialize;
use serde_json::{json, Map, Value};
use thiserror::Error;

use super::ast::{AttrValue, DarkModeOption, DocumentNode, ElementNode, Node, Span, UetlTag};
use crate::lexer::{LexError, Scanner, Token};

/// Profondeur maximale d'imbrication d'éléments. `parse_element` est
/// récursif sans autre garde-fou : un source de la forme
/// `<ue-bold><ue-bold>…` répété des dizaines de milliers de fois provoque
/// un débordement de pile, qui en Rust n'est pas récupérable — le
/// processus entier est tué, pas seulement la requête en cours. Ce
/// compilateur est partagé par tous les tenants, un seul template hostile
/// (ou un bug côté appelant) coupe la compilation pour tout le monde
/// jusqu'au redémarrage du conteneur. 64 niveaux dépasse largement toute
/// mise en page email réelle (la plus profonde du projet en fait moins de 10).
const MAX_DEPTH: usize = 64;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("lexer error: {0}")]
    Lex(#[from] LexError),

    #[error("expected {expected}, got {actual:?} (line {line}, column {column})")]
    UnexpectedToken {
        expected: String,
        actual: Token,
        line: usize,
        column: usize,
    },

    #[error("unknown tag '<{tag}>' (line {line}, column {column})")]
    UnknownTag {
        tag: String,
        line: usize,
        column: usize,
    },

    #[error("'<{tag}>' was never closed (line {line}, column {column})")]
    UnclosedTag {
        tag: String,
        line: usize,
        column: usize,
    },

    #[error(
        "mismatched closing tag: expected '</{expected}>', got '</{actual}>' (line {line}, column {column})"
    )]
    MismatchedClosingTag {
        expected: String,
        actual: String,
        line: usize,
        column: usize,
    },

    #[error("'<{child}>' is not a valid child of '<{parent}>' (line {line}, column {column})")]
    InvalidChild {
        parent: String,
        child: String,
        line: usize,
        column: usize,
    },

    #[error("root element must be '<ue-email>', found '<{tag}>' (line {line}, column {column})")]
    RootMustBeEmail {
        tag: String,
        line: usize,
        column: usize,
    },

    #[error("'<{tag}>' is missing required attribute '{attr}' (line {line}, column {column})")]
    MissingRequiredAttr {
        tag: String,
        attr: String,
        line: usize,
        column: usize,
    },

    #[error(
        "invalid heading level '{level}': must be between 1 and 6 (line {line}, column {column})"
    )]
    InvalidHeadingLevel {
        level: String,
        line: usize,
        column: usize,
    },

    #[error("template nesting too deep (max {max}) (line {line}, column {column})")]
    TooDeep {
        max: usize,
        line: usize,
        column: usize,
    },
}

impl ParseError {
    /// Position de l'erreur, utilisée par `to_diagnostic` — chaque variante
    /// (y compris `Lex`, dont les champs viennent de `LexError`) porte déjà
    /// une ligne/colonne, ce qui rend cette conversion triviale.
    fn position(&self) -> (usize, usize) {
        match self {
            ParseError::Lex(e) => (e.line, e.column),
            ParseError::UnexpectedToken { line, column, .. }
            | ParseError::UnknownTag { line, column, .. }
            | ParseError::UnclosedTag { line, column, .. }
            | ParseError::MismatchedClosingTag { line, column, .. }
            | ParseError::InvalidChild { line, column, .. }
            | ParseError::RootMustBeEmail { line, column, .. }
            | ParseError::MissingRequiredAttr { line, column, .. }
            | ParseError::InvalidHeadingLevel { line, column, .. }
            | ParseError::TooDeep { line, column, .. } => (*line, *column),
        }
    }

    /// Code machine-readable, stable across wording changes to `Display`
    /// (which callers — the editor's inline squiggles included — should
    /// not depend on parsing).
    fn code(&self) -> &'static str {
        match self {
            ParseError::Lex(_) => "lex_error",
            ParseError::UnexpectedToken { .. } => "unexpected_token",
            ParseError::UnknownTag { .. } => "unknown_tag",
            ParseError::UnclosedTag { .. } => "unclosed_tag",
            ParseError::MismatchedClosingTag { .. } => "mismatched_closing_tag",
            ParseError::InvalidChild { .. } => "invalid_child",
            ParseError::RootMustBeEmail { .. } => "root_must_be_email",
            ParseError::MissingRequiredAttr { .. } => "missing_required_attr",
            ParseError::InvalidHeadingLevel { .. } => "invalid_heading_level",
            ParseError::TooDeep { .. } => "too_deep",
        }
    }

    /// Champs spécifiques à la variante, exposés en JSON à plat aux côtés
    /// de `code`/`message`/`line`/`column` — additif uniquement : le champ
    /// `errors: Vec<String>` existant n'est jamais touché par ce type.
    fn details(&self) -> Map<String, Value> {
        let value = match self {
            ParseError::Lex(_)
            | ParseError::RootMustBeEmail { .. }
            | ParseError::TooDeep { .. } => json!({}),
            ParseError::UnexpectedToken {
                expected, actual, ..
            } => json!({ "expected": expected, "actual": format!("{actual:?}") }),
            ParseError::UnknownTag { tag, .. } => json!({ "tag": tag }),
            ParseError::UnclosedTag { tag, .. } => json!({ "tag": tag }),
            ParseError::MismatchedClosingTag {
                expected, actual, ..
            } => json!({ "expected": expected, "actual": actual }),
            ParseError::InvalidChild { parent, child, .. } => {
                json!({ "parent": parent, "child": child })
            }
            ParseError::MissingRequiredAttr { tag, attr, .. } => {
                json!({ "tag": tag, "attr": attr })
            }
            ParseError::InvalidHeadingLevel { level, .. } => json!({ "level": level }),
        };
        match value {
            Value::Object(map) => map,
            _ => Map::new(),
        }
    }

    pub fn to_diagnostic(&self) -> Diagnostic {
        let (line, column) = self.position();
        Diagnostic {
            code: self.code().to_string(),
            message: self.to_string(),
            line,
            column,
            details: self.details(),
        }
    }
}

/// Représentation structurée d'une erreur de compilation, pensée pour être
/// consommée par un éditeur (squiggles Monaco avec ligne/colonne réelles)
/// plutôt que par une regex sur le message d'erreur textuel. Additive vis-à-vis
/// de `errors: Vec<String>` (jamais retiré des réponses existantes) — voir
/// `ValidateResponse`/`ApiError::ParseFailed` dans `api::handlers`.
#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    #[serde(flatten)]
    pub details: Map<String, Value>,
}

pub struct Parser {
    scanner: Scanner,
    current: Token,
    current_span: Span,
}

impl Parser {
    pub fn parse_document(source: &str) -> Result<DocumentNode, ParseError> {
        let mut parser = Self::new(source)?;
        let root_span = parser.current_span;
        let root = parser.parse_element(None, 0)?;

        if root.tag != UetlTag::Email {
            return Err(ParseError::RootMustBeEmail {
                tag: root.tag.tag_name().to_string(),
                line: root_span.line,
                column: root_span.column,
            });
        }

        // Un saut de ligne final après `</ue-email>` est l'usage normal de
        // tout éditeur de texte (Monaco y compris) — sans cette tolérance,
        // coller un document dans l'éditeur produisait systématiquement
        // une erreur "expected end of input" sur ce seul retour à la ligne
        // de fin de fichier, alors que le document est par ailleurs valide.
        while matches!(&parser.current, Token::Text(text) if text.trim().is_empty())
            || matches!(parser.current, Token::Comment)
        {
            parser.bump()?;
        }

        if !matches!(parser.current, Token::Eof) {
            return Err(parser.unexpected("end of input", parser.current.clone()));
        }

        let lang = match root.attrs.get("lang") {
            Some(AttrValue::String(s)) => s.clone(),
            _ => "en".to_string(),
        };

        let dark_mode = match root.attrs.get("dark-mode") {
            Some(AttrValue::String(s)) if s == "auto" => DarkModeOption::Auto,
            Some(AttrValue::String(s)) if s == "manual" => DarkModeOption::Manual,
            _ => DarkModeOption::Off,
        };

        let font_family = match root.attrs.get("font-family") {
            Some(AttrValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        let preview_text = match root.attrs.get("preview-text") {
            Some(AttrValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        Ok(DocumentNode {
            children: root.children,
            lang,
            dark_mode,
            font_family,
            preview_text,
        })
    }

    fn new(source: &str) -> Result<Self, ParseError> {
        let mut scanner = Scanner::new(source);
        let span = Span {
            line: scanner.line(),
            column: scanner.column(),
        };
        let current = scanner.next_token()?;
        Ok(Self {
            scanner,
            current,
            current_span: span,
        })
    }

    fn bump(&mut self) -> Result<(), ParseError> {
        let span = Span {
            line: self.scanner.line(),
            column: self.scanner.column(),
        };
        self.current = self.scanner.next_token()?;
        self.current_span = span;
        Ok(())
    }

    fn unexpected(&self, expected: &str, actual: Token) -> ParseError {
        ParseError::UnexpectedToken {
            expected: expected.to_string(),
            actual,
            line: self.current_span.line,
            column: self.current_span.column,
        }
    }

    fn parse_element(
        &mut self,
        parent: Option<UetlTag>,
        depth: usize,
    ) -> Result<ElementNode, ParseError> {
        let span = self.current_span;

        if depth >= MAX_DEPTH {
            return Err(ParseError::TooDeep {
                max: MAX_DEPTH,
                line: span.line,
                column: span.column,
            });
        }

        let name = match self.current.clone() {
            Token::TagOpen(name) => name,
            other => return Err(self.unexpected("a tag", other)),
        };

        let tag = UetlTag::from_name(&name).ok_or_else(|| ParseError::UnknownTag {
            tag: name.clone(),
            line: span.line,
            column: span.column,
        })?;

        if let Some(parent_tag) = parent {
            if !parent_tag.allows_child(tag) {
                return Err(ParseError::InvalidChild {
                    parent: parent_tag.tag_name().to_string(),
                    child: name.clone(),
                    line: span.line,
                    column: span.column,
                });
            }
        }

        // Doit être armé avant de consommer le `>` de la balise ouvrante (le bump()
        // suivant), sans quoi le lexer aurait déjà tenté de tokeniser le contenu brut.
        if tag == UetlTag::Raw {
            self.scanner.enter_raw_mode(&name);
        }
        self.bump()?;

        let mut attrs = IndexMap::new();
        while let Token::AttrName(attr_name) = self.current.clone() {
            self.bump()?;
            let value = match self.current.clone() {
                Token::AttrValue(v) => v,
                other => return Err(self.unexpected("an attribute value", other)),
            };
            self.bump()?;
            attrs.insert(attr_name, parse_attr_value(&value));
        }

        let mut children = Vec::new();
        if matches!(self.current, Token::SelfClose) {
            self.bump()?;
        } else {
            loop {
                match self.current.clone() {
                    Token::TagClose(close_name) => {
                        if close_name != name {
                            return Err(ParseError::MismatchedClosingTag {
                                expected: name.clone(),
                                actual: close_name,
                                line: self.current_span.line,
                                column: self.current_span.column,
                            });
                        }
                        self.bump()?;
                        break;
                    }
                    Token::Text(text) => {
                        children.push(Node::Text(text));
                        self.bump()?;
                    }
                    Token::Template(expr) => {
                        children.push(Node::Template(expr));
                        self.bump()?;
                    }
                    Token::Comment => {
                        self.bump()?;
                    }
                    Token::TagOpen(_) => {
                        let child = self.parse_element(Some(tag), depth + 1)?;
                        children.push(Node::Element(child));
                    }
                    Token::Eof => {
                        return Err(ParseError::UnclosedTag {
                            tag: name.clone(),
                            line: span.line,
                            column: span.column,
                        });
                    }
                    other => return Err(self.unexpected("a child node or closing tag", other)),
                }
            }
        }

        validate_attrs(tag, &name, &attrs, span)?;

        Ok(ElementNode {
            tag,
            attrs,
            children,
            span,
        })
    }
}

fn parse_attr_value(value: &str) -> AttrValue {
    let trimmed = value.trim();
    if trimmed.len() >= 4 && trimmed.starts_with("{{") && trimmed.ends_with("}}") {
        AttrValue::Template(trimmed[2..trimmed.len() - 2].trim().to_string())
    } else {
        AttrValue::String(value.to_string())
    }
}

fn validate_attrs(
    tag: UetlTag,
    name: &str,
    attrs: &IndexMap<String, AttrValue>,
    span: Span,
) -> Result<(), ParseError> {
    match tag {
        UetlTag::Button => require_attr(name, attrs, "href", span),
        UetlTag::Image => {
            require_attr(name, attrs, "src", span)?;
            require_attr(name, attrs, "alt", span)
        }
        UetlTag::Heading => validate_heading_level(name, attrs, span),
        // Sans image de fond, un hero n'est qu'une ligne : autant utiliser
        // `ue-row`, dont le rendu est plus simple et mieux supporte.
        UetlTag::Hero => require_attr(name, attrs, "src", span),
        _ => Ok(()),
    }
}

fn require_attr(
    name: &str,
    attrs: &IndexMap<String, AttrValue>,
    attr: &str,
    span: Span,
) -> Result<(), ParseError> {
    if attrs.contains_key(attr) {
        Ok(())
    } else {
        Err(ParseError::MissingRequiredAttr {
            tag: name.to_string(),
            attr: attr.to_string(),
            line: span.line,
            column: span.column,
        })
    }
}

fn validate_heading_level(
    name: &str,
    attrs: &IndexMap<String, AttrValue>,
    span: Span,
) -> Result<(), ParseError> {
    match attrs.get("level") {
        Some(AttrValue::Template(_)) => Ok(()),
        Some(AttrValue::String(level)) => match level.parse::<u8>() {
            Ok(1..=6) => Ok(()),
            _ => Err(ParseError::InvalidHeadingLevel {
                level: level.clone(),
                line: span.line,
                column: span.column,
            }),
        },
        _ => Err(ParseError::MissingRequiredAttr {
            tag: name.to_string(),
            attr: "level".to_string(),
            line: span.line,
            column: span.column,
        }),
    }
}
