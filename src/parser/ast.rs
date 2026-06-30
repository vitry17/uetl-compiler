use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Node {
    Document(DocumentNode),
    Element(ElementNode),
    Text(String),
    Template(String),
}

#[derive(Debug, Clone)]
pub struct DocumentNode {
    pub children: Vec<Node>,
    pub lang: String,
    pub dark_mode: DarkModeOption,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DarkModeOption {
    Auto,
    Manual,
    Off,
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct ElementNode {
    pub tag: UetlTag,
    pub attrs: HashMap<String, AttrValue>,
    pub children: Vec<Node>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UetlTag {
    Email,
    Layout,
    Row,
    Col,
    Heading,
    Text,
    Button,
    Image,
    Divider,
    Spacer,
    Interactive,
    Raw,
}

impl UetlTag {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ue-email" => Some(Self::Email),
            "ue-layout" => Some(Self::Layout),
            "ue-row" => Some(Self::Row),
            "ue-col" => Some(Self::Col),
            "ue-heading" => Some(Self::Heading),
            "ue-text" => Some(Self::Text),
            "ue-button" => Some(Self::Button),
            "ue-image" => Some(Self::Image),
            "ue-divider" => Some(Self::Divider),
            "ue-spacer" => Some(Self::Spacer),
            "ue-interactive" => Some(Self::Interactive),
            "ue-raw" => Some(Self::Raw),
            _ => None,
        }
    }

    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Email => "ue-email",
            Self::Layout => "ue-layout",
            Self::Row => "ue-row",
            Self::Col => "ue-col",
            Self::Heading => "ue-heading",
            Self::Text => "ue-text",
            Self::Button => "ue-button",
            Self::Image => "ue-image",
            Self::Divider => "ue-divider",
            Self::Spacer => "ue-spacer",
            Self::Interactive => "ue-interactive",
            Self::Raw => "ue-raw",
        }
    }

    /// Validation sémantique : `<ue-col>` n'est valide que sous `<ue-row>`, etc.
    ///
    /// `Divider`/`Spacer` sont acceptés à la fois sous `Layout` (séparateur
    /// ou espacement pleine largeur entre deux sections, l'usage le plus
    /// courant) et sous `Col` (séparateur localisé à une colonne) : les deux
    /// génèrent un fragment autonome (`<hr>` / `<table>` de remplissage)
    /// simplement concaténé par `gen_layout`/`gen_col`, donc aucune
    /// structure de tableau ne dépend de l'endroit où ils apparaissent.
    pub fn allows_child(&self, child: Self) -> bool {
        use UetlTag::*;
        matches!(
            (self, child),
            (Email, Layout)
                | (Layout, Row | Divider | Spacer)
                | (Row, Col)
                | (
                    Col,
                    Heading | Text | Image | Button | Divider | Spacer | Interactive | Raw | Row
                )
        )
    }
}

#[derive(Debug, Clone)]
pub enum AttrValue {
    String(String),
    Template(String),
    Bool(bool),
}
