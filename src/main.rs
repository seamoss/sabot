mod ast;
mod builtins_db;
mod builtins_http;
mod builtins_io;
mod builtins_otel;
mod builtins_serial;
mod compiler;
mod formatter;
mod lexer;
mod opcode;
mod parser;
mod profiler;
mod token;
mod value;
mod vm;

use compiler::Compiler;
use lexer::Lexer;
use parser::Parser;
use std::io::{self, BufRead, Write};
use vm::VM;

fn run_source(source: &str, vm: &mut VM, compiler: &mut Compiler) -> Result<(), String> {
    let mut lex = Lexer::new(source);
    let tokens = lex.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    let compiled = compiler.compile(&program)?;
    vm.load_words(compiled.words);
    vm.run_block(compiled.main)?;
    Ok(())
}

fn load_rc(vm: &mut VM, compiler: &mut Compiler) {
    // Try .saborc in current directory, then home directory
    let candidates = vec![
        ".saborc".to_string(),
        dirs_next()
            .map(|h| format!("{}/.saborc", h))
            .unwrap_or_default(),
    ];

    for path in candidates {
        if path.is_empty() {
            continue;
        }
        if let Ok(source) = std::fs::read_to_string(&path) {
            match run_source(&source, vm, compiler) {
                Ok(()) => println!("Loaded {}", path),
                Err(e) => eprintln!("Warning: error in {}: {}", path, e),
            }
            return; // Only load the first one found
        }
    }
}

fn dirs_next() -> Option<String> {
    std::env::var("HOME").ok()
}

fn repl() {
    println!("Sabo v0.4.0 -- stack-based pattern matching language");
    println!("Type .help for commands, 'quit' to exit\n");

    let mut vm = VM::new();
    let mut compiler = Compiler::new();

    load_rc(&mut vm, &mut compiler);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut buffer = String::new();

    loop {
        if buffer.is_empty() {
            print!("sabo> ");
        } else {
            print!("  ... ");
        }
        stdout.flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }

        let trimmed = line.trim();
        if buffer.is_empty() {
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "quit" || trimmed == "exit" {
                break;
            }

            // REPL dot-commands
            match trimmed {
                ".help" => {
                    println!("  .stack    - show the current stack");
                    println!("  .words    - list defined words");
                    println!("  .globals  - list global variables");
                    println!("  .cells    - list reactive cells");
                    println!("  .clear    - clear the stack");
                    println!("  .reset    - reset the VM");
                    println!("  quit      - exit the REPL");
                    continue;
                }
                ".stack" | "stack" => {
                    println!("  {}", vm.stack_display());
                    continue;
                }
                ".words" => {
                    println!("  {}", vm.words_display());
                    continue;
                }
                ".globals" => {
                    println!("  {}", vm.globals_display());
                    continue;
                }
                ".cells" => {
                    println!("  {}", vm.cells_display());
                    continue;
                }
                ".clear" => {
                    vm.clear_stack();
                    println!("  stack cleared");
                    continue;
                }
                ".reset" => {
                    vm = VM::new();
                    compiler = Compiler::new();
                    println!("  VM reset");
                    continue;
                }
                _ => {}
            }
        }

        if buffer.is_empty() {
            buffer = line.to_string();
        } else {
            buffer.push_str(&line);
        }

        // Check if input is complete
        if lexer::is_incomplete(&buffer) {
            continue;
        }

        let source = std::mem::take(&mut buffer);
        let source = source.trim();
        if source.is_empty() {
            continue;
        }

        match run_source(source, &mut vm, &mut compiler) {
            Ok(()) => {
                let display = vm.stack_display();
                if !display.is_empty() {
                    println!("  {}", display);
                }
            }
            Err(e) => {
                eprintln!("  error: {}", e);
            }
        }
    }
}

