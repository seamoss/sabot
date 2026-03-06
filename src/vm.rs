use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::opcode::Op;
use crate::value::Value;
use crate::profiler::Profiler;
use crate::builtins_otel::OtelState;

/// A typed channel with close semantics
struct ChannelInner {
    queue: VecDeque<Value>,
    closed: bool,
}

/// Task metadata for structured concurrency
struct TaskInfo {
    result: Option<Result<Value, String>>,
    cancel_token: Arc<AtomicBool>,
    children: Vec<u64>,  // child task IDs (for group cancellation)
}

/// Registry of typed channels (by integer ID)
type ChannelRegistry = Arc<Mutex<HashMap<u64, Arc<Mutex<ChannelInner>>>>>;

/// Registry of task info (by task ID)
type TaskRegistry = Arc<Mutex<HashMap<u64, TaskInfo>>>;

fn find_label(code: &[Op], label: usize) -> Result<usize, String> {
    for (i, op) in code.iter().enumerate() {
        if let Op::Label(l) = op {
            if *l == label {
                return Ok(i);
            }
        }
    }
    Err(format!("Label {} not found", label))
}

struct CallFrame {
    code: Vec<Op>,
    lines: Vec<usize>,  // parallel line-number table (same length as code, or empty)
    ip: usize,
    locals: Vec<Value>,
    match_stack: Vec<Value>,
    word_name: Option<String>,  // for backtrace
}

/// Shared channel store for cross-thread communication
pub type Channels = Arc<Mutex<HashMap<String, VecDeque<Value>>>>;

/// Reactive computed cell: name + quotation to re-evaluate
struct ComputedCell {
    name: String,
    code: Vec<Op>,
}

/// Reactive watcher: condition quotation + action quotation
struct Watcher {
    condition: Vec<Op>,
    action: Vec<Op>,
    was_true: bool,
}

pub struct VM {
    stack: Vec<Value>,
    named_stacks: HashMap<String, Vec<Value>>,
    globals: HashMap<String, Value>,
    words: HashMap<String, crate::compiler::CodeBlock>,
    frames: Vec<CallFrame>,
    builtins: HashMap<String, fn(&mut VM) -> Result<(), String>>,

    // Reactive cells
    cells: HashMap<String, Value>,
    computed_cells: Vec<ComputedCell>,
    watchers: Vec<Watcher>,

    // Concurrency
    channels: Channels,
    next_task_id: u64,
    task_results: Arc<Mutex<HashMap<u64, Option<Result<Value, String>>>>>,

    // HTTP server
    routes: crate::builtins_http::Routes,
    static_dirs: crate::builtins_http::StaticDirs,

    // Database
    db: Option<Arc<Mutex<rusqlite::Connection>>>,

    // Memoization
    memoized: HashMap<String, usize>,  // word name -> arity
    memo_cache: HashMap<String, HashMap<String, Value>>,  // word -> (key_str -> result)

    // Structured error support: throw stashes value here
    thrown_value: Option<Value>,

    // Structured concurrency
    typed_channels: ChannelRegistry,
    next_channel_id: u64,
    task_registry: TaskRegistry,
    cancel_token: Arc<AtomicBool>,  // this VM's own cancellation token
    task_group_stack: Vec<Vec<u64>>,  // stack of task groups (each is a list of task IDs)

    // Profiling (None = disabled)
    pub profiler: Option<Profiler>,

    // Observability (tracing, metrics, logging)
    pub otel: OtelState,
}

impl VM {
    pub fn new() -> Self {
        let channels = Arc::new(Mutex::new(HashMap::new()));
        let task_results = Arc::new(Mutex::new(HashMap::new()));
        let routes = Arc::new(Mutex::new(Vec::new()));
        let static_dirs = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM {
            stack: Vec::new(),
            named_stacks: HashMap::new(),
            globals: HashMap::new(),
            words: HashMap::new(),
            frames: Vec::new(),
            builtins: HashMap::new(),
            cells: HashMap::new(),
            computed_cells: Vec::new(),
            watchers: Vec::new(),
            channels,
            next_task_id: 0,
            task_results,
            routes,
            static_dirs,
            db: None,
            memoized: HashMap::new(),
            memo_cache: HashMap::new(),
            thrown_value: None,
            typed_channels: Arc::new(Mutex::new(HashMap::new())),
            next_channel_id: 0,
            task_registry: Arc::new(Mutex::new(HashMap::new())),
            cancel_token: Arc::new(AtomicBool::new(false)),
            task_group_stack: Vec::new(),
            profiler: None,
            otel: OtelState::new(),
        };
        vm.register_builtins();
        vm.register_io_builtins();
        vm.register_tier3_builtins();
        vm.register_tier4_builtins();
        vm.register_http_builtins();
        vm.register_db_builtins();
        vm.register_concurrency_builtins();
        crate::builtins_serial::register(&mut vm);
        crate::builtins_otel::register(&mut vm);
        vm
    }

    /// Create a child VM for spawned tasks — shares words, builtins, channels, routes
    pub fn spawn_child(&self) -> VM {
        VM {
            stack: Vec::new(),
            named_stacks: HashMap::new(),
            globals: self.globals.clone(),
            words: self.words.clone(),
            frames: Vec::new(),
            builtins: self.builtins.clone(),
            cells: HashMap::new(),
            computed_cells: Vec::new(),
            watchers: Vec::new(),
            channels: Arc::clone(&self.channels),
            next_task_id: 0,
            task_results: Arc::clone(&self.task_results),
            routes: Arc::clone(&self.routes),
            static_dirs: Arc::clone(&self.static_dirs),
            db: self.db.clone(),
            memoized: self.memoized.clone(),
            memo_cache: self.memo_cache.clone(),
            thrown_value: None,
            typed_channels: Arc::clone(&self.typed_channels),
            next_channel_id: 0,
            task_registry: Arc::clone(&self.task_registry),
            cancel_token: Arc::new(AtomicBool::new(false)),  // child gets its own token
            task_group_stack: Vec::new(),
            profiler: None,  // children don't profile
            otel: OtelState::new(),  // children get their own otel state
        }
    }

    fn register_io_builtins(&mut self) {
        for (name, func) in crate::builtins_io::io_builtins() {
            self.builtins.insert(name.to_string(), func);
        }
    }

