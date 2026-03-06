#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Symbol(String), // :name
    Bool(bool),

    // String interpolation: "hello {name}, you are {age}"
    // Produces: StringInterp(vec![Lit("hello "), Var("name"), Lit(", you are "), Var("age")])
    StringInterp(Vec<StringPart>),

    // Identifiers & keywords
    Ident(String),
    Colon,        // : (word definition start)
    Semicolon,    // ; (word definition end)
    Arrow,        // ->
    Where,        // where
    Pipe,         // |
    Let,          // let
    Assign,       // =

    // Brackets
    LBracket,     // [ (quotation / pattern start)
    RBracket,     // ] (quotation / pattern end)
    LBrace,       // { (list start)
    RBrace,       // } (list end)
    LParen,       // (
    RParen,       // )

    // Map
    HashBrace,    // #{ (map start)
    FatArrow,     // =>

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,      // mod
    Dot,          // . (compose)
    Tilde,        // ~ (apply)
    Eq,           // ==
    NotEq,        // !=
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,          // and
    Or,           // or
    Not,          // not

    // Stack ops
    PushTo(String),   // >name (push to named stack)
    PopFrom(String),  // name> (pop from named stack)

    // Special
    Comma,
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Lit(String),
    Var(String),
}

#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}