fn run_file(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let mut vm = VM::new();
    let mut compiler = Compiler::new();

    if let Err(e) = run_source(&source, &mut vm, &mut compiler) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_single_test(path: &str) -> bool {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  FAIL  {}  (cannot read: {})", path, e);
            return false;
        }
    };

    let mut vm = VM::new();
    let mut compiler = Compiler::new();

    match run_source(&source, &mut vm, &mut compiler) {
        Ok(()) => {
            println!("  PASS  {}", path);
            true
        }
        Err(e) => {
            eprintln!("  FAIL  {}", path);
            eprintln!("        {}", e);
            false
        }
    }
}

fn run_tests(path: &str) {
    let meta = std::fs::metadata(path);
    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);

    if is_dir {
        // Run all test files in the directory
        let mut entries: Vec<String> = std::fs::read_dir(path)
            .unwrap_or_else(|e| {
                eprintln!("Cannot read directory '{}': {}", path, e);
                std::process::exit(1);
            })
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| {
                name.ends_with(".sabo") && (name.contains("test_") || name.contains("_test"))
            })
            .collect();
        entries.sort();

        if entries.is_empty() {
            eprintln!("No test files found in '{}'", path);
            std::process::exit(1);
        }

        let dir = path.trim_end_matches('/');
        println!("\nRunning {} test file(s) in {}/\n", entries.len(), dir);

        let mut passed = 0;
        let mut failed = 0;
        let start = std::time::Instant::now();

        for name in &entries {
            let full_path = format!("{}/{}", dir, name);
            if run_single_test(&full_path) {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        let elapsed = start.elapsed().as_millis();
        println!("\n--- Results ---");
        println!("Passed: {}", passed);
        println!("Failed: {}", failed);
        println!("Time:   {}ms", elapsed);

        if failed > 0 {
            std::process::exit(1);
        }
    } else {
        // Single file
        println!("Running tests: {}", path);
        if !run_single_test(path) {
            std::process::exit(1);
        }
        println!("  All tests passed!");
    }
}

fn format_file(path: &str, write_back: bool) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let formatted = formatter::format_source(&source);

    if write_back {
        if formatted == source {
            println!("{}: already formatted", path);
        } else {
            std::fs::write(path, &formatted).unwrap_or_else(|e| {
                eprintln!("Cannot write '{}': {}", path, e);
                std::process::exit(1);
            });
            println!("{}: formatted", path);
        }
    } else {
        print!("{}", formatted);
    }
}

fn format_files(args: &[String]) {
    let write_back = args.iter().any(|a| a == "-w" || a == "--write");
    let files: Vec<&String> = args
        .iter()
        .filter(|a| *a != "-w" && *a != "--write")
        .collect();

    if files.is_empty() {
        eprintln!("Usage: sabo fmt [-w] <file.sabo> [file2.sabo ...]");
        eprintln!("  -w, --write   Write formatted output back to file");
        std::process::exit(1);
    }

    for path in files {
        format_file(path, write_back);
    }
}

fn profile_file(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let mut vm = VM::new();
    vm.profiler = Some(profiler::Profiler::new());
    let mut compiler = Compiler::new();

    if let Err(e) = run_source(&source, &mut vm, &mut compiler) {
        eprintln!("Error: {}", e);
    }

    if let Some(ref p) = vm.profiler {
        p.report();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.len() {
        1 => repl(),
        2 => run_file(&args[1]),
        3 if args[1] == "test" => run_tests(&args[2]),
        _ if args.len() >= 2 && args[1] == "fmt" => format_files(&args[2..]),
        _ if args.len() >= 2 && args[1] == "profile" => {
            if args.len() < 3 {
                eprintln!("Usage: sabo profile <file.sabo>");
                std::process::exit(1);
            }
            profile_file(&args[2]);
        }
        _ if args.len() >= 2 && args[1] != "test" && args[1] != "fmt" && args[1] != "profile" => {
            run_file(&args[1])
        }
        _ => {
            eprintln!("Usage: sabo [file.sabo] [args...]");
            eprintln!("       sabo test <test_file_or_dir>");
            eprintln!("       sabo fmt [-w] <file.sabo> [...]");
            eprintln!("       sabo profile <file.sabo>");
            std::process::exit(1);
        }
    }
}
