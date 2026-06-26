use super::token::Token;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
#[error("{message} (line {line}, column {column})")]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

pub struct Scanner {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    in_tag: bool,
    pending_attr_value: bool,
    /// Quand renseigné, le contenu jusqu'à la prochaine balise fermante portant ce nom
    /// est capturé tel quel (un seul `Token::Text`), sans être tokenisé. Permet à
    /// `<ue-raw>` d'embarquer du HTML littéral sans que le lexer s'y casse les dents.
    raw_mode: Option<String>,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            in_tag: false,
            pending_attr_value: false,
            raw_mode: None,
        }
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }

    /// À appeler juste après avoir vu `Token::TagOpen(name)` pour une balise dont le
    /// contenu doit être lu tel quel (ex: `ue-raw`), avant de consommer ses attributs.
    pub fn enter_raw_mode(&mut self, closing_tag: &str) {
        self.raw_mode = Some(closing_tag.to_string());
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        if self.in_tag {
            return self.next_token_in_tag();
        }

        match self.peek() {
            None => Ok(Token::Eof),
            Some('<') => self.read_left_angle(),
            Some('{') if self.peek_at(1) == Some('{') => self.read_template(),
            _ => self.read_text(),
        }
    }

    fn next_token_in_tag(&mut self) -> Result<Token, LexError> {
        if self.pending_attr_value {
            self.pending_attr_value = false;
            return self.read_attr_value();
        }

        self.skip_whitespace();
        match self.peek() {
            None => Err(self.error("unexpected end of input inside tag")),
            Some('/') if self.peek_at(1) == Some('>') => {
                self.advance();
                self.advance();
                self.in_tag = false;
                self.raw_mode = None;
                Ok(Token::SelfClose)
            }
            Some('>') => {
                self.advance();
                self.in_tag = false;
                match self.raw_mode.take() {
                    Some(closing_tag) => self.read_raw_text(&closing_tag),
                    None => self.next_token(),
                }
            }
            Some(c) if is_name_start(c) => self.read_attr_name(),
            Some(c) => Err(self.error(format!("unexpected character '{c}' in tag"))),
        }
    }

    /// Capture tout le texte jusqu'à (sans la consommer) la prochaine occurrence de
    /// `</closing_tag`, sans interpréter quoi que ce soit entre les deux.
    fn read_raw_text(&mut self, closing_tag: &str) -> Result<Token, LexError> {
        let start = self.error("unterminated raw block");
        let closer = format!("</{closing_tag}");
        let mut content = String::new();
        loop {
            if self.starts_with(&closer) {
                return Ok(Token::Text(content));
            }
            match self.advance() {
                Some(c) => content.push(c),
                None => return Err(start),
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn starts_with(&self, s: &str) -> bool {
        s.chars()
            .enumerate()
            .all(|(i, c)| self.peek_at(i) == Some(c))
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.advance();
        }
    }

    fn error(&self, message: impl Into<String>) -> LexError {
        LexError {
            message: message.into(),
            line: self.line,
            column: self.column,
        }
    }

    fn read_left_angle(&mut self) -> Result<Token, LexError> {
        if self.starts_with("<!--") {
            self.read_comment()
        } else if self.peek_at(1) == Some('/') {
            self.read_closing_tag()
        } else {
            self.read_opening_tag()
        }
    }

    fn read_comment(&mut self) -> Result<Token, LexError> {
        let start = self.error("unterminated comment");
        for _ in 0..4 {
            self.advance();
        }
        loop {
            if self.starts_with("-->") {
                self.advance();
                self.advance();
                self.advance();
                return Ok(Token::Comment);
            }
            if self.advance().is_none() {
                return Err(start);
            }
        }
    }

    fn read_opening_tag(&mut self) -> Result<Token, LexError> {
        let start = self.error("expected tag name after '<'");
        self.advance(); // consume '<'
        let mut name = String::new();
        while matches!(self.peek(), Some(c) if is_name_char(c)) {
            name.push(self.advance().unwrap());
        }
        if name.is_empty() {
            return Err(start);
        }
        self.in_tag = true;
        Ok(Token::TagOpen(name))
    }

    fn read_closing_tag(&mut self) -> Result<Token, LexError> {
        let start = self.error("unterminated closing tag");
        self.advance(); // '<'
        self.advance(); // '/'
        let mut name = String::new();
        while matches!(self.peek(), Some(c) if is_name_char(c)) {
            name.push(self.advance().unwrap());
        }
        if name.is_empty() {
            return Err(start);
        }
        self.skip_whitespace();
        match self.peek() {
            Some('>') => {
                self.advance();
                Ok(Token::TagClose(name))
            }
            _ => Err(start),
        }
    }

    fn read_attr_name(&mut self) -> Result<Token, LexError> {
        let mut name = String::new();
        while matches!(self.peek(), Some(c) if is_name_char(c)) {
            name.push(self.advance().unwrap());
        }
        self.skip_whitespace();
        if self.peek() != Some('=') {
            return Err(self.error(format!("attribute '{name}' is missing a value")));
        }
        self.advance(); // '='
        self.skip_whitespace();
        self.pending_attr_value = true;
        Ok(Token::AttrName(name))
    }

    fn read_attr_value(&mut self) -> Result<Token, LexError> {
        let start = self.error("expected quoted attribute value");
        let quote = match self.peek() {
            Some(q) if q == '"' || q == '\'' => q,
            _ => return Err(start),
        };
        self.advance(); // opening quote
        let mut value = String::new();
        loop {
            match self.peek() {
                Some(c) if c == quote => {
                    self.advance();
                    return Ok(Token::AttrValue(value));
                }
                Some(_) => value.push(self.advance().unwrap()),
                None => return Err(self.error("unterminated attribute value")),
            }
        }
    }

    fn read_template(&mut self) -> Result<Token, LexError> {
        let start = self.error("unterminated template expression");
        self.advance();
        self.advance();
        let mut content = String::new();
        loop {
            if self.peek() == Some('}') && self.peek_at(1) == Some('}') {
                self.advance();
                self.advance();
                return Ok(Token::Template(content.trim().to_string()));
            }
            match self.advance() {
                Some(c) => content.push(c),
                None => return Err(start),
            }
        }
    }

    fn read_text(&mut self) -> Result<Token, LexError> {
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some('<') => break,
                Some('{') if self.peek_at(1) == Some('{') => break,
                Some(_) => text.push(self.advance().unwrap()),
            }
        }
        Ok(Token::Text(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mode_captures_literal_tags_without_tokenizing_them() {
        let mut scanner = Scanner::new("<ue-raw><div>hi</div></ue-raw>");
        assert_eq!(
            scanner.next_token().unwrap(),
            Token::TagOpen("ue-raw".into())
        );
        scanner.enter_raw_mode("ue-raw");
        assert_eq!(
            scanner.next_token().unwrap(),
            Token::Text("<div>hi</div>".into())
        );
        assert_eq!(
            scanner.next_token().unwrap(),
            Token::TagClose("ue-raw".into())
        );
    }

    #[test]
    fn raw_mode_is_cleared_by_a_self_closing_tag() {
        let mut scanner = Scanner::new("<ue-raw /><ue-text>after</ue-text>");
        assert_eq!(
            scanner.next_token().unwrap(),
            Token::TagOpen("ue-raw".into())
        );
        scanner.enter_raw_mode("ue-raw");
        assert_eq!(scanner.next_token().unwrap(), Token::SelfClose);
        assert_eq!(
            scanner.next_token().unwrap(),
            Token::TagOpen("ue-text".into())
        );
    }
}
