#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    TagOpen(String),
    TagClose(String),
    SelfClose,
    AttrName(String),
    AttrValue(String),
    Text(String),
    Template(String),
    Comment,
    Eof,
}
