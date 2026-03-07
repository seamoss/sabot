use crate::ast::*;
use crate::opcode::Op;
use std::collections::HashMap;

/// Code with parallel line-number table
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub ops: Vec<Op>,
    pub lines: Vec<usize>,
}

impl CodeBlock {
    pub fn new() -> Self {
        CodeBlock {
            ops: Vec::new(),
            lines: Vec::new(),
        }
    }
    pub fn push(&mut self, op: Op, line: usize) {
        self.ops.push(op);
        self.lines.push(line);
    }
}

pub struct Compiler {
    label_counter: usize,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler { label_counter: 0 }
    }

    fn next_label(&mut self) -> usize {
        let l = self.label_counter;
        self.label_counter += 1;
        l
    }

    pub fn compile(&mut self, program: &SpannedProgram) -> Result<CompiledProgram, String> {
        let mut words: HashMap<String, CodeBlock> = HashMap::new();
        let mut main = CodeBlock::new();

        for sitem in program {
            let line = sitem.line;
            match &sitem.item {
                Item::WordDef(def) => {
                    let code = self.compile_word(def)?;
                    words.insert(def.name.clone(), code);
                }
                Item::Let(binding) => {
                    // Evaluate the body expression(s)
                    for expr in &binding.body {
                        let _before = main.ops.len();
                        self.compile_expr(expr, &mut main.ops)?;
                        while main.lines.len() < main.ops.len() {
                            main.lines.push(line);
                        }
                    }
                    // Destructure based on target type
                    match &binding.target {
                        LetTarget::Simple(name) => {
                            main.push(Op::StoreGlobal(name.clone()), line);
                        }
                        LetTarget::List(names) => {
                            // Stack has the list. For each name, dup + nth + store.
                            for (i, name) in names.iter().enumerate() {
                                main.push(Op::Dup, line);
                                main.push(Op::PushInt(i as i64), line);
                                main.push(Op::Call("nth".to_string()), line);
                                main.push(Op::StoreGlobal(name.clone()), line);
                            }
                            main.push(Op::Drop, line); // drop the original list
                        }
                        LetTarget::ListCons { head, tail } => {
                            // Stack has the list. Dup, head, store. Then tail, store.
                            main.push(Op::Dup, line);
                            main.push(Op::Call("head".to_string()), line);
                            main.push(Op::StoreGlobal(head.clone()), line);
                            main.push(Op::Call("tail".to_string()), line);
                            main.push(Op::StoreGlobal(tail.clone()), line);
                        }
                        LetTarget::Map(pairs) => {
                            // Stack has the map. For each pair, dup + push key + get + store.
                            for (key, name) in pairs {
                                main.push(Op::Dup, line);
                                main.push(Op::PushStr(key.clone()), line);
                                main.push(Op::Call("get".to_string()), line);
                                main.push(Op::StoreGlobal(name.clone()), line);
                            }
                            main.push(Op::Drop, line); // drop the original map
                        }
                    }
                }
                Item::Expr(expr) => {
                    let _before = main.ops.len();
                    self.compile_expr(expr, &mut main.ops)?;
                    while main.lines.len() < main.ops.len() {
                        main.lines.push(line);
                    }
                }
            }
        }

        main.ops.push(Op::Halt);
        main.lines.push(0);
        Ok(CompiledProgram { words, main })
    }

    fn compile_word(&mut self, def: &WordDef) -> Result<CodeBlock, String> {
        let mut code = CodeBlock::new();
        let end_label = self.next_label();

        for arm in &def.arms {
            let next_arm_label = self.next_label();
            let mut locals = LocalScope::new();
            let line = arm.line;

            let n_patterns = arm.patterns.len();
            code.ops.push(Op::MatchBegin(n_patterns));
            code.lines.push(line);

            for (i, pat) in arm.patterns.iter().enumerate() {
                self.compile_pattern(pat, &mut locals, next_arm_label, i, &mut code.ops)?;
                code.lines.push(line);
            }

            if let Some(guard) = &arm.guard {
                let _before = code.ops.len();
                self.compile_guard(guard, &locals, &mut code.ops)?;
                while code.lines.len() < code.ops.len() {
                    code.lines.push(line);
                }
                code.ops.push(Op::GuardFail(next_arm_label));
                code.lines.push(line);
            }

            code.ops.push(Op::MatchPop(n_patterns));
            code.lines.push(line);

            for expr in &arm.body {
                let expr_line = expr.line.unwrap_or(line);
                let _before = code.ops.len();
                self.compile_body_expr(&expr.expr, &locals, &mut code.ops)?;
                while code.lines.len() < code.ops.len() {
                    code.lines.push(expr_line);
                }
            }

            // Tail-call optimization: if the last op is Call to self, replace with TailCall
            if let Some(Op::Call(name)) = code.ops.last()
                && name == &def.name
            {
                let name = name.clone();
                code.ops.pop();
                code.lines.pop();
                code.ops.push(Op::TailCall(name));
                code.lines.push(line);
            }

            code.ops.push(Op::Jump(end_label));
            code.lines.push(line);
            code.ops.push(Op::Label(next_arm_label));
            code.lines.push(line);
        }

        let line = def.arms.last().map(|a| a.line).unwrap_or(0);
        code.ops.push(Op::PushStr(format!(
            "No matching pattern in '{}'",
            def.name
        )));
        code.lines.push(line);
        code.ops.push(Op::Halt);
        code.lines.push(line);

        code.ops.push(Op::Label(end_label));
        code.lines.push(line);
        code.ops.push(Op::Return);
        code.lines.push(line);
        Ok(code)
    }

    fn compile_pattern(
        &mut self,
        pat: &Pattern,
        locals: &mut LocalScope,
        fail_label: usize,
        _stack_offset: usize,
        code: &mut Vec<Op>,
    ) -> Result<(), String> {
        match pat {
            Pattern::Literal(lit) => match lit {
                Literal::Int(n) => code.push(Op::MatchLitInt(*n, fail_label)),
                Literal::Float(f) => code.push(Op::MatchLitFloat(f.to_bits(), fail_label)),
                Literal::Str(s) => code.push(Op::MatchLitStr(s.clone(), fail_label)),
                Literal::Symbol(s) => code.push(Op::MatchLitSymbol(s.clone(), fail_label)),
                Literal::Bool(b) => code.push(Op::MatchLitBool(*b, fail_label)),
            },
            Pattern::Bind(name) => {
                let slot = locals.add(name.clone());
                code.push(Op::MatchBind(slot));
            }
            Pattern::Wildcard => {
                code.push(Op::MatchWildcard);
            }
            Pattern::ListEmpty => {
                code.push(Op::MatchListEmpty(fail_label));
            }
            Pattern::ListCons { head, tail } => {
                let head_slot = locals.add(head.clone());
                let tail_slot = locals.add(tail.clone());
                code.push(Op::MatchListCons(head_slot, tail_slot, fail_label));
            }
        }
        Ok(())
    }

    fn compile_guard(
        &mut self,
        guard: &Guard,
        locals: &LocalScope,
        code: &mut Vec<Op>,
    ) -> Result<(), String> {
        match guard {
            Guard::Compare { left, op, right } => {
                self.compile_body_expr(left, locals, code)?;
                self.compile_body_expr(right, locals, code)?;
                let cmp_op = match op {
                    CmpOp::Eq => Op::Eq,
                    CmpOp::NotEq => Op::NotEq,
                    CmpOp::Lt => Op::Lt,
                    CmpOp::Gt => Op::Gt,
                    CmpOp::LtEq => Op::LtEq,
                    CmpOp::GtEq => Op::GtEq,
                };
                code.push(cmp_op);
            }
            Guard::And(l, r) => {
                self.compile_guard(l, locals, code)?;
                self.compile_guard(r, locals, code)?;
                code.push(Op::And);
            }
            Guard::Or(l, r) => {
                self.compile_guard(l, locals, code)?;
                self.compile_guard(r, locals, code)?;
                code.push(Op::Or);
            }
            Guard::Not(inner) => {
                self.compile_guard(inner, locals, code)?;
                code.push(Op::Not);
            }
        }
        Ok(())
    }

    fn compile_body_expr(
        &self,
        expr: &Expr,
        locals: &LocalScope,
        code: &mut Vec<Op>,
    ) -> Result<(), String> {
        match expr {
            Expr::Word(name) => {
                if let Some(slot) = locals.get(name) {
                    code.push(Op::LoadLocal(slot));
                } else {
                    // Could be a global or a word call — VM resolves at runtime
                    code.push(Op::Call(name.clone()));
                }
            }
            Expr::StringInterp(parts) => {
                self.compile_string_interp(parts, Some(locals), code)?;
            }
            Expr::Map(pairs) => {
                for (k, v) in pairs {
                    self.compile_body_expr(k, locals, code)?;
                    self.compile_body_expr(v, locals, code)?;
                }
                code.push(Op::PushMap(pairs.len()));
            }
            Expr::Quotation(body) => {
                let mut inner = Vec::new();
                for e in body {
                    self.compile_body_expr(e, locals, &mut inner)?;
                }
                code.push(Op::PushQuotation(inner));
            }
            Expr::List(elements) => {
                for e in elements {
                    self.compile_body_expr(e, locals, code)?;
                }
                code.push(Op::PushList(elements.len()));
            }
            other => self.compile_expr(other, code)?,
        }
        Ok(())
    }

    fn compile_string_interp(
        &self,
        parts: &[StringInterpPart],
        locals: Option<&LocalScope>,
        code: &mut Vec<Op>,
    ) -> Result<(), String> {
        if parts.is_empty() {
            code.push(Op::PushStr(String::new()));
            return Ok(());
        }

        let mut first = true;
        for part in parts {
            match part {
                StringInterpPart::Lit(s) => {
                    code.push(Op::PushStr(s.clone()));
                }
                StringInterpPart::Var(name) => {
                    if let Some(locals) = locals {
                        if let Some(slot) = locals.get(name) {
                            code.push(Op::LoadLocal(slot));
                        } else {
                            code.push(Op::LoadGlobal(name.clone()));
                        }
                    } else {
                        code.push(Op::LoadGlobal(name.clone()));
                    }
                    // Convert to string for concatenation
                    code.push(Op::Call("to_str".to_string()));
                }
            }
            if !first {
                code.push(Op::StrConcat);
            }
            first = false;
        }
        Ok(())
    }

    fn compile_expr(&self, expr: &Expr, code: &mut Vec<Op>) -> Result<(), String> {
        match expr {
            Expr::IntLit(n) => code.push(Op::PushInt(*n)),
            Expr::FloatLit(f) => code.push(Op::PushFloat(*f)),
            Expr::StrLit(s) => code.push(Op::PushStr(s.clone())),
            Expr::SymbolLit(s) => code.push(Op::PushSymbol(s.clone())),
            Expr::BoolLit(b) => code.push(Op::PushBool(*b)),

            Expr::StringInterp(parts) => {
                self.compile_string_interp(parts, None, code)?;
            }

            Expr::Word(name) => code.push(Op::Call(name.clone())),

            Expr::Add => code.push(Op::Add),
            Expr::Sub => code.push(Op::Sub),
            Expr::Mul => code.push(Op::Mul),
            Expr::Div => code.push(Op::Div),
            Expr::Mod => code.push(Op::Mod),
            Expr::Eq => code.push(Op::Eq),
            Expr::NotEq => code.push(Op::NotEq),
            Expr::Lt => code.push(Op::Lt),
            Expr::Gt => code.push(Op::Gt),
            Expr::LtEq => code.push(Op::LtEq),
            Expr::GtEq => code.push(Op::GtEq),
            Expr::And => code.push(Op::And),
            Expr::Or => code.push(Op::Or),
            Expr::Not => code.push(Op::Not),

            Expr::PushTo(name) => code.push(Op::PushTo(name.clone())),
            Expr::PopFrom(name) => code.push(Op::PopFrom(name.clone())),

            Expr::Compose => code.push(Op::Compose),
            Expr::Apply => code.push(Op::Apply),

            Expr::Quotation(body) => {
                let mut inner = Vec::new();
                for e in body {
                    self.compile_expr(e, &mut inner)?;
                }
                code.push(Op::PushQuotation(inner));
            }

            Expr::List(elements) => {
                for e in elements {
                    self.compile_expr(e, code)?;
                }
                code.push(Op::PushList(elements.len()));
            }

            Expr::Map(pairs) => {
                for (k, v) in pairs {
                    self.compile_expr(k, code)?;
                    self.compile_expr(v, code)?;
                }
                code.push(Op::PushMap(pairs.len()));
            }
        }
        Ok(())
    }
}

struct LocalScope {
    vars: HashMap<String, usize>,
    next_slot: usize,
}

impl LocalScope {
    fn new() -> Self {
        LocalScope {
            vars: HashMap::new(),
            next_slot: 0,
        }
    }

    fn add(&mut self, name: String) -> usize {
        let slot = self.next_slot;
        self.vars.insert(name, slot);
        self.next_slot += 1;
        slot
    }

    fn get(&self, name: &str) -> Option<usize> {
        self.vars.get(name).copied()
    }
}

pub struct CompiledProgram {
    pub words: HashMap<String, CodeBlock>,
    pub main: CodeBlock,
}
