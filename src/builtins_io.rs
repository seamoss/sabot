use crate::value::Value;
use std::collections::HashMap;
use std::io::Read as _;

// Register all I/O, shell, HTTP, and env builtins.
// Each builtin is a fn(&mut super::vm::VM) -> Result<(), String>.
// We can't directly reference VM here due to circular deps, so we return
// a vec of (name, fn pointer) pairs and let vm.rs register them.
// We use a trait object approach: vm.rs will call this and register.

pub type BuiltinFn = fn(&mut crate::vm::VM) -> Result<(), String>;

pub fn io_builtins() -> Vec<(&'static str, BuiltinFn)> {
    vec![
        // ---- File I/O ----

        // "path" read_file -> string
        ("read_file", |vm| {
            let path = vm.pop_val()?;
            match path {
                Value::Str(p) => {
                    let content = std::fs::read_to_string(&p)
                        .map_err(|e| format!("Cannot read '{}': {}", p, e))?;
                    vm.push_val(Value::Str(content));
                    Ok(())
                }
                _ => Err(format!(
                    "'read_file' expects string path, got {}",
                    path.type_name()
                )),
            }
        }),
        // "path" read_lines -> {line1, line2, ...}
        ("read_lines", |vm| {
            let path = vm.pop_val()?;
            match path {
                Value::Str(p) => {
                    let content = std::fs::read_to_string(&p)
                        .map_err(|e| format!("Cannot read '{}': {}", p, e))?;
                    let lines: Vec<Value> =
                        content.lines().map(|l| Value::Str(l.to_string())).collect();
                    vm.push_val(Value::List(lines));
                    Ok(())
                }
                _ => Err(format!(
                    "'read_lines' expects string path, got {}",
                    path.type_name()
                )),
            }
        }),
        // "content" "path" write_file -> (nothing, writes file)
        ("write_file", |vm| {
            let path = vm.pop_val()?;
            let content = vm.pop_val()?;
            match (content, path) {
                (Value::Str(c), Value::Str(p)) => {
                    std::fs::write(&p, &c).map_err(|e| format!("Cannot write '{}': {}", p, e))?;
                    Ok(())
                }
                _ => Err("'write_file' expects two strings (content, path)".to_string()),
            }
        }),
        // "content" "path" append_file -> (nothing)
        ("append_file", |vm| {
            let path = vm.pop_val()?;
            let content = vm.pop_val()?;
            match (content, path) {
                (Value::Str(c), Value::Str(p)) => {
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&p)
                        .map_err(|e| format!("Cannot open '{}': {}", p, e))?;
                    file.write_all(c.as_bytes())
                        .map_err(|e| format!("Cannot write '{}': {}", p, e))?;
                    Ok(())
                }
                _ => Err("'append_file' expects two strings (content, path)".to_string()),
            }
        }),
        // "path" file_exists -> bool
        ("file_exists", |vm| {
            let path = vm.pop_val()?;
            match path {
                Value::Str(p) => {
                    vm.push_val(Value::Bool(std::path::Path::new(&p).exists()));
                    Ok(())
                }
                _ => Err(format!(
                    "'file_exists' expects string, got {}",
                    path.type_name()
                )),
            }
        }),
        // "path" ls -> {entries...}
        ("ls", |vm| {
            let path = vm.pop_val()?;
            match path {
                Value::Str(p) => {
                    let mut entries = Vec::new();
                    let dir = std::fs::read_dir(&p)
                        .map_err(|e| format!("Cannot read dir '{}': {}", p, e))?;
                    for entry in dir {
                        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
                        entries.push(Value::Str(entry.file_name().to_string_lossy().to_string()));
                    }
                    entries.sort_by(|a, b| {
                        if let (Value::Str(a), Value::Str(b)) = (a, b) {
                            a.cmp(b)
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    });
                    vm.push_val(Value::List(entries));
                    Ok(())
                }
                _ => Err(format!(
                    "'ls' expects string path, got {}",
                    path.type_name()
                )),
            }
        }),
        // ---- Shell Execution ----

        // "cmd" exec -> stdout_string
        ("exec", |vm| {
            let cmd = vm.pop_val()?;
            match cmd {
                Value::Str(c) => {
                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&c)
                        .output()
                        .map_err(|e| format!("exec failed: {}", e))?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    vm.push_val(Value::Str(stdout));
                    Ok(())
                }
                _ => Err(format!(
                    "'exec' expects string command, got {}",
                    cmd.type_name()
                )),
            }
        }),
        // "cmd" exec_full -> #{"stdout" => ..., "stderr" => ..., "code" => ...}
        ("exec_full", |vm| {
            let cmd = vm.pop_val()?;
            match cmd {
                Value::Str(c) => {
                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&c)
                        .output()
                        .map_err(|e| format!("exec failed: {}", e))?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let code = output.status.code().unwrap_or(-1) as i64;
                    let mut map = HashMap::new();
                    map.insert(Value::Str("stdout".to_string()), Value::Str(stdout));
                    map.insert(Value::Str("stderr".to_string()), Value::Str(stderr));
                    map.insert(Value::Str("code".to_string()), Value::Int(code));
                    vm.push_val(Value::Map(map));
                    Ok(())
                }
                _ => Err(format!(
                    "'exec_full' expects string, got {}",
                    cmd.type_name()
                )),
            }
        }),
        // ---- HTTP ----

        // "url" http_get -> string (response body)
        ("http_get", |vm| {
            let url = vm.pop_val()?;
            match url {
                Value::Str(u) => {
                    let body = ureq::get(&u)
                        .call()
                        .map_err(|e| format!("HTTP GET failed: {}", e))?
                        .into_body()
                        .read_to_string()
                        .map_err(|e| format!("Failed to read response: {}", e))?;
                    vm.push_val(Value::Str(body));
                    Ok(())
                }
                _ => Err(format!(
                    "'http_get' expects string URL, got {}",
                    url.type_name()
                )),
            }
        }),
        // "body" "url" http_post -> string (response body)
        ("http_post", |vm| {
            let url = vm.pop_val()?;
            let body = vm.pop_val()?;
            match (body, url) {
                (Value::Str(b), Value::Str(u)) => {
                    let resp = ureq::post(&u)
                        .header("Content-Type", "application/json")
                        .send(&b)
                        .map_err(|e| format!("HTTP POST failed: {}", e))?
                        .into_body()
                        .read_to_string()
                        .map_err(|e| format!("Failed to read response: {}", e))?;
                    vm.push_val(Value::Str(resp));
                    Ok(())
                }
                _ => Err("'http_post' expects two strings (body, url)".to_string()),
            }
        }),
        // ---- Stdin ----

        // stdin_read -> string (all of stdin)
        ("stdin_read", |vm| {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("stdin read failed: {}", e))?;
            vm.push_val(Value::Str(buf));
            Ok(())
        }),
        // stdin_lines -> {line1, line2, ...}
        ("stdin_lines", |vm| {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            let lines: Vec<Value> = stdin
                .lock()
                .lines()
                .map(|l| Value::Str(l.unwrap_or_default()))
                .collect();
            vm.push_val(Value::List(lines));
            Ok(())
        }),
        // "prompt" input -> string (print prompt, read one line)
        ("input", |vm| {
            use std::io::Write;
            let prompt = vm.pop_val()?;
            match prompt {
                Value::Str(p) => {
                    print!("{}", p);
                    std::io::stdout().flush().unwrap();
                    let mut line = String::new();
                    let bytes = std::io::stdin()
                        .read_line(&mut line)
                        .map_err(|e| format!("input failed: {}", e))?;
                    if bytes == 0 {
                        return Err("EOF on stdin".to_string());
                    }
                    // Trim the trailing newline
                    let trimmed = line
                        .trim_end_matches('\n')
                        .trim_end_matches('\r')
                        .to_string();
                    vm.push_val(Value::Str(trimmed));
                    Ok(())
                }
                _ => Err(format!(
                    "'input' expects string prompt, got {}",
                    prompt.type_name()
                )),
            }
        }),
        // ---- Terminal ----

        // term_raw -> (puts terminal in raw mode)
        ("term_raw", |_vm| {
            use std::os::unix::io::AsRawFd;
            let fd = std::io::stdin().as_raw_fd();
            unsafe {
                let mut termios: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut termios) != 0 {
                    return Err("tcgetattr failed".to_string());
                }
                libc::cfmakeraw(&mut termios);
                if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
                    return Err("tcsetattr failed".to_string());
                }
            }
            Ok(())
        }),
        // term_cooked -> (restores terminal to cooked/sane mode)
        ("term_cooked", |_vm| {
            use std::os::unix::io::AsRawFd;
            let fd = std::io::stdin().as_raw_fd();
            unsafe {
                let mut termios: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut termios) != 0 {
                    return Err("tcgetattr failed".to_string());
                }
                termios.c_iflag |= libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON;
                termios.c_oflag |= libc::OPOST;
                termios.c_cflag |= libc::CS8;
                termios.c_lflag |= libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG;
                if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
                    return Err("tcsetattr failed".to_string());
                }
            }
            Ok(())
        }),
        // stdin_char -> string (reads a single byte from stdin, returns 1-char string)
        ("stdin_char", |vm| {
            use std::io::Read;
            let mut buf = [0u8; 1];
            std::io::stdin()
                .read_exact(&mut buf)
                .map_err(|e| format!("stdin_char failed: {}", e))?;
            vm.push_val(Value::Str(String::from(buf[0] as char)));
            Ok(())
        }),
        // stdin_byte -> int (reads a single byte from stdin, returns its numeric value)
        ("stdin_byte", |vm| {
            use std::io::Read;
            let mut buf = [0u8; 1];
            std::io::stdin()
                .read_exact(&mut buf)
                .map_err(|e| format!("stdin_byte failed: {}", e))?;
            vm.push_val(Value::Int(buf[0] as i64));
            Ok(())
        }),
        // flush -> (flushes stdout)
        ("flush", |_vm| {
            use std::io::Write;
            std::io::stdout()
                .flush()
                .map_err(|e| format!("flush failed: {}", e))?;
            Ok(())
        }),
        // term_size -> {rows, cols}
        ("term_size", |vm| {
            unsafe {
                let mut ws: libc::winsize = std::mem::zeroed();
                if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) != 0 {
                    return Err("ioctl TIOCGWINSZ failed".to_string());
                }
                vm.push_val(Value::List(vec![
                    Value::Int(ws.ws_row as i64),
                    Value::Int(ws.ws_col as i64),
                ]));
            }
            Ok(())
        }),
        // ---- Environment ----

        // "NAME" env_get -> string (or error)
        ("env_get", |vm| {
            let name = vm.pop_val()?;
            match name {
                Value::Str(n) => {
                    match std::env::var(&n) {
                        Ok(v) => vm.push_val(Value::Str(v)),
                        Err(_) => vm.push_val(Value::Str(String::new())),
                    }
                    Ok(())
                }
                _ => Err(format!(
                    "'env_get' expects string, got {}",
                    name.type_name()
                )),
            }
        }),
        // env_all -> #{"KEY" => "VAL", ...}
        ("env_all", |vm| {
            let map: HashMap<Value, Value> = std::env::vars()
                .map(|(k, v)| (Value::Str(k), Value::Str(v)))
                .collect();
            vm.push_val(Value::Map(map));
            Ok(())
        }),
        // args -> {arg0, arg1, ...}
        ("args", |vm| {
            let args: Vec<Value> = std::env::args().map(Value::Str).collect();
            vm.push_val(Value::List(args));
            Ok(())
        }),
        // JSON/YAML/TOML/Proto are registered in builtins_serial.rs

        // ---- Time ----
        ("time_ms", |vm| {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            vm.push_val(Value::Int(ms));
            Ok(())
        }),
        // sleep_ms: int -> (pauses execution)
        ("sleep_ms", |vm| {
            let val = vm.pop_val()?;
            match val {
                Value::Int(ms) => {
                    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                    Ok(())
                }
                _ => Err(format!("'sleep_ms' expects int, got {}", val.type_name())),
            }
        }),
    ]
}