    fn register_tier3_builtins(&mut self) {
        // ---- Reactive Cells ----

        // value "name" cell -> (creates a reactive cell)
        self.builtins.insert("cell".to_string(), |vm| {
            let name = vm.pop()?;
            let value = vm.pop()?;
            match name {
                Value::Str(n) => {
                    vm.cells.insert(n, value);
                    Ok(())
                }
                _ => Err("'cell' expects string name".to_string()),
            }
        });

        // "name" get_cell -> value
        self.builtins.insert("get_cell".to_string(), |vm| {
            let name = vm.pop()?;
            match name {
                Value::Str(n) => {
                    let val = vm.cells.get(&n)
                        .ok_or_else(|| format!("Cell '{}' not found", n))?
                        .clone();
                    vm.stack.push(val);
                    Ok(())
                }
                _ => Err("'get_cell' expects string name".to_string()),
            }
        });

        // value "name" set_cell -> (updates cell, triggers recomputation)
        self.builtins.insert("set_cell".to_string(), |vm| {
            let name = vm.pop()?;
            let value = vm.pop()?;
            match name {
                Value::Str(n) => {
                    vm.cells.insert(n, value);
                    vm.recompute_cells()?;
                    vm.check_watchers()?;
                    Ok(())
                }
                _ => Err("'set_cell' expects string name".to_string()),
            }
        });

        // [quotation] "name" computed -> (registers a computed cell)
        self.builtins.insert("computed".to_string(), |vm| {
            let name = vm.pop()?;
            let quot = vm.pop()?;
            match (quot, name) {
                (Value::Quotation(code), Value::Str(n)) => {
                    // Evaluate initially
                    vm.run_quotation_internal(&code)?;
                    let val = vm.pop()?;
                    vm.cells.insert(n.clone(), val);
                    vm.computed_cells.push(ComputedCell { name: n, code });
                    Ok(())
                }
                _ => Err("'computed' expects quotation and string name".to_string()),
            }
        });

        // [condition] [action] when -> (registers a watcher)
        self.builtins.insert("when".to_string(), |vm| {
            let action = vm.pop()?;
            let condition = vm.pop()?;
            match (condition, action) {
                (Value::Quotation(cond), Value::Quotation(act)) => {
                    vm.watchers.push(Watcher { condition: cond, action: act, was_true: false });
                    Ok(())
                }
                _ => Err("'when' expects two quotations (condition, action)".to_string()),
            }
        });

        // ---- Concurrency ----

        // [quotation] spawn -> task_id
        self.builtins.insert("spawn".to_string(), |vm| {
            let quot = vm.pop()?;
            match quot {
                Value::Quotation(code) => {
                    let task_id = vm.next_task_id;
                    vm.next_task_id += 1;

                    let mut child = vm.spawn_child();
                    let results = Arc::clone(&vm.task_results);

                    // Mark as pending
                    results.lock().unwrap().insert(task_id, None);

                    std::thread::spawn(move || {
                        let result = child.run(code)
                            .and_then(|_| child.stack.pop().ok_or_else(|| "Task produced no result".to_string()));
                        results.lock().unwrap().insert(task_id, Some(result));
                    });

                    vm.stack.push(Value::Int(task_id as i64));
                    Ok(())
                }
                _ => Err("'spawn' expects quotation".to_string()),
            }
        });

        // task_id await -> result value
        self.builtins.insert("await".to_string(), |vm| {
            let id = vm.pop()?;
            match id {
                Value::Int(task_id) => {
                    let tid = task_id as u64;
                    loop {
                        let result = {
                            let guard = vm.task_results.lock().unwrap();
                            guard.get(&tid).and_then(|r| r.clone())
                        };
                        if let Some(result) = result {
                            match result {
                                Ok(val) => {
                                    vm.stack.push(val);
                                    return Ok(());
                                }
                                Err(e) => return Err(format!("Task {} failed: {}", task_id, e)),
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'await' expects int task id".to_string()),
            }
        });

        // "value" "channel" send -> (sends value to named channel)
        self.builtins.insert("send".to_string(), |vm| {
            let chan = vm.pop()?;
            let value = vm.pop()?;
            match chan {
                Value::Str(name) => {
                    let mut channels = vm.channels.lock().unwrap();
                    channels.entry(name).or_default().push_back(value);
                    Ok(())
                }
                _ => Err("'send' expects string channel name".to_string()),
            }
        });

        // "channel" recv -> value (blocking receive)
        self.builtins.insert("recv".to_string(), |vm| {
            let chan = vm.pop()?;
            match chan {
                Value::Str(name) => {
                    loop {
                        {
                            let mut channels = vm.channels.lock().unwrap();
                            if let Some(queue) = channels.get_mut(&name) {
                                if let Some(val) = queue.pop_front() {
                                    vm.stack.push(val);
                                    return Ok(());
                                }
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'recv' expects string channel name".to_string()),
            }
        });

        // "channel" try_recv -> {:ok, value} or {:error, "empty"}
        self.builtins.insert("try_recv".to_string(), |vm| {
            let chan = vm.pop()?;
            match chan {
                Value::Str(name) => {
                    let mut channels = vm.channels.lock().unwrap();
                    if let Some(queue) = channels.get_mut(&name) {
                        if let Some(val) = queue.pop_front() {
                            vm.stack.push(Value::List(vec![
                                Value::Symbol("ok".to_string()),
                                val,
                            ]));
                            return Ok(());
                        }
                    }
                    vm.stack.push(Value::List(vec![
                        Value::Symbol("error".to_string()),
                        Value::Str("empty".to_string()),
                    ]));
                    Ok(())
                }
                _ => Err("'try_recv' expects string channel name".to_string()),
            }
        });

        // ---- Stack Traces ----

        // backtrace -> {"word1", "word2", ...} (call stack as list)
        self.builtins.insert("backtrace".to_string(), |vm| {
            let trace: Vec<Value> = vm.frames.iter().rev()
                .filter_map(|f| f.word_name.as_ref())
                .map(|n| Value::Str(n.clone()))
                .collect();
            vm.stack.push(Value::List(trace));
            Ok(())
        });

        // depth -> int (current stack depth)
        self.builtins.insert("depth".to_string(), |vm| {
            vm.stack.push(Value::Int(vm.stack.len() as i64));
            Ok(())
        });

        // ---- Hot Reload ----

        // "path" load -> (evaluates a .sabo file in current VM context)
        self.builtins.insert("load".to_string(), |vm| {
            let path = vm.pop()?;
            match path {
                Value::Str(p) => {
                    let source = std::fs::read_to_string(&p)
                        .map_err(|e| format!("Cannot read '{}': {}", p, e))?;
                    vm.eval_source(&source)?;
                    Ok(())
                }
                _ => Err("'load' expects string path".to_string()),
            }
        });

        // "path" watch -> (watches file, re-evaluates on change)
        self.builtins.insert("watch".to_string(), |vm| {
            let path = vm.pop()?;
            match path {
                Value::Str(p) => {
                    // Get initial mtime
                    let meta = std::fs::metadata(&p)
                        .map_err(|e| format!("Cannot stat '{}': {}", p, e))?;
                    let initial_mtime = meta.modified()
                        .map_err(|e| format!("Cannot get mtime: {}", e))?;

                    // Load initially
                    let source = std::fs::read_to_string(&p)
                        .map_err(|e| format!("Cannot read '{}': {}", p, e))?;
                    vm.eval_source(&source)?;

                    // Spawn watcher thread that sends on channel when file changes
                    let channels = Arc::clone(&vm.channels);
                    let path_clone = p.clone();
                    std::thread::spawn(move || {
                        let mut last_mtime = initial_mtime;
                        loop {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            if let Ok(meta) = std::fs::metadata(&path_clone) {
                                if let Ok(mtime) = meta.modified() {
                                    if mtime != last_mtime {
                                        last_mtime = mtime;
                                        let mut ch = channels.lock().unwrap();
                                        ch.entry("__watch__".to_string())
                                            .or_default()
                                            .push_back(Value::Str(path_clone.clone()));
                                    }
                                }
                            }
                        }
                    });

                    println!("  watching '{}'...", p);
                    Ok(())
                }
                _ => Err("'watch' expects string path".to_string()),
            }
        });

        // poll_watch -> reloads any changed watched files
        self.builtins.insert("poll_watch".to_string(), |vm| {
            let paths: Vec<Value> = {
                let mut channels = vm.channels.lock().unwrap();
                if let Some(queue) = channels.get_mut("__watch__") {
                    queue.drain(..).collect()
                } else {
                    vec![]
                }
            };
            for path_val in paths {
                if let Value::Str(p) = path_val {
                    match std::fs::read_to_string(&p) {
                        Ok(source) => {
                            println!("  reloading '{}'...", p);
                            vm.eval_source(&source)?;
                        }
                        Err(e) => eprintln!("  watch reload error: {}", e),
                    }
                }
            }
            Ok(())
        });
    }

    fn register_tier4_builtins(&mut self) {
        // ---- Testing / Assertions ----

        // value assert -> () (fails if not truthy)
        self.builtins.insert("assert".to_string(), |vm| {
            let val = vm.pop()?;
            if val.is_truthy() {
                Ok(())
            } else {
                Err(format!("Assertion failed: got {}", val))
            }
        });

        // a b assert_eq -> () (fails if a != b)
        self.builtins.insert("assert_eq".to_string(), |vm| {
            let b = vm.pop()?;
            let a = vm.pop()?;
            if a == b {
                Ok(())
            } else {
                Err(format!("Assertion failed: {} != {}", a, b))
            }
        });

        // a b assert_neq -> () (fails if a == b)
        self.builtins.insert("assert_neq".to_string(), |vm| {
            let b = vm.pop()?;
            let a = vm.pop()?;
            if a != b {
                Ok(())
            } else {
                Err(format!("Assertion failed: {} == {} (expected different)", a, b))
            }
        });

        // ---- Introspection ----

        // value type_of -> string
        self.builtins.insert("type_of".to_string(), |vm| {
            let val = vm.pop()?;
            let t = match &val {
                Value::Int(_) => "int",
                Value::Float(_) => "float",
                Value::Str(_) => "string",
                Value::Symbol(_) => "symbol",
                Value::Bool(_) => "bool",
                Value::List(_) => "list",
                Value::Map(_) => "map",
                Value::Quotation(_) => "quotation",
            };
            vm.stack.push(Value::Str(t.to_string()));
            Ok(())
        });

        // words -> list of all defined word names
        self.builtins.insert("words".to_string(), |vm| {
            let mut names: Vec<Value> = vm.words.keys()
                .map(|k| Value::Str(k.clone()))
                .collect();
            names.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            vm.stack.push(Value::List(names));
            Ok(())
        });

        // globals -> list of all global variable names
        self.builtins.insert("globals".to_string(), |vm| {
            let mut names: Vec<Value> = vm.globals.keys()
                .map(|k| Value::Str(k.clone()))
                .collect();
            names.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            vm.stack.push(Value::List(names));
            Ok(())
        });

        // ---- Module System ----

        // "path" import -> (loads file, prefixes its words with module name)
        self.builtins.insert("import".to_string(), |vm| {
            let path_val = vm.pop()?;
            match path_val {
                Value::Str(path) => {
                    // Derive module name from filename (e.g., "lib/math.sabo" -> "math")
                    let module_name = std::path::Path::new(&path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| format!("Invalid module path: {}", path))?
                        .to_string();

                    let source = std::fs::read_to_string(&path)
                        .map_err(|e| format!("Cannot import '{}': {}", path, e))?;

                    // Compile and run in a temporary child to capture words
                    let mut lexer = crate::lexer::Lexer::new(&source);
                    let tokens = lexer.tokenize().map_err(|e| format!("Import '{}': {}", path, e))?;
                    let mut parser = crate::parser::Parser::new(tokens);
                    let program = parser.parse().map_err(|e| format!("Import '{}': {}", path, e))?;
                    let mut compiler = crate::compiler::Compiler::new();
                    let compiled = compiler.compile(&program).map_err(|e| format!("Import '{}': {}", path, e))?;

                    // Add words with module prefix
                    for (name, code) in &compiled.words {
                        let qualified = format!("{}.{}", module_name, name);
                        vm.words.insert(qualified, code.clone());
                        // Also keep unqualified for backward compat
                        vm.words.insert(name.clone(), code.clone());
                    }

                    // Run the module's main code (top-level expressions)
                    vm.run_block(compiled.main)?;

                    Ok(())
                }
                _ => Err("'import' expects string path".to_string()),
            }
        });

        // ---- String Utilities ----

        // string split -> list (split on whitespace)
        self.builtins.insert("split".to_string(), |vm| {
            let delim = vm.pop()?;
            let s = vm.pop()?;
            match (s, delim) {
                (Value::Str(s), Value::Str(d)) => {
                    let parts: Vec<Value> = s.split(&d)
                        .map(|p| Value::Str(p.to_string()))
                        .collect();
                    vm.stack.push(Value::List(parts));
                    Ok(())
                }
                _ => Err("'split' expects two strings".to_string()),
            }
        });

        // list delim join -> string
        self.builtins.insert("join".to_string(), |vm| {
            let delim = vm.pop()?;
            let list = vm.pop()?;
            match (list, delim) {
                (Value::List(items), Value::Str(d)) => {
                    let s: String = items.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(&d);
                    vm.stack.push(Value::Str(s));
                    Ok(())
                }
                _ => Err("'join' expects list and string delimiter".to_string()),
            }
        });

        // string trim -> string
        self.builtins.insert("trim".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Str(s) => {
                    vm.stack.push(Value::Str(s.trim().to_string()));
                    Ok(())
                }
                _ => Err("'trim' expects string".to_string()),
            }
        });

        // string lowercase -> string
        self.builtins.insert("lowercase".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Str(s) => {
                    vm.stack.push(Value::Str(s.to_lowercase()));
                    Ok(())
                }
                _ => Err("'lowercase' expects string".to_string()),
            }
        });

        // string uppercase -> string
        self.builtins.insert("uppercase".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Str(s) => {
                    vm.stack.push(Value::Str(s.to_uppercase()));
                    Ok(())
                }
                _ => Err("'uppercase' expects string".to_string()),
            }
        });

        // string starts_with -> bool
        self.builtins.insert("starts_with".to_string(), |vm| {
            let prefix = vm.pop()?;
            let s = vm.pop()?;
            match (s, prefix) {
                (Value::Str(s), Value::Str(p)) => {
                    vm.stack.push(Value::Bool(s.starts_with(&p)));
                    Ok(())
                }
                _ => Err("'starts_with' expects two strings".to_string()),
            }
        });

        // string contains -> bool
        self.builtins.insert("contains".to_string(), |vm| {
            let needle = vm.pop()?;
            let haystack = vm.pop()?;
            match (haystack, needle) {
                (Value::Str(h), Value::Str(n)) => {
                    vm.stack.push(Value::Bool(h.contains(&n)));
                    Ok(())
                }
                (Value::List(items), needle) => {
                    vm.stack.push(Value::Bool(items.contains(&needle)));
                    Ok(())
                }
                _ => Err("'contains' expects string+string or list+value".to_string()),
            }
        });

        // value to_int -> int
        self.builtins.insert("to_int".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Int(n) => { vm.stack.push(Value::Int(n)); Ok(()) }
                Value::Float(f) => { vm.stack.push(Value::Int(f as i64)); Ok(()) }
                Value::Str(s) => {
                    let n = s.parse::<i64>().map_err(|_| format!("Cannot convert '{}' to int", s))?;
                    vm.stack.push(Value::Int(n));
                    Ok(())
                }
                Value::Bool(b) => { vm.stack.push(Value::Int(if b { 1 } else { 0 })); Ok(()) }
                _ => Err(format!("Cannot convert {} to int", val)),
            }
        });

        // value to_float -> float
        self.builtins.insert("to_float".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Float(f) => { vm.stack.push(Value::Float(f)); Ok(()) }
                Value::Int(n) => { vm.stack.push(Value::Float(n as f64)); Ok(()) }
                Value::Str(s) => {
                    let f = s.parse::<f64>().map_err(|_| format!("Cannot convert '{}' to float", s))?;
                    vm.stack.push(Value::Float(f));
                    Ok(())
                }
                _ => Err(format!("Cannot convert {} to float", val)),
            }
        });

        // ---- Memoization ----

        // "word_name" memo -> () (enable memoization for a word)
        self.builtins.insert("memo".to_string(), |vm| {
            let name = vm.pop()?;
            match name {
                Value::Str(n) => {
                    // Determine arity from the word's first MatchBegin op
                    let arity = vm.words.get(&n)
                        .and_then(|block| {
                            block.ops.iter().find_map(|op| {
                                if let Op::MatchBegin(a) = op { Some(*a) } else { None }
                            })
                        })
                        .ok_or_else(|| format!("Cannot memo '{}': word not found", n))?;
                    vm.memoized.insert(n, arity);
                    Ok(())
                }
                _ => Err("'memo' expects string word name".into()),
            }
        });

        // "word_name" memo_clear -> () (clear memo cache for a word)
        self.builtins.insert("memo_clear".to_string(), |vm| {
            let name = vm.pop()?;
            match name {
                Value::Str(n) => {
                    vm.memo_cache.remove(&n);
                    Ok(())
                }
                _ => Err("'memo_clear' expects string word name".into()),
            }
        });

        // ---- Regex ----

        // "string" "pattern" regex_match? -> bool
        self.builtins.insert("regex_match?".to_string(), |vm| {
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat) {
                (Value::Str(s), Value::Str(p)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    vm.stack.push(Value::Bool(re.is_match(&s)));
                    Ok(())
                }
                _ => Err("'regex_match?' expects two strings".into()),
            }
        });

        // "string" "pattern" regex_find -> {:ok, match_str} or :none
        self.builtins.insert("regex_find".to_string(), |vm| {
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat) {
                (Value::Str(s), Value::Str(p)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    match re.find(&s) {
                        Some(m) => {
                            vm.stack.push(Value::List(vec![
                                Value::Symbol("ok".into()),
                                Value::Str(m.as_str().to_string()),
                            ]));
                        }
                        None => vm.stack.push(Value::Symbol("none".into())),
                    }
                    Ok(())
                }
                _ => Err("'regex_find' expects two strings".into()),
            }
        });

        // "string" "pattern" regex_find_all -> {match1, match2, ...}
        self.builtins.insert("regex_find_all".to_string(), |vm| {
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat) {
                (Value::Str(s), Value::Str(p)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    let matches: Vec<Value> = re.find_iter(&s)
                        .map(|m| Value::Str(m.as_str().to_string()))
                        .collect();
                    vm.stack.push(Value::List(matches));
                    Ok(())
                }
                _ => Err("'regex_find_all' expects two strings".into()),
            }
        });

        // "string" "pattern" "replacement" regex_replace -> string
        self.builtins.insert("regex_replace".to_string(), |vm| {
            let replacement = vm.pop()?;
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat, replacement) {
                (Value::Str(s), Value::Str(p), Value::Str(r)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    let result = re.replace_all(&s, r.as_str()).to_string();
                    vm.stack.push(Value::Str(result));
                    Ok(())
                }
                _ => Err("'regex_replace' expects three strings".into()),
            }
        });

        // "string" "pattern" regex_split -> {parts...}
        self.builtins.insert("regex_split".to_string(), |vm| {
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat) {
                (Value::Str(s), Value::Str(p)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    let parts: Vec<Value> = re.split(&s)
                        .map(|part| Value::Str(part.to_string()))
                        .collect();
                    vm.stack.push(Value::List(parts));
                    Ok(())
                }
                _ => Err("'regex_split' expects two strings".into()),
            }
        });

        // "string" "pattern" regex_captures -> {capture1, capture2, ...} or :none
        self.builtins.insert("regex_captures".to_string(), |vm| {
            let pat = vm.pop()?;
            let s = vm.pop()?;
            match (s, pat) {
                (Value::Str(s), Value::Str(p)) => {
                    let re = regex::Regex::new(&p)
                        .map_err(|e| format!("Invalid regex '{}': {}", p, e))?;
                    match re.captures(&s) {
                        Some(caps) => {
                            let groups: Vec<Value> = caps.iter()
                                .map(|m| match m {
                                    Some(m) => Value::Str(m.as_str().to_string()),
                                    None => Value::Symbol("none".into()),
                                })
                                .collect();
                            vm.stack.push(Value::List(groups));
                        }
                        None => vm.stack.push(Value::Symbol("none".into())),
                    }
                    Ok(())
                }
                _ => Err("'regex_captures' expects two strings".into()),
            }
        });
    }

    fn register_http_builtins(&mut self) {
        // [handler_quot] "/path" "METHOD" route
        self.builtins.insert("route".to_string(), |vm| {
            let method = vm.pop()?;
            let pattern = vm.pop()?;
            let handler = vm.pop()?;
            match (handler, pattern, method) {
                (Value::Quotation(code), Value::Str(pat), Value::Str(meth)) => {
                    let segments = crate::builtins_http::compile_pattern(&pat);
                    let route = crate::builtins_http::Route {
                        method: meth.to_uppercase(),
                        pattern: pat,
                        segments,
                        handler: code,
                    };
                    vm.routes.lock().unwrap().push(route);
                    Ok(())
                }
                _ => Err("'route' expects: [handler] \"/path\" \"METHOD\"".into()),
            }
        });

        // "/fs/path" "/url/prefix" static_dir
        self.builtins.insert("static_dir".to_string(), |vm| {
            let url_prefix = vm.pop()?;
            let fs_path = vm.pop()?;
            match (fs_path, url_prefix) {
                (Value::Str(fs), Value::Str(url)) => {
                    vm.static_dirs.lock().unwrap().push(
                        crate::builtins_http::StaticDir {
                            url_prefix: url,
                            fs_path: fs,
                        }
                    );
                    Ok(())
                }
                _ => Err("'static_dir' expects two strings (fs_path, url_prefix)".into()),
            }
        });

        // port serve  (blocks — runs the HTTP server)
        self.builtins.insert("serve".to_string(), |vm| {
            let port = vm.pop()?;
            match port {
                Value::Int(p) => {
                    let addr = format!("0.0.0.0:{}", p);
                    let listener = std::net::TcpListener::bind(&addr)
                        .map_err(|e| format!("Cannot bind to {}: {}", addr, e))?;

                    println!("  Sabo HTTP server listening on http://localhost:{}", p);
                    println!("  Press Ctrl+C to stop\n");

                    // Snapshot routes and static dirs
                    let routes = vm.routes.lock().unwrap().clone();
                    let static_dirs = vm.static_dirs.lock().unwrap().clone();

                    // Log registered routes
                    for r in &routes {
                        println!("  {} {}", r.method, r.pattern);
                    }
                    for sd in &static_dirs {
                        println!("  STATIC {}/* -> {}/", sd.url_prefix, sd.fs_path);
                    }
                    println!();

                    // Create a template VM for spawning request handlers
                    let vm_template = vm.spawn_child();

                    for stream in listener.incoming() {
                        match stream {
                            Ok(stream) => {
                                let routes = routes.clone();
                                let static_dirs = static_dirs.clone();
                                let template = vm_template.spawn_child();
                                std::thread::spawn(move || {
                                    crate::builtins_http::handle_connection(
                                        stream, &routes, &static_dirs, &template,
                                    );
                                });
                            }
                            Err(e) => {
                                eprintln!("  Connection error: {}", e);
                            }
                        }
                    }
                    Ok(())
                }
                _ => Err("'serve' expects int port number".into()),
            }
        });
    }

    fn register_db_builtins(&mut self) {
        // "path/to/db.sqlite" db_open
        self.builtins.insert("db_open".to_string(), |vm| {
            let path = vm.pop()?;
            match path {
                Value::Str(p) => {
                    let conn = rusqlite::Connection::open(&p)
                        .map_err(|e| format!("db_open failed: {}", e))?;
                    vm.db = Some(Arc::new(Mutex::new(conn)));
                    Ok(())
                }
                _ => Err("'db_open' expects string path".into()),
            }
        });

        // ":memory:" db_open  (in-memory database)
        // Already handled by db_open since rusqlite::Connection::open(":memory:") works

        // db_close
        self.builtins.insert("db_close".to_string(), |vm| {
            vm.db = None;
            Ok(())
        });

        // "SQL" db_exec  (execute DDL/DML, pushes affected row count)
        self.builtins.insert("db_exec".to_string(), |vm| {
            let sql = vm.pop()?;
            match sql {
                Value::Str(s) => {
                    let db = vm.db.as_ref()
                        .ok_or("No database open (use db_open first)")?;
                    let conn = db.lock().unwrap();
                    let count = crate::builtins_db::exec(&conn, &s, &[])?;
                    vm.stack.push(Value::Int(count as i64));
                    Ok(())
                }
                _ => Err("'db_exec' expects string SQL".into()),
            }
        });

        // "SQL" {params} db_exec_params  (execute with bound parameters)
        self.builtins.insert("db_exec_params".to_string(), |vm| {
            let params = vm.pop()?;
            let sql = vm.pop()?;
            match (sql, params) {
                (Value::Str(s), Value::List(p)) => {
                    let db = vm.db.as_ref()
                        .ok_or("No database open (use db_open first)")?;
                    let conn = db.lock().unwrap();
                    let count = crate::builtins_db::exec(&conn, &s, &p)?;
                    vm.stack.push(Value::Int(count as i64));
                    Ok(())
                }
                _ => Err("'db_exec_params' expects string SQL and list of params".into()),
            }
        });

        // "SQL" db_query  (query, returns list of maps)
        self.builtins.insert("db_query".to_string(), |vm| {
            let sql = vm.pop()?;
            match sql {
                Value::Str(s) => {
                    let db = vm.db.as_ref()
                        .ok_or("No database open (use db_open first)")?;
                    let conn = db.lock().unwrap();
                    let result = crate::builtins_db::query(&conn, &s, &[])?;
                    vm.stack.push(result);
                    Ok(())
                }
                _ => Err("'db_query' expects string SQL".into()),
            }
        });

        // "SQL" {params} db_query_params  (query with bound parameters)
        self.builtins.insert("db_query_params".to_string(), |vm| {
            let params = vm.pop()?;
            let sql = vm.pop()?;
            match (sql, params) {
                (Value::Str(s), Value::List(p)) => {
                    let db = vm.db.as_ref()
                        .ok_or("No database open (use db_open first)")?;
                    let conn = db.lock().unwrap();
                    let result = crate::builtins_db::query(&conn, &s, &p)?;
                    vm.stack.push(result);
                    Ok(())
                }
                _ => Err("'db_query_params' expects string SQL and list of params".into()),
            }
        });
    }

    fn register_concurrency_builtins(&mut self) {
        // ---- Typed Channels ----

        // channel -> channel_id (creates a new typed channel)
        self.builtins.insert("channel".to_string(), |vm| {
            let id = vm.next_channel_id;
            vm.next_channel_id += 1;
            let inner = Arc::new(Mutex::new(ChannelInner {
                queue: VecDeque::new(),
                closed: false,
            }));
            vm.typed_channels.lock().unwrap().insert(id, inner);
            vm.stack.push(Value::Int(id as i64));
            Ok(())
        });

        // value channel_id ch_send -> () (send to typed channel)
        self.builtins.insert("ch_send".to_string(), |vm| {
            let ch_id = vm.pop()?;
            let value = vm.pop()?;
            match ch_id {
                Value::Int(id) => {
                    let registry = vm.typed_channels.lock().unwrap();
                    let ch = registry.get(&(id as u64))
                        .ok_or_else(|| format!("Channel {} does not exist", id))?
                        .clone();
                    drop(registry);
                    let mut inner = ch.lock().unwrap();
                    if inner.closed {
                        return Err(format!("Channel {} is closed", id));
                    }
                    inner.queue.push_back(value);
                    Ok(())
                }
                _ => Err("'ch_send' expects int channel id".into()),
            }
        });

        // channel_id ch_recv -> value (blocking receive from typed channel)
        self.builtins.insert("ch_recv".to_string(), |vm| {
            let ch_id = vm.pop()?;
            match ch_id {
                Value::Int(id) => {
                    let ch = {
                        let registry = vm.typed_channels.lock().unwrap();
                        registry.get(&(id as u64))
                            .ok_or_else(|| format!("Channel {} does not exist", id))?
                            .clone()
                    };
                    loop {
                        {
                            let mut inner = ch.lock().unwrap();
                            if let Some(val) = inner.queue.pop_front() {
                                vm.stack.push(val);
                                return Ok(());
                            }
                            if inner.closed {
                                vm.stack.push(Value::Symbol("closed".into()));
                                return Ok(());
                            }
                        }
                        // Check cancellation while waiting
                        if vm.cancel_token.load(Ordering::Relaxed) {
                            return Err("Task cancelled".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'ch_recv' expects int channel id".into()),
            }
        });

        // channel_id ch_try_recv -> {:ok, value} or :empty or :closed
        self.builtins.insert("ch_try_recv".to_string(), |vm| {
            let ch_id = vm.pop()?;
            match ch_id {
                Value::Int(id) => {
                    let registry = vm.typed_channels.lock().unwrap();
                    let ch = registry.get(&(id as u64))
                        .ok_or_else(|| format!("Channel {} does not exist", id))?
                        .clone();
                    drop(registry);
                    let mut inner = ch.lock().unwrap();
                    if let Some(val) = inner.queue.pop_front() {
                        vm.stack.push(Value::List(vec![
                            Value::Symbol("ok".into()),
                            val,
                        ]));
                    } else if inner.closed {
                        vm.stack.push(Value::Symbol("closed".into()));
                    } else {
                        vm.stack.push(Value::Symbol("empty".into()));
                    }
                    Ok(())
                }
                _ => Err("'ch_try_recv' expects int channel id".into()),
            }
        });

        // channel_id ch_close -> () (close a channel)
        self.builtins.insert("ch_close".to_string(), |vm| {
            let ch_id = vm.pop()?;
            match ch_id {
                Value::Int(id) => {
                    let registry = vm.typed_channels.lock().unwrap();
                    let ch = registry.get(&(id as u64))
                        .ok_or_else(|| format!("Channel {} does not exist", id))?
                        .clone();
                    drop(registry);
                    ch.lock().unwrap().closed = true;
                    Ok(())
                }
                _ => Err("'ch_close' expects int channel id".into()),
            }
        });

        // ---- Cancellation ----

        // task_id cancel -> () (signal cancellation to a task)
        self.builtins.insert("cancel".to_string(), |vm| {
            let id = vm.pop()?;
            match id {
                Value::Int(task_id) => {
                    let registry = vm.task_registry.lock().unwrap();
                    if let Some(info) = registry.get(&(task_id as u64)) {
                        info.cancel_token.store(true, Ordering::Relaxed);
                        // Also cancel all children
                        for &child_id in &info.children {
                            if let Some(child_info) = registry.get(&child_id) {
                                child_info.cancel_token.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    Ok(())
                }
                _ => Err("'cancel' expects int task id".into()),
            }
        });

        // cancelled? -> bool (check if current task is cancelled)
        self.builtins.insert("cancelled?".to_string(), |vm| {
            let cancelled = vm.cancel_token.load(Ordering::Relaxed);
            vm.stack.push(Value::Bool(cancelled));
            Ok(())
        });

        // ---- Task Groups (Structured Concurrency) ----

        // [quotation] spawn_linked -> task_id (spawn and track in current group)
        self.builtins.insert("spawn_linked".to_string(), |vm| {
            let quot = vm.pop()?;
            match quot {
                Value::Quotation(code) => {
                    let task_id = vm.next_task_id;
                    vm.next_task_id += 1;

                    let cancel_token = Arc::new(AtomicBool::new(false));
                    let mut child = vm.spawn_child();
                    child.cancel_token = Arc::clone(&cancel_token);

                    let registry = Arc::clone(&vm.task_registry);
                    let results = Arc::clone(&vm.task_results);

                    // Register task info
                    registry.lock().unwrap().insert(task_id, TaskInfo {
                        result: None,
                        cancel_token: Arc::clone(&cancel_token),
                        children: Vec::new(),
                    });
                    results.lock().unwrap().insert(task_id, None);

                    // Track in current task group (if any)
                    if let Some(group) = vm.task_group_stack.last_mut() {
                        group.push(task_id);
                    }

                    std::thread::spawn(move || {
                        let result = child.run(code)
                            .and_then(|_| child.stack.pop().ok_or_else(|| "Task produced no result".to_string()));
                        results.lock().unwrap().insert(task_id, Some(result.clone()));
                        registry.lock().unwrap().entry(task_id).and_modify(|info| {
                            info.result = Some(result);
                        });
                    });

                    vm.stack.push(Value::Int(task_id as i64));
                    Ok(())
                }
                _ => Err("'spawn_linked' expects quotation".into()),
            }
        });

        // {task_ids} await_all -> {results} (wait for all tasks, return list of results)
        self.builtins.insert("await_all".to_string(), |vm| {
            let ids = vm.pop()?;
            match ids {
                Value::List(id_list) => {
                    let mut results = Vec::new();
                    for id_val in &id_list {
                        match id_val {
                            Value::Int(task_id) => {
                                let tid = *task_id as u64;
                                loop {
                                    let result = {
                                        let guard = vm.task_results.lock().unwrap();
                                        guard.get(&tid).and_then(|r| r.clone())
                                    };
                                    if let Some(result) = result {
                                        match result {
                                            Ok(val) => {
                                                results.push(Value::List(vec![
                                                    Value::Symbol("ok".into()),
                                                    val,
                                                ]));
                                                break;
                                            }
                                            Err(e) => {
                                                results.push(Value::List(vec![
                                                    Value::Symbol("error".into()),
                                                    Value::Str(e),
                                                ]));
                                                break;
                                            }
                                        }
                                    }
                                    if vm.cancel_token.load(Ordering::Relaxed) {
                                        return Err("Task cancelled while awaiting".into());
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(1));
                                }
                            }
                            _ => return Err("'await_all' expects list of int task ids".into()),
                        }
                    }
                    vm.stack.push(Value::List(results));
                    Ok(())
                }
                _ => Err("'await_all' expects list of task ids".into()),
            }
        });

        // {task_ids} await_any -> {task_id, result} (wait for first, cancel rest)
        self.builtins.insert("await_any".to_string(), |vm| {
            let ids = vm.pop()?;
            match ids {
                Value::List(id_list) => {
                    let task_ids: Vec<u64> = id_list.iter().map(|v| match v {
                        Value::Int(id) => Ok(*id as u64),
                        _ => Err("'await_any' expects list of int task ids".to_string()),
                    }).collect::<Result<_, _>>()?;

                    // Poll until any task completes
                    loop {
                        for &tid in &task_ids {
                            let result = {
                                let guard = vm.task_results.lock().unwrap();
                                guard.get(&tid).and_then(|r| r.clone())
                            };
                            if let Some(result) = result {
                                // Cancel all other tasks
                                let registry = vm.task_registry.lock().unwrap();
                                for &other_tid in &task_ids {
                                    if other_tid != tid {
                                        if let Some(info) = registry.get(&other_tid) {
                                            info.cancel_token.store(true, Ordering::Relaxed);
                                        }
                                    }
                                }
                                drop(registry);

                                let result_val = match result {
                                    Ok(val) => Value::List(vec![Value::Symbol("ok".into()), val]),
                                    Err(e) => Value::List(vec![Value::Symbol("error".into()), Value::Str(e)]),
                                };
                                vm.stack.push(Value::List(vec![
                                    Value::Int(tid as i64),
                                    result_val,
                                ]));
                                return Ok(());
                            }
                        }
                        if vm.cancel_token.load(Ordering::Relaxed) {
                            return Err("Task cancelled while awaiting".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'await_any' expects list of task ids".into()),
            }
        });

        // ---- Scheduling Primitives ----

        // [quotation] timeout_ms timeout -> {:ok, result} or {:error, :timeout}
        self.builtins.insert("timeout".to_string(), |vm| {
            let ms = vm.pop()?;
            let quot = vm.pop()?;
            match (quot, ms) {
                (Value::Quotation(code), Value::Int(timeout_ms)) => {
                    let cancel_token = Arc::new(AtomicBool::new(false));
                    let mut child = vm.spawn_child();
                    child.cancel_token = Arc::clone(&cancel_token);

                    let results: Arc<Mutex<Option<Result<Value, String>>>> = Arc::new(Mutex::new(None));
                    let results_clone = Arc::clone(&results);

                    std::thread::spawn(move || {
                        let result = child.run(code)
                            .and_then(|_| child.stack.pop().ok_or_else(|| "No result".into()));
                        *results_clone.lock().unwrap() = Some(result);
                    });

                    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms as u64);
                    loop {
                        if let Some(result) = results.lock().unwrap().take() {
                            match result {
                                Ok(val) => {
                                    vm.stack.push(Value::List(vec![
                                        Value::Symbol("ok".into()),
                                        val,
                                    ]));
                                    return Ok(());
                                }
                                Err(e) => {
                                    vm.stack.push(Value::List(vec![
                                        Value::Symbol("error".into()),
                                        Value::Str(e),
                                    ]));
                                    return Ok(());
                                }
                            }
                        }
                        if std::time::Instant::now() >= deadline {
                            cancel_token.store(true, Ordering::Relaxed);
                            vm.stack.push(Value::List(vec![
                                Value::Symbol("error".into()),
                                Value::Symbol("timeout".into()),
                            ]));
                            return Ok(());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'timeout' expects quotation and int milliseconds".into()),
            }
        });

        // {channel_ids} select -> {channel_id, value} (wait on multiple channels)
        self.builtins.insert("select".to_string(), |vm| {
            let ids = vm.pop()?;
            match ids {
                Value::List(id_list) => {
                    let ch_ids: Vec<u64> = id_list.iter().map(|v| match v {
                        Value::Int(id) => Ok(*id as u64),
                        _ => Err("'select' expects list of int channel ids".to_string()),
                    }).collect::<Result<_, _>>()?;

                    // Gather Arc handles once
                    let channels: Vec<(u64, Arc<Mutex<ChannelInner>>)> = {
                        let registry = vm.typed_channels.lock().unwrap();
                        ch_ids.iter().map(|&id| {
                            let ch = registry.get(&id)
                                .ok_or_else(|| format!("Channel {} does not exist", id))?
                                .clone();
                            Ok((id, ch))
                        }).collect::<Result<_, String>>()?
                    };

                    loop {
                        for (id, ch) in &channels {
                            let mut inner = ch.lock().unwrap();
                            if let Some(val) = inner.queue.pop_front() {
                                vm.stack.push(Value::List(vec![
                                    Value::Int(*id as i64),
                                    val,
                                ]));
                                return Ok(());
                            }
                            if inner.closed {
                                vm.stack.push(Value::List(vec![
                                    Value::Int(*id as i64),
                                    Value::Symbol("closed".into()),
                                ]));
                                return Ok(());
                            }
                        }
                        if vm.cancel_token.load(Ordering::Relaxed) {
                            return Err("Task cancelled while selecting".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                _ => Err("'select' expects list of channel ids".into()),
            }
        });
    }

    fn register_builtins(&mut self) {
        self.builtins.insert("print".to_string(), |vm| {
            let val = vm.pop()?;
            print!("{}", val);
            Ok(())
        });
        self.builtins.insert("println".to_string(), |vm| {
            let val = vm.pop()?;
            println!("{}", val);
            Ok(())
        });
        self.builtins.insert("dup".to_string(), |vm| {
            let val = vm.peek()?.clone();
            vm.stack.push(val);
            Ok(())
        });
        self.builtins.insert("drop".to_string(), |vm| {
            vm.pop()?;
            Ok(())
        });
        self.builtins.insert("swap".to_string(), |vm| {
            let a = vm.pop()?;
            let b = vm.pop()?;
            vm.stack.push(a);
            vm.stack.push(b);
            Ok(())
        });
        self.builtins.insert("over".to_string(), |vm| {
            if vm.stack.len() < 2 {
                return Err("Stack underflow on 'over'".to_string());
            }
            let val = vm.stack[vm.stack.len() - 2].clone();
            vm.stack.push(val);
            Ok(())
        });
        self.builtins.insert("rot".to_string(), |vm| {
            if vm.stack.len() < 3 {
                return Err("Stack underflow on 'rot'".to_string());
            }
            let len = vm.stack.len();
            let a = vm.stack.remove(len - 3);
            vm.stack.push(a);
            Ok(())
        });
        self.builtins.insert("len".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(l) => vm.stack.push(Value::Int(l.len() as i64)),
                Value::Str(s) => vm.stack.push(Value::Int(s.len() as i64)),
                Value::Map(m) => vm.stack.push(Value::Int(m.len() as i64)),
                _ => return Err(format!("'len' expects list, string, or map, got {}", val.type_name())),
            }
            Ok(())
        });
        self.builtins.insert("head".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(l) => {
                    if l.is_empty() { return Err("'head' on empty list".to_string()); }
                    vm.stack.push(l[0].clone());
                }
                _ => return Err(format!("'head' expects list, got {}", val.type_name())),
            }
            Ok(())
        });
        self.builtins.insert("tail".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(l) => {
                    if l.is_empty() { return Err("'tail' on empty list".to_string()); }
                    vm.stack.push(Value::List(l[1..].to_vec()));
                }
                _ => return Err(format!("'tail' expects list, got {}", val.type_name())),
            }
            Ok(())
        });
        self.builtins.insert("cons".to_string(), |vm| {
            let list = vm.pop()?;
            let item = vm.pop()?;
            match list {
                Value::List(mut l) => {
                    l.insert(0, item);
                    vm.stack.push(Value::List(l));
                }
                _ => return Err(format!("'cons' expects list, got {}", list.type_name())),
            }
            Ok(())
        });
        self.builtins.insert("empty".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(l) => vm.stack.push(Value::Bool(l.is_empty())),
                Value::Str(s) => vm.stack.push(Value::Bool(s.is_empty())),
                Value::Map(m) => vm.stack.push(Value::Bool(m.is_empty())),
                _ => return Err(format!("'empty' expects list, string, or map, got {}", val.type_name())),
            }
            Ok(())
        });
        self.builtins.insert("type".to_string(), |vm| {
            let val = vm.pop()?;
            vm.stack.push(Value::Symbol(val.type_name().to_string()));
            Ok(())
        });
        self.builtins.insert("stack".to_string(), |vm| {
            print!("<");
            for (i, v) in vm.stack.iter().enumerate() {
                if i > 0 { print!(" "); }
                print!("{}", v);
            }
            println!(">");
            Ok(())
        });

        // to_str: convert any value to its string representation
        self.builtins.insert("to_str".to_string(), |vm| {
            let val = vm.pop()?;
            vm.stack.push(Value::Str(val.to_display_string()));
            Ok(())
        });

        // Map operations
        self.builtins.insert("get".to_string(), |vm| {
            let key = vm.pop()?;
            let map = vm.pop()?;
            match map {
                Value::Map(map) => {
                    match map.get(&key) {
                        Some(v) => { vm.stack.push(v.clone()); Ok(()) }
                        None => Err(format!("Key {} not found in map", key)),
                    }
                }
                _ => Err(format!("'get' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("get_or".to_string(), |vm| {
            let default = vm.pop()?;
            let key = vm.pop()?;
            let map = vm.pop()?;
            match map {
                Value::Map(map) => {
                    match map.get(&key) {
                        Some(v) => vm.stack.push(v.clone()),
                        None => vm.stack.push(default),
                    }
                    Ok(())
                }
                _ => Err(format!("'get_or' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("set".to_string(), |vm| {
            let val = vm.pop()?;
            let key = vm.pop()?;
            let map = vm.pop()?;
            match map {
                Value::Map(mut map) => {
                    map.insert(key, val);
                    vm.stack.push(Value::Map(map));
                    Ok(())
                }
                _ => Err(format!("'set' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("keys".to_string(), |vm| {
            let map = vm.pop()?;
            match map {
                Value::Map(pairs) => {
                    let keys: Vec<Value> = pairs.into_iter().map(|(k, _)| k).collect();
                    vm.stack.push(Value::List(keys));
                    Ok(())
                }
                _ => Err(format!("'keys' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("values".to_string(), |vm| {
            let map = vm.pop()?;
            match map {
                Value::Map(pairs) => {
                    let vals: Vec<Value> = pairs.into_iter().map(|(_, v)| v).collect();
                    vm.stack.push(Value::List(vals));
                    Ok(())
                }
                _ => Err(format!("'values' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("has".to_string(), |vm| {
            let key = vm.pop()?;
            let map = vm.pop()?;
            match map {
                Value::Map(map) => {
                    vm.stack.push(Value::Bool(map.contains_key(&key)));
                    Ok(())
                }
                _ => Err(format!("'has' expects map, got {}", map.type_name())),
            }
        });
        self.builtins.insert("merge".to_string(), |vm| {
            let b = vm.pop()?;
            let a = vm.pop()?;
            match (a, b) {
                (Value::Map(mut a_map), Value::Map(b_map)) => {
                    a_map.extend(b_map);
                    vm.stack.push(Value::Map(a_map));
                    Ok(())
                }
                _ => Err("'merge' expects two maps".to_string()),
            }
        });

        // bool [then] [else] if_else -> runs then or else based on condition
        self.builtins.insert("if_else".to_string(), |vm| {
            let else_branch = vm.pop()?;
            let then_branch = vm.pop()?;
            let cond = vm.pop()?;
            let branch = if cond.is_truthy() { then_branch } else { else_branch };
            match branch {
                Value::Quotation(code) => {
                    vm.run_quotation_internal(&code)?;
                    Ok(())
                }
                _ => Err("'if_else' expects two quotations".to_string()),
            }
        });

        // bool [then] if_true -> runs then if condition is truthy
        self.builtins.insert("if_true".to_string(), |vm| {
            let then_branch = vm.pop()?;
            let cond = vm.pop()?;
            if cond.is_truthy() {
                match then_branch {
                    Value::Quotation(code) => {
                        vm.run_quotation_internal(&code)?;
                    }
                    _ => return Err("'if_true' expects quotation".to_string()),
                }
            }
            Ok(())
        });

        // try: execute a quotation, catch errors
        // [code] try -> {:ok, result} or {:error, message}
        self.builtins.insert("try".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Quotation(code) => {
                    // Save stack state
                    let saved_stack = vm.stack.clone();
                    let saved_frames_len = vm.frames.len();

                    // Push frame and run
                    vm.frames.push(CallFrame {
                        code,
                        lines: Vec::new(),
                        ip: 0,
                        locals: Vec::new(),
                        match_stack: Vec::new(),
                        word_name: Some("try".to_string()),
                    });

                    // Execute until this frame completes or error
                    loop {
                        if vm.frames.len() <= saved_frames_len {
                            break;
                        }
                        match vm.step_inner() {
                            Ok(true) => break,
                            Ok(false) => continue,
                            Err(e) => {
                                // Restore stack and push error
                                vm.stack = saved_stack;
                                // Pop any frames we added
                                while vm.frames.len() > saved_frames_len {
                                    vm.frames.pop();
                                }
                                // Check if this was a structured throw
                                if let Some(thrown) = vm.thrown_value.take() {
                                    vm.stack.push(Value::List(vec![
                                        Value::Symbol("error".to_string()),
                                        thrown,
                                    ]));
                                } else {
                                    vm.stack.push(Value::List(vec![
                                        Value::Symbol("error".to_string()),
                                        Value::Str(e),
                                    ]));
                                }
                                return Ok(());
                            }
                        }
                    }

                    // Success: wrap top of stack in {:ok, result}
                    let result = vm.stack.pop().unwrap_or(Value::Bool(false));
                    vm.stack.push(Value::List(vec![
                        Value::Symbol("ok".to_string()),
                        result,
                    ]));
                    Ok(())
                }
                _ => Err(format!("'try' expects quotation, got {}", val.type_name())),
            }
        });

        // throw: throw a structured error value (caught by try)
        // value throw -> error
        self.builtins.insert("throw".to_string(), |vm| {
            let val = vm.pop()?;
            vm.thrown_value = Some(val.clone());
            Err(format!("Thrown: {}", val))
        });

        // error: convenience to build error tuples
        // "message" :type error -> throws {:error, :type, "message"}
        self.builtins.insert("error".to_string(), |vm| {
            let error_type = vm.pop()?;
            let message = vm.pop()?;
            let error_val = Value::List(vec![
                Value::Symbol("error".to_string()),
                error_type,
                message,
            ]);
            vm.thrown_value = Some(error_val.clone());
            Err(format!("Thrown: {}", error_val))
        });

        // each: iterate a quotation over a list
        // {list} [quot] each
        self.builtins.insert("each".to_string(), |vm| {
            let quot = vm.pop()?;
            let list = vm.pop()?;
            match (list, quot) {
                (Value::List(items), Value::Quotation(code)) => {
                    for item in items {
                        vm.stack.push(item);
                        vm.frames.push(CallFrame {
                            code: code.clone(),
                            lines: Vec::new(),
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: None,
                        });
                        // Run the quotation frame to completion
                        let target_depth = vm.frames.len() - 1;
                        loop {
                            if vm.frames.len() <= target_depth {
                                break;
                            }
                            let done = vm.step_inner()?;
                            if done { break; }
                        }
                    }
                    Ok(())
                }
                _ => Err("'each' expects list and quotation".to_string()),
            }
        });

        // map_list: {list} [quot] map_list -> {new_list}
        self.builtins.insert("map_list".to_string(), |vm| {
            let quot = vm.pop()?;
            let list = vm.pop()?;
            match (list, quot) {
                (Value::List(items), Value::Quotation(code)) => {
                    let mut results = Vec::new();
                    for item in items {
                        vm.stack.push(item);
                        vm.frames.push(CallFrame {
                            code: code.clone(),
                            lines: Vec::new(),
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: None,
                        });
                        let target_depth = vm.frames.len() - 1;
                        loop {
                            if vm.frames.len() <= target_depth {
                                break;
                            }
                            let done = vm.step_inner()?;
                            if done { break; }
                        }
                        results.push(vm.pop()?);
                    }
                    vm.stack.push(Value::List(results));
                    Ok(())
                }
                _ => Err("'map_list' expects list and quotation".to_string()),
            }
        });

        // map: alias for map_list
        self.builtins.insert("map".to_string(), self.builtins["map_list"]);

        // filter: {list} [pred] filter -> {filtered}
        self.builtins.insert("filter".to_string(), |vm| {
            let quot = vm.pop()?;
            let list = vm.pop()?;
            match (list, quot) {
                (Value::List(items), Value::Quotation(code)) => {
                    let mut results = Vec::new();
                    for item in items {
                        vm.stack.push(item.clone());
                        vm.frames.push(CallFrame {
                            code: code.clone(),
                            lines: Vec::new(),
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: None,
                        });
                        let target_depth = vm.frames.len() - 1;
                        loop {
                            if vm.frames.len() <= target_depth { break; }
                            let done = vm.step_inner()?;
                            if done { break; }
                        }
                        let pred = vm.pop()?;
                        if pred.is_truthy() {
                            results.push(item);
                        }
                    }
                    vm.stack.push(Value::List(results));
                    Ok(())
                }
                _ => Err("'filter' expects list and quotation".to_string()),
            }
        });

        // fold: {list} init [quot] fold -> result
        // quot receives: accumulator element
        self.builtins.insert("fold".to_string(), |vm| {
            let quot = vm.pop()?;
            let init = vm.pop()?;
            let list = vm.pop()?;
            match (list, quot) {
                (Value::List(items), Value::Quotation(code)) => {
                    vm.stack.push(init);
                    for item in items {
                        vm.stack.push(item);
                        vm.frames.push(CallFrame {
                            code: code.clone(),
                            lines: Vec::new(),
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: None,
                        });
                        let target_depth = vm.frames.len() - 1;
                        loop {
                            if vm.frames.len() <= target_depth { break; }
                            let done = vm.step_inner()?;
                            if done { break; }
                        }
                    }
                    Ok(())
                }
                _ => Err("'fold' expects list, init value, and quotation".to_string()),
            }
        });

        // range: start end range -> {start, start+1, ..., end-1}
        self.builtins.insert("range".to_string(), |vm| {
            let end = vm.pop()?;
            let start = vm.pop()?;
            match (start, end) {
                (Value::Int(s), Value::Int(e)) => {
                    let items: Vec<Value> = (s..e).map(Value::Int).collect();
                    vm.stack.push(Value::List(items));
                    Ok(())
                }
                _ => Err("'range' expects two integers".to_string()),
            }
        });

        // split: "str" "delim" split -> {parts}
        self.builtins.insert("split".to_string(), |vm| {
            let delim = vm.pop()?;
            let s = vm.pop()?;
            match (s, delim) {
                (Value::Str(s), Value::Str(d)) => {
                    let parts: Vec<Value> = s.split(&d).map(|p| Value::Str(p.to_string())).collect();
                    vm.stack.push(Value::List(parts));
                    Ok(())
                }
                _ => Err("'split' expects two strings".to_string()),
            }
        });

        // join: {list} "delim" join -> "str"
        self.builtins.insert("join".to_string(), |vm| {
            let delim = vm.pop()?;
            let list = vm.pop()?;
            match (list, delim) {
                (Value::List(items), Value::Str(d)) => {
                    let parts: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                    vm.stack.push(Value::Str(parts.join(&d)));
                    Ok(())
                }
                _ => Err("'join' expects list and string".to_string()),
            }
        });

        // int: convert to int
        self.builtins.insert("int".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Int(_) => vm.stack.push(val),
                Value::Float(f) => vm.stack.push(Value::Int(f as i64)),
                Value::Str(s) => {
                    match s.parse::<i64>() {
                        Ok(n) => vm.stack.push(Value::Int(n)),
                        Err(_) => return Err(format!("Cannot convert '{}' to int", s)),
                    }
                }
                Value::Bool(b) => vm.stack.push(Value::Int(if b { 1 } else { 0 })),
                _ => return Err(format!("Cannot convert {} to int", val.type_name())),
            }
            Ok(())
        });

        // float: convert to float
        self.builtins.insert("float".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::Float(_) => vm.stack.push(val),
                Value::Int(n) => vm.stack.push(Value::Float(n as f64)),
                Value::Str(s) => {
                    match s.parse::<f64>() {
                        Ok(f) => vm.stack.push(Value::Float(f)),
                        Err(_) => return Err(format!("Cannot convert '{}' to float", s)),
                    }
                }
                _ => return Err(format!("Cannot convert {} to float", val.type_name())),
            }
            Ok(())
        });

        // append: {list} val append -> {list, val}
        self.builtins.insert("append".to_string(), |vm| {
            let val = vm.pop()?;
            let list = vm.pop()?;
            match list {
                Value::List(mut l) => {
                    l.push(val);
                    vm.stack.push(Value::List(l));
                    Ok(())
                }
                _ => Err(format!("'append' expects list, got {}", list.type_name())),
            }
        });

        // reverse
        self.builtins.insert("reverse".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(mut l) => {
                    l.reverse();
                    vm.stack.push(Value::List(l));
                    Ok(())
                }
                Value::Str(s) => {
                    vm.stack.push(Value::Str(s.chars().rev().collect()));
                    Ok(())
                }
                _ => Err(format!("'reverse' expects list or string, got {}", val.type_name())),
            }
        });

        // sort (lists of comparable values)
        self.builtins.insert("sort".to_string(), |vm| {
            let val = vm.pop()?;
            match val {
                Value::List(mut l) => {
                    l.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    vm.stack.push(Value::List(l));
                    Ok(())
                }
                _ => Err(format!("'sort' expects list, got {}", val.type_name())),
            }
        });

        // nth — list index access
        self.builtins.insert("nth".to_string(), |vm| {
            let idx = vm.pop()?;
            let list = vm.pop()?;
            match (list, idx) {
                (Value::List(l), Value::Int(i)) => {
                    let i = i as usize;
                    if i >= l.len() {
                        return Err(format!("Index {} out of bounds (len {})", i, l.len()));
                    }
                    vm.stack.push(l[i].clone());
                    Ok(())
                }
                _ => Err("'nth' expects list and int".to_string()),
            }
        });

        // replace — string replacement
        self.builtins.insert("replace".to_string(), |vm| {
            let replacement = vm.pop()?;
            let pattern = vm.pop()?;
            let s = vm.pop()?;
            match (s, pattern, replacement) {
                (Value::Str(s), Value::Str(p), Value::Str(r)) => {
                    vm.stack.push(Value::Str(s.replace(&p, &r)));
                    Ok(())
                }
                _ => Err("'replace' expects three strings (str, pattern, replacement)".to_string()),
            }
        });

        // chars — split string into list of single-character strings
        self.builtins.insert("chars".to_string(), |vm| {
            let s = vm.pop()?;
            match s {
                Value::Str(s) => {
                    let chars: Vec<Value> = s.chars()
                        .map(|c| Value::Str(c.to_string()))
                        .collect();
                    vm.stack.push(Value::List(chars));
                    Ok(())
                }
                _ => Err(format!("'chars' expects string, got {}", s.type_name())),
            }
        });

        // contains
        self.builtins.insert("contains".to_string(), |vm| {
            let needle = vm.pop()?;
            let haystack = vm.pop()?;
            match haystack {
                Value::List(l) => {
                    vm.stack.push(Value::Bool(l.contains(&needle)));
                    Ok(())
                }
                Value::Str(s) => {
                    match needle {
                        Value::Str(n) => vm.stack.push(Value::Bool(s.contains(&n))),
                        _ => vm.stack.push(Value::Bool(false)),
                    }
                    Ok(())
                }
                _ => Err(format!("'contains' expects list or string, got {}", haystack.type_name())),
            }
        });
    }

    pub fn load_words(&mut self, words: HashMap<String, crate::compiler::CodeBlock>) {
        self.words.extend(words);
    }

    /// Evaluate a source string in this VM's context
    pub fn eval_source(&mut self, source: &str) -> Result<(), String> {
        let mut lex = crate::lexer::Lexer::new(source);
        let tokens = lex.tokenize()?;
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse()?;
        let mut compiler = crate::compiler::Compiler::new();
        let compiled = compiler.compile(&program)?;
        self.load_words(compiled.words);
        self.run_block(compiled.main)?;
        Ok(())
    }

    /// Run a quotation within the current VM (for reactive cells / internal use)
    fn run_quotation_internal(&mut self, code: &[Op]) -> Result<(), String> {
        // Inherit parent frame's locals so quotations can access enclosing bindings
        let parent_locals = self.frames.last()
            .map(|f| f.locals.clone())
            .unwrap_or_default();
        self.frames.push(CallFrame {
            code: code.to_vec(),
            lines: Vec::new(),
            ip: 0,
            locals: parent_locals,
            match_stack: Vec::new(),
            word_name: None,
        });
        let target_depth = self.frames.len() - 1;
        loop {
            if self.frames.len() <= target_depth {
                break;
            }
            let done = self.step_inner().map_err(|e| self.format_error(e))?;
            if done { break; }
        }
        Ok(())
    }

    /// Recompute all computed cells (called after set_cell)
    fn recompute_cells(&mut self) -> Result<(), String> {
        let saved_stack = self.stack.clone();
        for i in 0..self.computed_cells.len() {
            let code = self.computed_cells[i].code.clone();
            let name = self.computed_cells[i].name.clone();
            self.stack.clear();
            self.run_quotation_internal(&code)?;
            let val = self.pop()?;
            self.cells.insert(name, val);
        }
        self.stack = saved_stack;
        Ok(())
    }

    /// Check all watchers and fire those whose conditions become true (edge-triggered)
    fn check_watchers(&mut self) -> Result<(), String> {
        let saved_stack = self.stack.clone();
        for i in 0..self.watchers.len() {
            let cond = self.watchers[i].condition.clone();
            self.stack.clear();
            self.run_quotation_internal(&cond)?;
            let result = self.pop()?;
            let is_true = result.is_truthy();
            let was_true = self.watchers[i].was_true;
            self.watchers[i].was_true = is_true;
            if is_true && !was_true {
                let action = self.watchers[i].action.clone();
                self.stack = saved_stack.clone();
                self.run_quotation_internal(&action)?;
            }
        }
        self.stack = saved_stack;
        Ok(())
    }

    pub fn run_block(&mut self, block: crate::compiler::CodeBlock) -> Result<(), String> {
        self.frames.push(CallFrame {
            code: block.ops,
            lines: block.lines,
            ip: 0,
            locals: Vec::new(),
            match_stack: Vec::new(),
            word_name: None,
        });

        while !self.frames.is_empty() {
            let done = self.step_inner().map_err(|e| self.format_error(e))?;
            if done {
                break;
            }
        }
        Ok(())
    }

    pub fn run(&mut self, code: Vec<Op>) -> Result<(), String> {
        self.frames.push(CallFrame {
            code,
            lines: Vec::new(),
            ip: 0,
            locals: Vec::new(),
            match_stack: Vec::new(),
            word_name: None,
        });

        while !self.frames.is_empty() {
            let done = self.step_inner().map_err(|e| self.format_error(e))?;
            if done {
                break;
            }
        }
        Ok(())
    }

    /// Get the source line for the current instruction, if available
    fn current_source_line(&self) -> Option<usize> {
        let frame = self.frames.last()?;
        let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 };
        frame.lines.get(ip).copied().filter(|&l| l > 0)
    }

    /// Format an error with source location context
    fn format_error(&self, msg: String) -> String {
        // Don't double-wrap errors that already have line info
        if msg.starts_with("[line ") || msg.starts_with("in '") {
            return msg;
        }
        let line = self.current_source_line();
        let word = self.frames.last().and_then(|f| f.word_name.as_ref());
        match (line, word) {
            (Some(l), Some(w)) => format!("[line {}] in '{}': {}", l, w, msg),
            (Some(l), None) => format!("[line {}] {}", l, msg),
            (None, Some(w)) => format!("in '{}': {}", w, msg),
            (None, None) => msg,
        }
    }

    fn step_inner(&mut self) -> Result<bool, String> {
        if let Some(ref mut p) = self.profiler { p.tick(); }

        let frame = self.frames.last_mut().unwrap();
        if frame.ip >= frame.code.len() {
            let was_word = frame.word_name.clone();
            self.frames.pop();
            if was_word.is_some() {
                if let Some(ref mut p) = self.profiler { p.exit_word(); }
            }
            return Ok(self.frames.is_empty());
        }

        let op = frame.code[frame.ip].clone();
        frame.ip += 1;

        match op {
            Op::PushInt(n) => self.stack.push(Value::Int(n)),
            Op::PushFloat(f) => self.stack.push(Value::Float(f)),
            Op::PushStr(s) => self.stack.push(Value::Str(s)),
            Op::PushSymbol(s) => self.stack.push(Value::Symbol(s)),
            Op::PushBool(b) => self.stack.push(Value::Bool(b)),

            Op::PushList(n) => {
                if self.stack.len() < n {
                    return Err("Stack underflow creating list".to_string());
                }
                let start = self.stack.len() - n;
                let items: Vec<Value> = self.stack.drain(start..).collect();
                self.stack.push(Value::List(items));
            }

            Op::PushMap(n) => {
                let needed = n * 2;
                if self.stack.len() < needed {
                    return Err("Stack underflow creating map".to_string());
                }
                let start = self.stack.len() - needed;
                let flat: Vec<Value> = self.stack.drain(start..).collect();
                let mut map = std::collections::HashMap::with_capacity(n);
                for chunk in flat.chunks(2) {
                    map.insert(chunk[0].clone(), chunk[1].clone());
                }
                self.stack.push(Value::Map(map));
            }

            Op::Add => self.binary_op("+")?,
            Op::Sub => self.binary_op("-")?,
            Op::Mul => self.binary_op("*")?,
            Op::Div => self.binary_op("/")?,
            Op::Mod => self.binary_op("%")?,

            Op::Eq => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a == b)); }
            Op::NotEq => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a != b)); }
            Op::Lt => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a < b)); }
            Op::Gt => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a > b)); }
            Op::LtEq => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a <= b)); }
            Op::GtEq => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a >= b)); }

            Op::And => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a.is_truthy() && b.is_truthy())); }
            Op::Or => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a.is_truthy() || b.is_truthy())); }
            Op::Not => { let a = self.pop()?; self.stack.push(Value::Bool(!a.is_truthy())); }

            Op::Dup => { let v = self.peek()?.clone(); self.stack.push(v); }
            Op::Drop => { self.pop()?; }
            Op::Swap => {
                let a = self.pop()?;
                let b = self.pop()?;
                self.stack.push(a);
                self.stack.push(b);
            }
            Op::Over => {
                if self.stack.len() < 2 { return Err("Stack underflow".to_string()); }
                let v = self.stack[self.stack.len() - 2].clone();
                self.stack.push(v);
            }
            Op::Rot => {
                if self.stack.len() < 3 { return Err("Stack underflow".to_string()); }
                let len = self.stack.len();
                let v = self.stack.remove(len - 3);
                self.stack.push(v);
            }

            Op::PushTo(name) => {
                let val = self.pop()?;
                self.named_stacks.entry(name).or_default().push(val);
            }
            Op::PopFrom(name) => {
                let val = self.named_stacks.get_mut(&name)
                    .and_then(|s| s.pop())
                    .ok_or_else(|| format!("Named stack '{}' is empty", name))?;
                self.stack.push(val);
            }

            Op::StoreGlobal(name) => {
                let val = self.pop()?;
                self.globals.insert(name, val);
            }
            Op::LoadGlobal(name) => {
                let val = self.globals.get(&name)
                    .ok_or_else(|| format!("Undefined variable '{}'", name))?
                    .clone();
                self.stack.push(val);
            }

            Op::MatchBegin(n) => {
                if self.stack.len() < n {
                    return Err(format!("Stack underflow: need {} items for pattern match, have {}", n, self.stack.len()));
                }
                let start = self.stack.len() - n;
                let match_items: Vec<Value> = self.stack[start..].to_vec();
                let frame = self.frames.last_mut().unwrap();
                frame.match_stack = match_items;
                frame.locals.resize(16, Value::Bool(false));
            }

            Op::MatchLitInt(expected, fail) => {
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::Int(n)) if *n == expected);
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchLitFloat(expected_bits, fail) => {
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::Float(f)) if f.to_bits() == expected_bits);
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchLitStr(ref expected, fail) => {
                let expected = expected.clone();
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::Str(s)) if *s == expected);
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchLitSymbol(ref expected, fail) => {
                let expected = expected.clone();
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::Symbol(s)) if *s == expected);
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchLitBool(expected, fail) => {
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::Bool(b)) if *b == expected);
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchBind(slot) => {
                let frame = self.frames.last_mut().unwrap();
                if frame.match_stack.is_empty() {
                    return Err("Match bind failed: no value to bind".to_string());
                }
                let val = frame.match_stack.remove(0);
                if slot >= frame.locals.len() {
                    frame.locals.resize(slot + 1, Value::Bool(false));
                }
                frame.locals[slot] = val;
            }

            Op::MatchWildcard => {
                let frame = self.frames.last_mut().unwrap();
                if !frame.match_stack.is_empty() {
                    frame.match_stack.remove(0);
                }
            }

            Op::MatchListEmpty(fail) => {
                let frame = self.frames.last_mut().unwrap();
                let matched = matches!(frame.match_stack.first(), Some(Value::List(l)) if l.is_empty());
                if matched {
                    frame.match_stack.remove(0);
                } else {
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::MatchListCons(head_slot, tail_slot, fail) => {
                let frame = self.frames.last_mut().unwrap();
                let list_val = frame.match_stack.first().cloned();
                match list_val {
                    Some(Value::List(l)) if !l.is_empty() => {
                        let head = l[0].clone();
                        let tail = Value::List(l[1..].to_vec());
                        frame.match_stack.remove(0);
                        let max_slot = head_slot.max(tail_slot);
                        if max_slot >= frame.locals.len() {
                            frame.locals.resize(max_slot + 1, Value::Bool(false));
                        }
                        frame.locals[head_slot] = head;
                        frame.locals[tail_slot] = tail;
                    }
                    _ => {
                        frame.ip = find_label(&frame.code, fail)?;
                    }
                }
            }

            Op::MatchPop(n) => {
                if self.stack.len() >= n {
                    let new_len = self.stack.len() - n;
                    self.stack.truncate(new_len);
                }
            }

            Op::GuardFail(fail) => {
                let val = self.pop()?;
                if !val.is_truthy() {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = find_label(&frame.code, fail)?;
                }
            }

            Op::LoadLocal(slot) => {
                let frame = self.frames.last().unwrap();
                if slot >= frame.locals.len() {
                    let word = frame.word_name.clone().unwrap_or("(anonymous)".into());
                    return Err(format!(
                        "Local variable slot {} out of range (word '{}' has {} locals). \
                         This usually means a closure is referencing a variable from an outer scope \
                         that wasn't captured correctly.",
                        slot, word, frame.locals.len()
                    ));
                }
                let val = frame.locals[slot].clone();
                self.stack.push(val);
            }

            Op::Call(name) => {
                // Check builtins first, then words, then globals
                if let Some(builtin) = self.builtins.get(&name).cloned() {
                    if let Some(ref mut p) = self.profiler { p.enter_word(&name); }
                    builtin(self)?;
                    if let Some(ref mut p) = self.profiler { p.exit_word(); }
                } else if let Some(block) = self.words.get(&name).cloned() {
                    // Check memoization
                    if let Some(&arity) = self.memoized.get(&name) {
                        if self.stack.len() >= arity {
                            let key_start = self.stack.len() - arity;
                            let cache_key: String = self.stack[key_start..]
                                .iter()
                                .map(|v| format!("{:?}", v))
                                .collect::<Vec<_>>()
                                .join(",");

                            if let Some(cached) = self.memo_cache
                                .get(&name)
                                .and_then(|m| m.get(&cache_key))
                                .cloned()
                            {
                                // Cache hit: pop args, push result
                                for _ in 0..arity { self.stack.pop(); }
                                self.stack.push(cached);
                            } else {
                                // Cache miss: run word to completion, cache result
                                let _stack_len_before = self.stack.len() - arity;
                                if let Some(ref mut p) = self.profiler { p.enter_word(&name); }
                                self.frames.push(CallFrame {
                                    code: block.ops.clone(),
                                    lines: block.lines.clone(),
                                    ip: 0,
                                    locals: Vec::new(),
                                    match_stack: Vec::new(),
                                    word_name: Some(name.clone()),
                                });
                                let target_depth = self.frames.len() - 1;
                                loop {
                                    if self.frames.len() <= target_depth { break; }
                                    let done = self.step_inner()?;
                                    if done { break; }
                                }
                                // Cache the result (top of stack after execution)
                                if let Some(result) = self.stack.last().cloned() {
                                    self.memo_cache
                                        .entry(name)
                                        .or_default()
                                        .insert(cache_key, result);
                                }
                            }
                        } else {
                            // Not enough args, just run normally
                            if let Some(ref mut p) = self.profiler { p.enter_word(&name); }
                            self.frames.push(CallFrame {
                                code: block.ops,
                                lines: block.lines,
                                ip: 0,
                                locals: Vec::new(),
                                match_stack: Vec::new(),
                                word_name: Some(name),
                            });
                        }
                    } else {
                        if let Some(ref mut p) = self.profiler { p.enter_word(&name); }
                        self.frames.push(CallFrame {
                            code: block.ops,
                            lines: block.lines,
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: Some(name),
                        });
                    }
                } else if let Some(val) = self.globals.get(&name).cloned() {
                    self.stack.push(val);
                } else if let Some(val) = self.cells.get(&name).cloned() {
                    // Reactive cell: push its current value
                    self.stack.push(val);
                } else {
                    return Err(format!("Unknown word '{}'", name));
                }
            }

            Op::TailCall(name) => {
                // Reuse current frame instead of pushing a new one
                if let Some(block) = self.words.get(&name).cloned() {
                    if let Some(ref mut p) = self.profiler {
                        p.exit_word();
                        p.enter_word(&name);
                    }
                    let frame = self.frames.last_mut().unwrap();
                    frame.code = block.ops;
                    frame.lines = block.lines;
                    frame.ip = 0;
                    frame.locals.clear();
                    frame.match_stack.clear();
                    frame.word_name = Some(name);
                } else {
                    return Err(format!("Unknown word '{}' in tail call", name));
                }
            }

            Op::Jump(label) => {
                let frame = self.frames.last_mut().unwrap();
                frame.ip = find_label(&frame.code, label)?;
            }

            Op::JumpIf(label) => {
                let val = self.pop()?;
                if val.is_truthy() {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = find_label(&frame.code, label)?;
                }
            }

            Op::JumpIfNot(label) => {
                let val = self.pop()?;
                if !val.is_truthy() {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = find_label(&frame.code, label)?;
                }
            }

            Op::Return => {
                let was_word = self.frames.last().and_then(|f| f.word_name.clone());
                self.frames.pop();
                if was_word.is_some() {
                    if let Some(ref mut p) = self.profiler { p.exit_word(); }
                }
                if self.frames.is_empty() {
                    return Ok(true);
                }
            }

            Op::PushQuotation(code) => {
                self.stack.push(Value::Quotation(code));
            }

            Op::Apply => {
                let val = self.pop()?;
                match val {
                    Value::Quotation(code) => {
                        self.frames.push(CallFrame {
                            code,
                            lines: Vec::new(),
                            ip: 0,
                            locals: Vec::new(),
                            match_stack: Vec::new(),
                            word_name: Some("<apply>".to_string()),
                        });
                    }
                    _ => return Err(format!("'apply' expects quotation, got {}", val.type_name())),
                }
            }

            Op::Compose => {
                let b = self.pop()?;
                let a = self.pop()?;
                match (a, b) {
                    (Value::Quotation(mut a_ops), Value::Quotation(b_ops)) => {
                        a_ops.extend(b_ops);
                        self.stack.push(Value::Quotation(a_ops));
                    }
                    _ => return Err("'compose' expects two quotations".to_string()),
                }
            }

            Op::StrConcat => {
                let b = self.pop()?;
                let a = self.pop()?;
                match (a, b) {
                    (Value::Str(a), Value::Str(b)) => {
                        self.stack.push(Value::Str(format!("{}{}", a, b)));
                    }
                    (a, b) => {
                        self.stack.push(Value::Str(format!("{}{}", a, b)));
                    }
                }
            }

            Op::Print => {
                let val = self.pop()?;
                print!("{}", val);
            }
            Op::PrintLn => {
                let val = self.pop()?;
                println!("{}", val);
            }

            Op::Halt => {
                return Ok(true);
            }

            Op::Nop | Op::Label(_) => {}
        }

        Ok(false)
    }

    fn pop(&mut self) -> Result<Value, String> {
        self.stack.pop().ok_or_else(|| {
            let context = self.frames.last()
                .and_then(|f| f.word_name.as_ref())
                .map(|w| format!(" in '{}'", w))
                .unwrap_or_default();
            format!("Stack underflow{} (tried to pop from empty stack)", context)
        })
    }

    fn peek(&self) -> Result<&Value, String> {
        self.stack.last().ok_or_else(|| "Stack underflow".to_string())
    }

    fn binary_op(&mut self, op: &str) -> Result<(), String> {
        let b = self.pop()?;
        let a = self.pop()?;
        let result = match (a, &b, op) {
            (Value::Int(a), Value::Int(b), "+") => Value::Int(a + b),
            (Value::Int(a), Value::Int(b), "-") => Value::Int(a - b),
            (Value::Int(a), Value::Int(b), "*") => Value::Int(a * b),
            (Value::Int(a), Value::Int(b), "/") => {
                if *b == 0 { return Err("Division by zero".to_string()); }
                Value::Int(a / b)
            }
            (Value::Int(a), Value::Int(b), "%") => {
                if *b == 0 { return Err("Modulo by zero".to_string()); }
                Value::Int(a % b)
            }
            (Value::Float(a), Value::Float(b), "+") => Value::Float(a + b),
            (Value::Float(a), Value::Float(b), "-") => Value::Float(a - b),
            (Value::Float(a), Value::Float(b), "*") => Value::Float(a * b),
            (Value::Float(a), Value::Float(b), "/") => Value::Float(a / b),
            (Value::Float(a), Value::Float(b), "%") => Value::Float(a % b),
            (Value::Int(a), Value::Float(b), op) => {
                let a = a as f64;
                match op {
                    "+" => Value::Float(a + b),
                    "-" => Value::Float(a - b),
                    "*" => Value::Float(a * b),
                    "/" => Value::Float(a / b),
                    "%" => Value::Float(a % b),
                    _ => unreachable!(),
                }
            }
            (Value::Float(a), Value::Int(b), op) => {
                let b = *b as f64;
                match op {
                    "+" => Value::Float(a + b),
                    "-" => Value::Float(a - b),
                    "*" => Value::Float(a * b),
                    "/" => Value::Float(a / b),
                    "%" => Value::Float(a % b),
                    _ => unreachable!(),
                }
            }
            (Value::Str(a), Value::Str(b), "+") => Value::Str(format!("{}{}", a, b)),
            (a, b, op) => return Err(format!("Cannot apply '{}' to {} and {}", op, a.type_name(), b.type_name())),
        };
        self.stack.push(result);
        Ok(())
    }

    // Public accessors for builtins_io
    pub fn pop_val(&mut self) -> Result<Value, String> {
        self.pop()
    }

    pub fn push_val(&mut self, val: Value) {
        self.stack.push(val);
    }

    pub fn register_builtin(&mut self, name: &str, f: fn(&mut VM) -> Result<(), String>) {
        self.builtins.insert(name.to_string(), f);
    }

    pub fn otel_mut(&mut self) -> &mut OtelState {
        &mut self.otel
    }

    pub fn run_quotation(&mut self, code: &[Op]) -> Result<(), String> {
        self.run_quotation_internal(code)
    }

    pub fn stack_display(&self) -> String {
        if self.stack.is_empty() {
            return String::new();
        }
        let mut s = String::from("<");
        for (i, v) in self.stack.iter().enumerate() {
            if i > 0 { s.push(' '); }
            s.push_str(&format!("{}", v));
        }
        s.push('>');
        s
    }

    pub fn words_display(&self) -> String {
        if self.words.is_empty() {
            return "no words defined".to_string();
        }
        let mut names: Vec<&String> = self.words.keys().collect();
        names.sort();
        names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
    }

    pub fn globals_display(&self) -> String {
        if self.globals.is_empty() {
            return "no globals".to_string();
        }
        let mut pairs: Vec<String> = self.globals.iter()
            .map(|(k, v)| format!("{} = {}", k, v))
            .collect();
        pairs.sort();
        pairs.join(", ")
    }

    pub fn cells_display(&self) -> String {
        if self.cells.is_empty() {
            return "no cells".to_string();
        }
        let mut pairs: Vec<String> = self.cells.iter()
            .map(|(k, v)| format!("{} = {}", k, v))
            .collect();
        pairs.sort();
        pairs.join(", ")
    }

    pub fn clear_stack(&mut self) {
        self.stack.clear();
    }
}
