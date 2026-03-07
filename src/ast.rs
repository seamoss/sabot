/// A pattern that matches against the stack
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Match a literal value: [42], ["hello"], [:ok]
    Literal(Literal),
    /// Bind top of stack to a name: [n]
    Bind(String),
    /// Wildcard, matches anything: [_]
    Wildcard,
    /// List destructure: {[head | tail]} or {[]}
    ListEmpty,
    ListCons {
        head: String,
        tail: String,
    },
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(String),
    Symbol(String),
    Bool(bool),
}

/// A guard condition on a pattern match arm
#[derive(Debug, Clone)]
pub enum Guard {
    Compare { left: Expr, op: CmpOp, right: Expr },
    And(Box<Guard>, Box<Guard>),
    Or(Box<Guard>, Box<Guard>),
    Not(Box<Guard>),
}

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

/// A single arm: pattern(s) [where guard] -> body
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub patterns: Vec<Pattern>,
    pub guard: Option<Guard>,
    pub body: Vec<SpannedExpr>,
    pub line: usize,
}

/// Expression with optional source line
#[derive(Debug, Clone)]
pub struct SpannedExpr {
    pub expr: Expr,
    pub line: Option<usize>,
}

/// Top-level expression / instruction
#[derive(Debug, Clone)]
pub enum Expr {
    // Literals (push to stack)
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    SymbolLit(String),
    BoolLit(bool),

    // String interpolation: "hello {name}"
    StringInterp(Vec<StringInterpPart>),

    // Word reference (call or push)
    Word(String),

    // Built-in operators
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    Not,

    // Quotation: [expr...]
    Quotation(Vec<Expr>),

    // List literal: {expr, expr, ...}
    List(Vec<Expr>),

    // Map literal: #{"key" => val, ...}
    Map(Vec<(Expr, Expr)>),

    // Named stack ops
    PushTo(String),
    PopFrom(String),

    // Compose operator
    Compose,

    // Apply (execute quotation on top of stack)
    Apply,
}

#[derive(Debug, Clone)]
pub enum StringInterpPart {
    Lit(String),
    Var(String),
}

/// A word definition with pattern-matched arms
#[derive(Debug, Clone)]
pub struct WordDef {
    pub name: String,
    pub arms: Vec<MatchArm>,
}

/// Let binding target: simple name or destructuring pattern
#[derive(Debug, Clone)]
pub enum LetTarget {
    /// let name = ...
    Simple(String),
    /// let {a, b, c} = ... (list positional)
    List(Vec<String>),
    /// let {h | t} = ... (head/tail)
    ListCons { head: String, tail: String },
    /// let #{"key" => name, ...} = ... (map keys)
    Map(Vec<(String, String)>), // (key, binding_name)
}

/// Let binding: let <target> = <exprs until newline/semicolon>
#[derive(Debug, Clone)]
pub struct LetBinding {
    pub target: LetTarget,
    pub body: Vec<Expr>,
}

/// Top-level program items
#[derive(Debug, Clone)]
pub enum Item {
    WordDef(WordDef),
    Let(LetBinding),
    Expr(Expr),
}

/// Item with source line info
#[derive(Debug, Clone)]
pub struct SpannedItem {
    pub item: Item,
    pub line: usize,
}

pub type SpannedProgram = Vec<SpannedItem>;
