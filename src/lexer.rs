use crate::token::{Span, Spanned, StringPart, Token};

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            self.skip_comment();
            self.skip_whitespace();

            if self.at_end() {
                tokens.push(self.spanned(Token::Eof));
                break;
            }

            let tok = self.next_token()?;
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Spanned, String> {
        let ch = self.peek();
        match ch {
            '\n' => {
                let sp = self.spanned(Token::Newline);
                self.advance();
                Ok(sp)
            }
            '"' => self.read_string(),
            '#' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && self.peek() == '{' {
                    self.advance();
                    Ok(Spanned { token: Token::HashBrace, span })
                } else {
                    Err(format!("Unexpected '#' at {}:{}, did you mean '#{{' for a map?", span.line, span.col))
                }
            }
            ':' => {
                let span = self.current_span();
                self.advance();
                if self.at_end() || self.peek().is_whitespace() || self.peek() == '\n' {
                    Ok(Spanned { token: Token::Colon, span })
                } else {
                    let name = self.read_ident_str();
                    Ok(Spanned { token: Token::Symbol(name), span })
                }
            }
            ';' => { let sp = self.spanned(Token::Semicolon); self.advance(); Ok(sp) }
            '[' => { let sp = self.spanned(Token::LBracket); self.advance(); Ok(sp) }
            ']' => { let sp = self.spanned(Token::RBracket); self.advance(); Ok(sp) }
            '{' => { let sp = self.spanned(Token::LBrace); self.advance(); Ok(sp) }
            '}' => { let sp = self.spanned(Token::RBrace); self.advance(); Ok(sp) }
            '(' => { let sp = self.spanned(Token::LParen); self.advance(); Ok(sp) }
            ')' => { let sp = self.spanned(Token::RParen); self.advance(); Ok(sp) }
            ',' => { let sp = self.spanned(Token::Comma); self.advance(); Ok(sp) }
            '.' => { let sp = self.spanned(Token::Dot); self.advance(); Ok(sp) }
            '~' => { let sp = self.spanned(Token::Tilde); self.advance(); Ok(sp) }
            '+' => { let sp = self.spanned(Token::Plus); self.advance(); Ok(sp) }
            '*' => { let sp = self.spanned(Token::Star); self.advance(); Ok(sp) }
            '/' => { let sp = self.spanned(Token::Slash); self.advance(); Ok(sp) }
            '%' => { let sp = self.spanned(Token::Percent); self.advance(); Ok(sp) }
            '-' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && self.peek() == '>' {
                    self.advance();
                    Ok(Spanned { token: Token::Arrow, span })
                } else if !self.at_end() && self.peek().is_ascii_digit() {
                    let num_tok = self.read_number(true)?;
                    Ok(Spanned { token: num_tok, span })
                } else {
                    Ok(Spanned { token: Token::Minus, span })
                }
            }
            '=' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    Ok(Spanned { token: Token::Eq, span })
                } else if !self.at_end() && self.peek() == '>' {
                    self.advance();
                    Ok(Spanned { token: Token::FatArrow, span })
                } else {
                    Ok(Spanned { token: Token::Assign, span })
                }
            }
            '!' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    Ok(Spanned { token: Token::NotEq, span })
                } else {
                    Err(format!("Unexpected '!' at {}:{}, did you mean '!='?", span.line, span.col))
                }
            }
            '<' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    Ok(Spanned { token: Token::LtEq, span })
                } else {
                    Ok(Spanned { token: Token::Lt, span })
                }
            }
            '>' => {
                let span = self.current_span();
                self.advance();
                if !self.at_end() && (self.peek().is_alphanumeric() || self.peek() == '_') {
                    let name = self.read_ident_str();
                    Ok(Spanned { token: Token::PushTo(name), span })
                } else if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    Ok(Spanned { token: Token::GtEq, span })
                } else {
                    Ok(Spanned { token: Token::Gt, span })
                }
            }
            '|' => { let sp = self.spanned(Token::Pipe); self.advance(); Ok(sp) }
            c if c.is_ascii_digit() => {
                let span = self.current_span();
                let tok = self.read_number(false)?;
                Ok(Spanned { token: tok, span })
            }
            c if c.is_alphabetic() || c == '_' => {
                let span = self.current_span();
                let ident = self.read_ident_str();
                if !self.at_end() && self.peek() == '>' {
                    self.advance();
                    return Ok(Spanned { token: Token::PopFrom(ident), span });
                }
                let tok = match ident.as_str() {
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    "where" => Token::Where,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "let" => Token::Let,
                    _ => Token::Ident(ident),
                };
                Ok(Spanned { token: tok, span })
            }
            c => {
                Err(format!("Unexpected character '{}' at {}:{}", c, self.line, self.col))
            }
        }
    }

    fn read_string(&mut self) -> Result<Spanned, String> {
        let span = self.current_span();
        self.advance(); // skip opening "
        let mut parts: Vec<StringPart> = Vec::new();
        let mut current_lit = String::new();
        let mut has_interp = false;

        loop {
            if self.at_end() {
                return Err(format!("Unterminated string starting at {}:{}", span.line, span.col));
            }
            let ch = self.peek();
            if ch == '"' {
                self.advance();
                break;
            }
            if ch == '\\' {
                self.advance();
                if self.at_end() {
                    return Err("Unterminated escape in string".to_string());
                }
                match self.peek() {
                    'n' => current_lit.push('\n'),
                    't' => current_lit.push('\t'),
                    '\\' => current_lit.push('\\'),
                    '"' => current_lit.push('"'),
                    '{' => current_lit.push('{'),
                    c => current_lit.push(c),
                }
                self.advance();
            } else if ch == '{' {
                has_interp = true;
                self.advance();
                if !current_lit.is_empty() {
                    parts.push(StringPart::Lit(std::mem::take(&mut current_lit)));
                }
                let mut var_name = String::new();
                while !self.at_end() && self.peek() != '}' {
                    var_name.push(self.peek());
                    self.advance();
                }
                if self.at_end() {
                    return Err("Unterminated interpolation in string".to_string());
                }
                self.advance(); // skip '}'
                let var_name = var_name.trim().to_string();
                if var_name.is_empty() {
                    return Err("Empty interpolation '{}' in string".to_string());
                }
                parts.push(StringPart::Var(var_name));
            } else {
                current_lit.push(ch);
                self.advance();
            }
        }

        if has_interp {
            if !current_lit.is_empty() {
                parts.push(StringPart::Lit(current_lit));
            }
            Ok(Spanned { token: Token::StringInterp(parts), span })
        } else {
            Ok(Spanned { token: Token::Str(current_lit), span })
        }
    }

    fn read_number(&mut self, negative: bool) -> Result<Token, String> {
        let mut num_str = String::new();
        if negative {
            num_str.push('-');
        }
        let mut is_float = false;
        while !self.at_end() && (self.peek().is_ascii_digit() || self.peek() == '.') {
            if self.peek() == '.' {
                if is_float {
                    break;
                }
                is_float = true;
            }
            num_str.push(self.peek());
            self.advance();
        }
        if is_float {
            num_str.parse::<f64>()
                .map(Token::Float)
                .map_err(|e| format!("Invalid float '{}': {}", num_str, e))
        } else {
            num_str.parse::<i64>()
                .map(Token::Int)
                .map_err(|e| format!("Invalid int '{}': {}", num_str, e))
        }
    }

    fn read_ident_str(&mut self) -> String {
        let mut s = String::new();
        while !self.at_end() && (self.peek().is_alphanumeric() || self.peek() == '_' || self.peek() == '.') {
            s.push(self.peek());
            self.advance();
        }
        // Allow ? or ! as trailing character (Ruby/Elixir convention)
        if !self.at_end() && (self.peek() == '?' || self.peek() == '!') {
            s.push(self.peek());
            self.advance();
        }
        s
    }

    fn skip_whitespace(&mut self) {
        while !self.at_end() && self.peek() != '\n' && self.peek().is_whitespace() {
            self.advance();
        }
    }

    fn skip_comment(&mut self) {
        if !self.at_end() && self.peek() == '-' {
            if self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '-' {
                while !self.at_end() && self.peek() != '\n' {
                    self.advance();
                }
            }
        }
    }

    fn peek(&self) -> char {
        self.input[self.pos]
    }

    fn advance(&mut self) {
        if self.peek() == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.pos += 1;
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn current_span(&self) -> Span {
        Span { line: self.line, col: self.col }
    }

    fn spanned(&self, token: Token) -> Spanned {
        Spanned { token, span: self.current_span() }
    }
}

