use crate::ast::*;
use crate::token::{Token, Spanned, StringPart};

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<SpannedProgram, String> {
        let mut items = Vec::new();
        self.skip_newlines();
        while !self.at_end() {
            let line = self.current_line();
            let item = self.parse_item()?;
            items.push(SpannedItem { item, line });
            self.skip_newlines();
        }
        Ok(items)
    }

    fn parse_item(&mut self) -> Result<Item, String> {
        if self.check(&Token::Colon) {
            self.parse_word_def().map(Item::WordDef)
        } else if self.check(&Token::Let) {
            self.parse_let().map(Item::Let)
        } else {
            self.parse_expr().map(Item::Expr)
        }
    }

    fn parse_let(&mut self) -> Result<LetBinding, String> {
        self.expect(&Token::Let)?;

        let target = if self.check(&Token::LBrace) {
            // let {a, b, c} = ... or let {h | t} = ...
            self.advance(); // consume {
            let mut names = Vec::new();
            let first = self.expect_ident()?;
            if self.check(&Token::Pipe) {
                // let {h | t} = ...
                self.advance(); // consume |
                let tail = self.expect_ident()?;
                self.expect(&Token::RBrace)?;
                LetTarget::ListCons { head: first, tail }
            } else {
                names.push(first);
                while self.check(&Token::Comma) {
                    self.advance();
                    names.push(self.expect_ident()?);
                }
                self.expect(&Token::RBrace)?;
                LetTarget::List(names)
            }
        } else if self.check(&Token::HashBrace) {
            // let #{"key" => name, ...} = ...
            self.advance(); // consume #{
            let mut pairs = Vec::new();
            if !self.check(&Token::RBrace) {
                loop {
                    let key = match &self.current().token {
                        Token::Str(s) => s.clone(),
                        other => return Err(format!("Expected string key in let destructure, got {:?}", other)),
                    };
                    self.advance();
                    self.expect(&Token::FatArrow)?;
                    let bind_name = self.expect_ident()?;
                    pairs.push((key, bind_name));
                    if !self.check(&Token::Comma) { break; }
                    self.advance();
                }
            }
            self.expect(&Token::RBrace)?;
            LetTarget::Map(pairs)
        } else {
            // let name = ...
            let name = self.expect_ident()?;
            LetTarget::Simple(name)
        };

        self.expect(&Token::Assign)?;
        let mut body = Vec::new();
        while !self.at_end()
            && !matches!(self.current().token, Token::Newline | Token::Semicolon | Token::Eof)
        {
            body.push(self.parse_expr()?);
        }
        // Consume optional semicolon
        if self.check(&Token::Semicolon) {
            self.advance();
        }
        Ok(LetBinding { target, body })
    }

    fn parse_word_def(&mut self) -> Result<WordDef, String> {
        self.expect(&Token::Colon)?;
        let name = self.expect_ident()?;
        self.skip_newlines();

        let mut arms = Vec::new();
        while !self.check(&Token::Semicolon) && !self.at_end() {
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            self.skip_newlines();
        }
        self.expect(&Token::Semicolon)?;

        if arms.is_empty() {
            return Err(format!("Word '{}' has no match arms", name));
        }

        Ok(WordDef { name, arms })
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, String> {
        let line = self.current_line();
        let patterns = self.parse_patterns()?;

        let guard = if self.check(&Token::Where) {
            self.advance();
            Some(self.parse_guard()?)
        } else {
            None
        };

        self.expect(&Token::Arrow)?;

        let body = self.parse_spanned_body()?;

        Ok(MatchArm { patterns, guard, body, line })
    }

    fn parse_patterns(&mut self) -> Result<Vec<Pattern>, String> {
        let mut patterns = Vec::new();

        if self.check(&Token::LBrace) {
            self.advance();
            let pat = self.parse_list_pattern()?;
            patterns.push(pat);
            self.expect(&Token::RBrace)?;
        } else if self.check(&Token::LBracket) {
            self.advance();
            if !self.check(&Token::RBracket) {
                patterns.push(self.parse_single_pattern()?);
                while self.check(&Token::Comma) {
                    self.advance();
                    patterns.push(self.parse_single_pattern()?);
                }
            }
            self.expect(&Token::RBracket)?;
        } else {
            return Err(self.error("Expected '[' or '{' to start pattern"));
        }

        Ok(patterns)
    }

    fn parse_list_pattern(&mut self) -> Result<Pattern, String> {
        self.expect(&Token::LBracket)?;
        if self.check(&Token::RBracket) {
            self.advance();
            return Ok(Pattern::ListEmpty);
        }
        let head = self.expect_ident()?;
        self.expect(&Token::Pipe)?;
        let tail = self.expect_ident()?;
        self.expect(&Token::RBracket)?;
        Ok(Pattern::ListCons { head, tail })
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, String> {
        match &self.current().token {
            Token::Int(n) => { let n = *n; self.advance(); Ok(Pattern::Literal(Literal::Int(n))) }
            Token::Float(f) => { let f = *f; self.advance(); Ok(Pattern::Literal(Literal::Float(f))) }
            Token::Str(s) => { let s = s.clone(); self.advance(); Ok(Pattern::Literal(Literal::Str(s))) }
            Token::Symbol(s) => { let s = s.clone(); self.advance(); Ok(Pattern::Literal(Literal::Symbol(s))) }
            Token::Bool(b) => { let b = *b; self.advance(); Ok(Pattern::Literal(Literal::Bool(b))) }
            Token::Ident(s) if s == "_" => { self.advance(); Ok(Pattern::Wildcard) }
            Token::Ident(s) => { let s = s.clone(); self.advance(); Ok(Pattern::Bind(s)) }
            _ => Err(self.error("Expected pattern element")),
        }
    }

    fn parse_guard(&mut self) -> Result<Guard, String> {
        let left = self.parse_guard_atom()?;
        if self.check(&Token::And) {
            self.advance();
            let right = self.parse_guard()?;
            Ok(Guard::And(Box::new(left), Box::new(right)))
        } else if self.check(&Token::Or) {
            self.advance();
            let right = self.parse_guard()?;
            Ok(Guard::Or(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
    }

    fn parse_guard_atom(&mut self) -> Result<Guard, String> {
        if self.check(&Token::Not) {
            self.advance();
            let inner = self.parse_guard_atom()?;
            return Ok(Guard::Not(Box::new(inner)));
        }

        let left = self.parse_guard_expr()?;
        let op = match &self.current().token {
            Token::Eq => CmpOp::Eq,
            Token::NotEq => CmpOp::NotEq,
            Token::Lt => CmpOp::Lt,
            Token::Gt => CmpOp::Gt,
            Token::LtEq => CmpOp::LtEq,
            Token::GtEq => CmpOp::GtEq,
            _ => return Err(self.error("Expected comparison operator in guard")),
        };
        self.advance();
        let right = self.parse_guard_expr()?;
        Ok(Guard::Compare { left, op, right })
    }

    fn parse_guard_expr(&mut self) -> Result<Expr, String> {
        match &self.current().token {
            Token::Int(n) => { let n = *n; self.advance(); Ok(Expr::IntLit(n)) }
            Token::Float(f) => { let f = *f; self.advance(); Ok(Expr::FloatLit(f)) }
            Token::Str(s) => { let s = s.clone(); self.advance(); Ok(Expr::StrLit(s)) }
            Token::Bool(b) => { let b = *b; self.advance(); Ok(Expr::BoolLit(b)) }
            Token::Ident(s) => { let s = s.clone(); self.advance(); Ok(Expr::Word(s)) }
            _ => Err(self.error("Expected expression in guard")),
        }
    }

    fn parse_spanned_body(&mut self) -> Result<Vec<SpannedExpr>, String> {
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_end()
                || self.check(&Token::Semicolon)
                || self.is_arm_start()
            {
                break;
            }
            let line = self.current_line();
            let expr = self.parse_expr()?;
            body.push(SpannedExpr { expr, line: Some(line) });
        }
        Ok(body)
    }

    fn is_arm_start(&self) -> bool {
        matches!(self.current().token, Token::LBracket | Token::LBrace)
            && self.is_pattern_context()
    }

    fn is_pattern_context(&self) -> bool {
        let mut depth = 0;
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].token {
                Token::LBracket | Token::LBrace => depth += 1,
                Token::RBracket | Token::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        let mut j = i + 1;
                        while j < self.tokens.len() && matches!(self.tokens[j].token, Token::Newline) {
                            j += 1;
                        }
                        if j < self.tokens.len() {
                            return matches!(self.tokens[j].token, Token::Arrow | Token::Where);
                        }
                        return false;
                    }
                }
                Token::Semicolon | Token::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        match &self.current().token {
            Token::Int(n) => { let n = *n; self.advance(); Ok(Expr::IntLit(n)) }
            Token::Float(f) => { let f = *f; self.advance(); Ok(Expr::FloatLit(f)) }
            Token::Str(s) => { let s = s.clone(); self.advance(); Ok(Expr::StrLit(s)) }
            Token::Symbol(s) => { let s = s.clone(); self.advance(); Ok(Expr::SymbolLit(s)) }
            Token::Bool(b) => { let b = *b; self.advance(); Ok(Expr::BoolLit(b)) }
            Token::StringInterp(parts) => {
                let ast_parts: Vec<StringInterpPart> = parts.iter().map(|p| match p {
                    StringPart::Lit(s) => StringInterpPart::Lit(s.clone()),
                    StringPart::Var(s) => StringInterpPart::Var(s.clone()),
                }).collect();
                self.advance();
                Ok(Expr::StringInterp(ast_parts))
            }
            Token::Ident(s) => {
                let s = s.clone();
                self.advance();
                if s == "apply" {
                    Ok(Expr::Apply)
                } else {
                    Ok(Expr::Word(s))
                }
            }
            Token::Plus => { self.advance(); Ok(Expr::Add) }
            Token::Minus => { self.advance(); Ok(Expr::Sub) }
            Token::Star => { self.advance(); Ok(Expr::Mul) }
            Token::Slash => { self.advance(); Ok(Expr::Div) }
            Token::Percent => { self.advance(); Ok(Expr::Mod) }
            Token::Eq => { self.advance(); Ok(Expr::Eq) }
            Token::NotEq => { self.advance(); Ok(Expr::NotEq) }
            Token::Lt => { self.advance(); Ok(Expr::Lt) }
            Token::Gt => { self.advance(); Ok(Expr::Gt) }
            Token::LtEq => { self.advance(); Ok(Expr::LtEq) }
            Token::GtEq => { self.advance(); Ok(Expr::GtEq) }
            Token::And => { self.advance(); Ok(Expr::And) }
            Token::Or => { self.advance(); Ok(Expr::Or) }
            Token::Not => { self.advance(); Ok(Expr::Not) }
            Token::Dot => { self.advance(); Ok(Expr::Compose) }
            Token::Tilde => { self.advance(); Ok(Expr::Apply) }
            Token::PushTo(name) => { let n = name.clone(); self.advance(); Ok(Expr::PushTo(n)) }
            Token::PopFrom(name) => { let n = name.clone(); self.advance(); Ok(Expr::PopFrom(n)) }
            Token::LBracket => self.parse_quotation(),
            Token::LBrace => self.parse_list(),
            Token::HashBrace => self.parse_map(),
            _ => Err(self.error(&format!("Unexpected token: {:?}", self.current().token))),
        }
    }

    fn parse_quotation(&mut self) -> Result<Expr, String> {
        self.expect(&Token::LBracket)?;
        let mut body = Vec::new();
        while !self.check(&Token::RBracket) && !self.at_end() {
            self.skip_newlines();
            if self.check(&Token::RBracket) { break; }
            body.push(self.parse_expr()?);
        }
        self.expect(&Token::RBracket)?;
        Ok(Expr::Quotation(body))
    }

    fn parse_list(&mut self) -> Result<Expr, String> {
        self.expect(&Token::LBrace)?;
        let mut elements = Vec::new();
        if !self.check(&Token::RBrace) {
            elements.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                if self.check(&Token::RBrace) { break; }
                elements.push(self.parse_expr()?);
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::List(elements))
    }

    fn parse_map(&mut self) -> Result<Expr, String> {
        self.expect(&Token::HashBrace)?;
        let mut pairs = Vec::new();
        self.skip_newlines();
        if !self.check(&Token::RBrace) {
            let key = self.parse_expr()?;
            self.skip_newlines();
            self.expect(&Token::FatArrow)?;
            self.skip_newlines();
            let val = self.parse_expr()?;
            pairs.push((key, val));
            self.skip_newlines();
            while self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
                if self.check(&Token::RBrace) { break; }
                let key = self.parse_expr()?;
                self.skip_newlines();
                self.expect(&Token::FatArrow)?;
                self.skip_newlines();
                let val = self.parse_expr()?;
                pairs.push((key, val));
                self.skip_newlines();
            }
        }
        self.skip_newlines();
        self.expect(&Token::RBrace)?;
        Ok(Expr::Map(pairs))
    }

    // --- Helpers ---

    fn current(&self) -> &Spanned {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn current_line(&self) -> usize {
        self.current().span.line
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.current().token, Token::Eof)
    }

    fn check(&self, expected: &Token) -> bool {
        !self.at_end() && std::mem::discriminant(&self.current().token) == std::mem::discriminant(expected)
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        if self.check(expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(&format!("Expected {:?}, got {:?}", expected, self.current().token)))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        if let Token::Ident(s) = &self.current().token {
            let s = s.clone();
            self.advance();
            Ok(s)
        } else {
            Err(self.error("Expected identifier"))
        }
    }

    fn skip_newlines(&mut self) {
        while !self.at_end() && matches!(self.current().token, Token::Newline) {
            self.advance();
        }
    }

    fn error(&self, msg: &str) -> String {
        let span = &self.current().span;
        format!("[{}:{}] {}", span.line, span.col, msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> SpannedProgram {
        let mut lex = Lexer::new(input);
        let tokens = lex.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_simple_exprs() {
        let prog = parse("42 3.14 +");
        assert_eq!(prog.len(), 3);
    }

    #[test]
    fn test_word_def() {
        let prog = parse(": double [n] -> n n + ;");
        assert_eq!(prog.len(), 1);
        match &prog[0].item {
            Item::WordDef(w) => {
                assert_eq!(w.name, "double");
                assert_eq!(w.arms.len(), 1);
            }
            _ => panic!("Expected word def"),
        }
    }

    #[test]
    fn test_multi_arm() {
        let prog = parse(": fact [0] -> 1 [n] -> n n 1 - fact * ;");
        match &prog[0].item {
            Item::WordDef(w) => {
                assert_eq!(w.name, "fact");
                assert_eq!(w.arms.len(), 2);
            }
            _ => panic!("Expected word def"),
        }
    }

    #[test]
    fn test_let_binding() {
        let prog = parse("let x = 42");
        assert_eq!(prog.len(), 1);
        match &prog[0].item {
            Item::Let(l) => {
                assert!(matches!(&l.target, LetTarget::Simple(n) if n == "x"));
                assert_eq!(l.body.len(), 1);
            }
            _ => panic!("Expected let"),
        }
    }

    #[test]
    fn test_map_literal() {
        let prog = parse("#{\"a\" => 1, \"b\" => 2}");
        assert_eq!(prog.len(), 1);
        match &prog[0].item {
            Item::Expr(Expr::Map(pairs)) => {
                assert_eq!(pairs.len(), 2);
            }
            _ => panic!("Expected map"),
        }
    }
}
