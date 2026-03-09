/// Bytecode instructions for the Sabot VM
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Op {
    // Push constants
    PushInt(i64),
    PushFloat(f64),
    PushStr(String),
    PushSymbol(String),
    PushBool(bool),
    PushList(usize), // create list from top N stack items
    PushMap(usize),  // create map from top N*2 stack items (key, val pairs)

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    // Logic
    And,
    Or,
    Not,

    // Stack manipulation
    Dup,
    Drop,
    Swap,
    Over,
    Rot,

    // Named stack ops
    PushTo(String),
    PopFrom(String),

    // Globals (let bindings)
    StoreGlobal(String),
    LoadGlobal(String),

    // Pattern matching
    MatchBegin(usize),
    MatchLitInt(i64, usize),
    MatchLitFloat(u64, usize),
    MatchLitStr(String, usize),
    MatchLitSymbol(String, usize),
    MatchLitBool(bool, usize),
    MatchBind(usize),
    MatchListEmpty(usize),
    MatchListCons(usize, usize, usize),
    MatchWildcard,
    MatchPop(usize),

    // Guards
    GuardFail(usize),

    // Variables (pattern bindings)
    LoadLocal(usize),

    // Control flow
    Call(String),
    TailCall(String),
    Jump(usize),
    JumpIf(usize),
    JumpIfNot(usize),
    Return,

    // Quotations
    PushQuotation(Vec<Op>),
    Apply,
    Compose,

    // I/O
    Print,
    PrintLn,

    // String
    StrConcat, // pop two strings, push concatenated

    // Special
    Halt,
    Nop,
    Label(usize),
}