/// Check if input looks incomplete (unclosed delimiters)
pub fn is_incomplete(input: &str) -> bool {
    let mut colon_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let mut paren_depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            ':' => {
                // Only count as word-def colon if followed by space
                // This is approximate but good enough for REPL
                colon_depth += 1;
            }
            ';' => colon_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            _ => {}
        }
    }

    in_string || bracket_depth > 0 || brace_depth > 0 || paren_depth > 0 || colon_depth > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lex = Lexer::new("42 3.14 \"hello\" :ok");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::Int(42)));
        assert!(matches!(tokens[1].token, Token::Float(f) if (f - 3.14).abs() < 0.001));
        assert!(matches!(&tokens[2].token, Token::Str(s) if s == "hello"));
        assert!(matches!(&tokens[3].token, Token::Symbol(s) if s == "ok"));
    }

    #[test]
    fn test_word_definition() {
        let mut lex = Lexer::new(": foo [n] -> n 1 + ;");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::Colon));
        assert!(matches!(&tokens[1].token, Token::Ident(s) if s == "foo"));
        assert!(matches!(tokens[2].token, Token::LBracket));
    }

    #[test]
    fn test_named_stacks() {
        let mut lex = Lexer::new("42 >aux aux>");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::Int(42)));
        assert!(matches!(&tokens[1].token, Token::PushTo(s) if s == "aux"));
        assert!(matches!(&tokens[2].token, Token::PopFrom(s) if s == "aux"));
    }

    #[test]
    fn test_string_interpolation() {
        let mut lex = Lexer::new("\"hello {name}, age {age}\"");
        let tokens = lex.tokenize().unwrap();
        match &tokens[0].token {
            Token::StringInterp(parts) => {
                assert_eq!(parts.len(), 4);
                assert_eq!(parts[0], StringPart::Lit("hello ".to_string()));
                assert_eq!(parts[1], StringPart::Var("name".to_string()));
                assert_eq!(parts[2], StringPart::Lit(", age ".to_string()));
                assert_eq!(parts[3], StringPart::Var("age".to_string()));
            }
            _ => panic!("Expected StringInterp"),
        }
    }

    #[test]
    fn test_map_tokens() {
        let mut lex = Lexer::new("#{\"a\" => 1}");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::HashBrace));
        assert!(matches!(&tokens[1].token, Token::Str(s) if s == "a"));
        assert!(matches!(tokens[2].token, Token::FatArrow));
        assert!(matches!(tokens[3].token, Token::Int(1)));
        assert!(matches!(tokens[4].token, Token::RBrace));
    }

    #[test]
    fn test_let_token() {
        let mut lex = Lexer::new("let x = 42");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::Let));
        assert!(matches!(&tokens[1].token, Token::Ident(s) if s == "x"));
        assert!(matches!(tokens[2].token, Token::Assign));
        assert!(matches!(tokens[3].token, Token::Int(42)));
    }

    #[test]
    fn test_predicate_identifiers() {
        let mut lex = Lexer::new(": even? [n] -> n 2 % 0 == ;");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(tokens[0].token, Token::Colon));
        assert!(matches!(&tokens[1].token, Token::Ident(s) if s == "even?"));

        let mut lex = Lexer::new("5 even? :ok!");
        let tokens = lex.tokenize().unwrap();
        assert!(matches!(&tokens[1].token, Token::Ident(s) if s == "even?"));
        assert!(matches!(&tokens[2].token, Token::Symbol(s) if s == "ok!"));
    }

    #[test]
    fn test_incomplete() {
        assert!(is_incomplete(": foo"));
        assert!(is_incomplete("[1 2"));
        assert!(is_incomplete("{1, 2"));
        assert!(!is_incomplete(": foo [n] -> n ;"));
        assert!(!is_incomplete("42 3 +"));
    }
}
