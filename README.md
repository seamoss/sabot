# Sabo

**A stack-based pattern matching language.**

Sabo is an expressive scripting language that fuses the compositional power of Forth-style stack programming with the clarity of Erlang-style pattern matching. It compiles to bytecode and runs on a purpose-built virtual machine, all implemented in Rust with zero unsafe code.

```sabo
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;

10 factorial println   -- 3628800
```

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
  - [Verifying the Installation](#verifying-the-installation)
- [Quick Start](#quick-start)
  - [Hello World](#hello-world)
  - [The REPL](#the-repl)
  - [Running Files](#running-files)
- [Language Tour](#language-tour)
  - [Stack Operations](#stack-operations)
  - [Types](#types)
  - [Word Definitions and Pattern Matching](#word-definitions-and-pattern-matching)
  - [Guards](#guards)
  - [Let Bindings](#let-bindings)
  - [Lists](#lists)
  - [Maps](#maps)
  - [Quotations (Lambdas)](#quotations-lambdas)
  - [Higher-Order Functions](#higher-order-functions)
  - [String Interpolation](#string-interpolation)
  - [Named Stacks](#named-stacks)
  - [Error Handling](#error-handling)
  - [Regular Expressions](#regular-expressions)
  - [Modules](#modules)
- [Standard Library](#standard-library)
- [Built-in Capabilities](#built-in-capabilities)
  - [File I/O](#file-io)
  - [Shell Execution](#shell-execution)
  - [HTTP Client](#http-client)
  - [HTTP Server](#http-server)
  - [SQLite Database](#sqlite-database)
  - [Serialization (JSON, YAML, TOML, Protobuf)](#serialization-json-yaml-toml-protobuf)
  - [Reactive Cells](#reactive-cells)
  - [Concurrency](#concurrency)
  - [Observability (Tracing, Metrics, Logging)](#observability-tracing-metrics-logging)
  - [Memoization](#memoization)
- [CLI Tools](#cli-tools)
  - [Test Runner](#test-runner)
  - [Formatter](#formatter)
  - [Profiler](#profiler)
  - [Benchmarks](#benchmarks)
- [Editor Support](#editor-support)
- [Example Programs](#example-programs)
- [Project Structure](#project-structure)
- [Architecture](#architecture)
- [Documentation](#documentation)
- [Dependencies](#dependencies)

---

## Features

- **Stack-based execution** -- postfix (reverse Polish) notation, no operator precedence to memorize
- **Pattern matching** -- multi-arm word definitions with literal, variable, wildcard, and list destructuring patterns
- **Guards** -- conditional constraints on pattern arms (`where n > 0 and n < 100`)
- **Tail-call optimization** -- self-recursive calls in tail position run in O(1) stack space
- **First-class quotations** -- anonymous code blocks that capture lexical scope (closures)
- **Reactive cells** -- spreadsheet-style computed values and edge-triggered watchers
- **Structured concurrency** -- spawn/await, typed channels, cancellation, timeouts, select
- **Built-in HTTP server** -- route matching, path parameters, static file serving
- **SQLite database** -- parameterized queries, type conversion, one connection per VM
- **Multi-format serialization** -- JSON, YAML, TOML, and protobuf-style binary encoding
- **Observability** -- tracing spans, counters/gauges/histograms, structured logging
- **Regular expressions** -- match, find, replace, split, captures
- **Module system** -- import files, dotted name access
- **Memoization** -- per-word result caching with automatic arity detection
- **Comprehensive tooling** -- REPL, test runner, source formatter, execution profiler, benchmarks
- **Editor support** -- VS Code syntax highlighting with interpolation, bracket matching, and all builtins

---

## Installation

### Prerequisites

Sabo is written in Rust. You need:

- **Rust toolchain** (1.85+ recommended, edition 2024)
  - Install via [rustup](https://rustup.rs/):
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
  - Or if you already have rustup:
    ```bash
    rustup update stable
    ```

- **C compiler** (for the bundled SQLite)
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `build-essential` or equivalent (`apt install build-essential`)
  - Windows: MSVC build tools (via Visual Studio installer)

### Building from Source

Clone the repository and build:

```bash
git clone https://github.com/seamoss/sabo.git
cd sabo
cargo build --release
```

The binary will be at `./target/release/sabo`.

To install it system-wide (copies to `~/.cargo/bin/`):

```bash
cargo install --path .
```

Or add the release binary to your PATH manually:

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="/path/to/sabo/target/release:$PATH"
```

### Verifying the Installation

```bash
sabo --help    # Currently just runs the REPL if no args
sabo           # Start the interactive REPL
```

You should see:

```
Sabo v0.4.0 -- stack-based pattern matching language
Type .help for commands, 'quit' to exit

sabo>
```

Type `2 3 +` and press Enter. You should see `5`. Type `quit` to exit.

---

## Quick Start

### Hello World

Create a file called `hello.sabo`:

```sabo
"Hello, World!" println
```

Run it:

```bash
sabo hello.sabo
```

### The REPL

Start the interactive REPL by running `sabo` with no arguments:

```
sabo> 2 3 + 4 *
  20
sabo> "hello" " " + "world" +
  "hello world"
sabo> : double [n] -> n 2 * ;
sabo> 21 double
  42
sabo> .stack
  42
sabo> .clear
  stack cleared
sabo> quit
```

The REPL shows the stack after each expression. Available dot-commands:

| Command    | Description              |
|------------|--------------------------|
| `.help`    | Show all commands        |
| `.stack`   | Display current stack    |
| `.words`   | List defined words       |
| `.globals` | List global variables    |
| `.cells`   | List reactive cells      |
| `.clear`   | Clear the stack          |
| `.reset`   | Reset the entire VM      |
| `quit`     | Exit the REPL            |

Multi-line input is supported. The REPL detects incomplete expressions (unclosed `:` definitions, brackets, braces) and continues prompting with `...` until the expression is complete:

```
sabo> : factorial
  ...   [0] -> 1
  ...   [n] -> n n 1 - factorial *
  ... ;
sabo> 10 factorial
  3628800
```

#### REPL Startup File (.saborc)

If a `.saborc` file exists in the current directory (or `~/.saborc`), it is automatically loaded when the REPL starts. This is useful for importing your standard library and defining aliases:

```sabo
-- .saborc example
"lib/math.sabo" import
"lib/list.sabo" import
"lib/string.sabo" import
"lib/functional.sabo" import
"lib/map.sabo" import

-- Handy REPL aliases
: pp  [x] -> x println ;
: clr [] -> depth [drop] times ;
```

### Running Files

```bash
sabo program.sabo              # Run a Sabo source file
sabo programs/todo.sabo list   # Run with arguments (access via `args` builtin)
```

---

## Language Tour

Sabo is a **postfix** language. Values are pushed onto a stack, and operations consume values from the stack and push results back. There is no operator precedence -- everything evaluates strictly left-to-right.

```sabo
3 4 + 2 *    -- (3+4)*2 = 14
3 4 2 * +    -- 3+(4*2) = 11
```

### Stack Operations

The stack is the central data structure. These are the primitive operations:

```
dup     ( a -- a a )         Duplicate the top value
drop    ( a -- )             Discard the top value
swap    ( a b -- b a )       Swap the top two values
over    ( a b -- a b a )     Copy second-from-top to top
rot     ( a b c -- b c a )   Rotate the top three values
depth   ( -- n )             Push the current stack depth
```

### Types

Sabo has seven value types:

| Type        | Examples                          | Description                          |
|-------------|-----------------------------------|--------------------------------------|
| `Int`       | `42`, `-7`, `0`                   | 64-bit signed integer                |
| `Float`     | `3.14`, `-0.5`                    | 64-bit floating point                |
| `Str`       | `"hello"`, `"line\nbreak"`        | UTF-8 string                         |
| `Symbol`    | `:ok`, `:error`, `:null`          | Interned atom (lightweight tag)      |
| `Bool`      | `true`, `false`                   | Boolean                              |
| `List`      | `{1, 2, 3}`, `{}`                | Ordered collection (mixed types OK)  |
| `Map`       | `#{"key" => "val"}`, `#{}`        | Hash map (any value as key)          |
| `Quotation` | `[2 *]`, `[dup +]`               | First-class code block               |

Check a value's type at runtime with `type_of`:

```sabo
42 type_of         -- "int"
"hello" type_of    -- "str"
{1,2,3} type_of    -- "list"
```

### Word Definitions and Pattern Matching

Words are Sabo's functions. Every word is defined with one or more pattern-matching arms. Patterns consume values from the stack and bind them to local variables:

```sabo
-- Zero arguments (matches without consuming anything)
: greet [] -> "hello!" println ;

-- Single argument
: double [n] -> n 2 * ;

-- Multiple arguments (top of stack is rightmost)
: add [a, b] -> a b + ;

-- Multi-arm with pattern matching (tried top-to-bottom, first match wins)
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;

-- Literal matching
: describe
  [0]     -> "zero"
  [1]     -> "one"
  [:ok]   -> "success"
  [true]  -> "affirmative"
  [_]     -> "something else"    -- wildcard: matches anything
;

-- List patterns
: sum
  {[]}      -> 0                 -- empty list
  {[h | t]} -> h t sum +         -- head/tail destructuring
;
```

Identifiers may end with `?` or `!` by convention (`?` for predicates, `!` for side-effects):

```sabo
: even? [n] -> n 2 % 0 == ;
: save! [data, path] -> data path write_file ;
```

**Tail-call optimization**: Self-recursive calls in the last position of an arm body are optimized to reuse the current stack frame, enabling deep recursion without stack overflow:

```sabo
: count_down
  [0] -> "done"
  [n] -> n 1 - count_down    -- tail call: O(1) stack space
;

1000000 count_down    -- works fine, no stack overflow
```

### Guards

Guards add boolean conditions to pattern arms:

```sabo
: classify
  [n] where n > 0 -> "positive"
  [n] where n < 0 -> "negative"
  [n]             -> "zero"
;

: clamp
  [n] where n < 0   -> 0
  [n] where n > 100  -> 100
  [n]                -> n
;

: in_range
  [n] where n >= 0 and n <= 100 -> true
  [_] -> false
;
```

Guard operators: `==`, `!=`, `<`, `>`, `<=`, `>=`, `and`, `or`, `not`. Guards can only reference pattern-bound variables and literals -- no word calls or stack operations.

### Let Bindings

`let` binds the top of the stack to a global name:

```sabo
42 let answer =
"hello" " " + "world" + let greeting =

answer println       -- 42
greeting println     -- "hello world"
```

Destructuring is supported for lists and maps:

```sabo
{10, 20, 30} let {a, b, c} =       -- positional: a=10, b=20, c=30
{1, 2, 3, 4} let {h | t} =         -- head/tail: h=1, t={2, 3, 4}
#{"name" => "sabo"} let #{"name" => n} =   -- map key: n="sabo"
```

### Lists

Lists are ordered collections that can contain mixed types:

```sabo
{1, 2, 3}                -- list literal
{}                        -- empty list
{1, "two", :three, true}  -- mixed types

-- Operations
{1, 2, 3} head           -- 1
{1, 2, 3} tail           -- {2, 3}
0 {1, 2, 3} cons         -- {0, 1, 2, 3}
{1, 2, 3} 4 append       -- {1, 2, 3, 4}
{1, 2, 3} 0 nth          -- 1 (zero-indexed)
{1, 2, 3} len            -- 3
{3, 1, 2} sort           -- {1, 2, 3}
{1, 2, 3} reverse        -- {3, 2, 1}
1 10 range                -- {1, 2, 3, 4, 5, 6, 7, 8, 9, 10}
```

### Maps

Maps are hash maps with O(1) lookup. Any value type can be a key:

```sabo
#{"name" => "sabo", "version" => 4}   -- map literal
#{}                                     -- empty map

-- Operations
#{"a" => 1} "a" get            -- 1
#{"a" => 1} "b" 0 get_or       -- 0 (default if key missing)
#{"a" => 1} "b" 2 set          -- #{"a" => 1, "b" => 2}
#{"a" => 1} "a" has            -- true
#{"a" => 1, "b" => 2} keys     -- {"a", "b"}
#{"a" => 1, "b" => 2} values   -- {1, 2}

-- Merge two maps (right side wins on conflict)
#{"a" => 1} #{"b" => 2, "a" => 99} merge   -- #{"a" => 99, "b" => 2}
```

### Quotations (Lambdas)

Quotations are first-class code blocks -- Sabo's equivalent of anonymous functions or lambdas. They are delimited by square brackets and executed with `~` or `apply`:

```sabo
[2 3 +] ~        -- 5 (execute the quotation)
[2 3 +] apply    -- same thing

-- Store in a variable
[dup *] let square =
7 square ~        -- 49

-- Pass as argument
{1, 2, 3} [2 *] map    -- {2, 4, 6}
```

Quotations inside word bodies capture enclosing pattern-bound variables (closures):

```sabo
: make_adder [n] -> [n +] ;

5 make_adder let add5 =
3 add5 ~    -- 8
10 add5 ~   -- 15
```

Compose quotations with `.`:

```sabo
[2 *] [1 +] .    -- creates a quotation equivalent to [2 * 1 +]
6 [2 *] [1 +] . ~   -- 13
```

### Higher-Order Functions

```sabo
-- map: apply quotation to each element
{1, 2, 3, 4} [2 *] map            -- {2, 4, 6, 8}

-- filter: keep elements where quotation returns true
{1, 2, 3, 4, 5} [2 % 0 ==] filter -- {2, 4}

-- fold: reduce a list (list init quotation)
{1, 2, 3, 4} 0 [+] fold           -- 10

-- each: execute quotation for side effects
{1, 2, 3} [println] each          -- prints 1, 2, 3

-- sort with custom comparator
{3, 1, 4, 1, 5} [<] sort          -- {1, 1, 3, 4, 5}
```

### String Interpolation

Variables inside `{}` in strings are automatically expanded:

```sabo
let name = "world"
let n = 42
"hello {name}, the answer is {n}!" println
-- hello world, the answer is 42!
```

Use `\{` for a literal brace. Only simple variable names are supported inside `{}` -- no expressions.

Escape sequences: `\n` (newline), `\t` (tab), `\\` (backslash), `\"` (quote), `\{` (literal brace).

### Named Stacks

Named stacks are secondary stacks for temporary storage:

```sabo
42 >temp       -- push 42 onto named stack "temp"
"hi" >temp     -- push "hi" onto "temp"
temp>          -- pop from "temp" -> "hi"
temp>          -- pop from "temp" -> 42
```

Syntax: `>name` (push), `name>` (pop). Any alphanumeric name works. Useful for keeping the main stack clean during complex computations.

### Error Handling

`try` wraps an expression and catches errors:

```sabo
[1 0 /] try          -- {:error, "Division by zero"}
[42] try             -- {:ok, 42}

-- Throw custom errors
[:not_found throw] try            -- {:error, :not_found}

-- Structured errors
["file missing" :io error] try    -- {:error, {:error, :io, "file missing"}}
```

Runtime errors include source location:

```
[line 4] in 'my_word': Unknown word 'foo_bar'
[line 11] Cannot apply '+' to string and int
```

### Regular Expressions

Built on Rust's `regex` crate:

```sabo
"hello123" "\\d+" regex_match?           -- true
"hello 42 world" "\\d+" regex_find       -- {:ok, "42"}
"a1 b2 c3" "\\d+" regex_find_all        -- {"1", "2", "3"}
"foo  bar" "\\s+" " " regex_replace      -- "foo bar"
"a,b,,c" "," regex_split                -- {"a", "b", "", "c"}
"2026-03-06" "(\\d+)-(\\d+)-(\\d+)" regex_captures
-- {"2026-03-06", "2026", "03", "06"}
```

### Modules

Import other Sabo files as modules:

```sabo
"lib/math.sabo" import

5 math.square    -- 25 (dotted access)
5 square         -- 25 (unqualified also works)
```

All words defined in the imported file become available, both with and without the module prefix.

---

## Standard Library

Sabo ships with a standard library written in Sabo itself, located in the `lib/` directory:

| Module               | Contents                                                                        |
|----------------------|---------------------------------------------------------------------------------|
| `lib/math.sabo`      | `abs`, `negate`, `sign`, `min`, `max`, `clamp`, `pow`, `gcd`, `lcm`, `sum`, `product`, `average`, `factorial`, `fib`, `is_even`, `is_odd`, `is_divisible`, `square`, `cube` |
| `lib/list.sabo`      | `take`, `drop_n`, `concat`, `flatten`, `zip`, `find`, `any_match`, `all_match`, `unique`, `last`, `init`, `repeat`, `index_of`, `sum_list`, `prod_list`, `chunk` |
| `lib/string.sabo`    | `str_repeat`, `pad_left`, `pad_right`, `center`, `split_words`, `lines`, `join_words`, `unlines`, `is_empty_str`, `is_blank`, `surround`, `quote`, `parens`, `brackets`, `braces` |
| `lib/functional.sabo` | `identity`, `const`, `flip`, `times`, `compose`, `when_true`, `when_false`, `pipe`, `iterate`, `until` |
| `lib/map.sabo`       | `is_empty_map`, `map_size`                                                      |

Import them in your scripts:

```sabo
"lib/math.sabo" import
"lib/list.sabo" import

25 square           -- 625
{1, 2, 3, 4} 2 take -- {1, 2}
```

Or load them all in your `.saborc` for REPL use.

---

## Built-in Capabilities

Sabo has an extensive set of builtins that require no imports. Everything below is available out of the box.

### File I/O

```sabo
"config.txt" read_file                -- read entire file as string
"data.csv" read_lines                 -- read file as list of lines
"hello" "out.txt" write_file          -- write string to file (creates/overwrites)
"\nnew line" "out.txt" append_file    -- append to file
"out.txt" file_exists                 -- true/false
"." ls                                -- list directory entries
```

### Shell Execution

```sabo
"ls -la" exec                         -- execute shell command, return stdout
"grep -c error log.txt" exec_full     -- returns #{"stdout" => ..., "stderr" => ..., "code" => 0}
```

### HTTP Client

```sabo
"https://httpbin.org/get" http_get    -- GET request, returns response body
"body" "https://httpbin.org/post" http_post   -- POST with body
```

### HTTP Server

Sabo includes a built-in threaded HTTP/1.1 server:

```sabo
-- Define route handlers (each receives a request map, returns a response map)
: handle_hello
  [req] -> #{"status" => 200, "body" => "Hello, World!"}
;

: handle_greet
  [req] ->
    let name = req "params" get "name" get
    #{"status" => 200, "body" => "Hello, " name +}
;

-- Register routes (supports path parameters with :param syntax)
[handle_hello] "/hello" "GET" route
[handle_greet] "/greet/:name" "GET" route

-- Serve static files from a directory
"public" "/static" static_dir

-- Start the server
8080 serve
```

Request maps contain: `method`, `path`, `query` (map), `params` (map from path params), `headers` (map), `body` (string).

### SQLite Database

Built-in SQLite via `rusqlite` (bundled, no external install required):

```sabo
-- Open a database (file or :memory:)
"app.db" db_open

-- Execute statements
"CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INT)" db_exec drop

-- Parameterized queries (safe from SQL injection)
"INSERT INTO users (name, age) VALUES (?, ?)" {"Alice", 30} db_exec_params drop
"INSERT INTO users (name, age) VALUES (?, ?)" {"Bob", 25} db_exec_params drop

-- Query (returns list of maps)
"SELECT * FROM users" db_query
-- {#{"id" => 1, "name" => "Alice", "age" => 30}, #{"id" => 2, "name" => "Bob", "age" => 25}}

"SELECT * FROM users WHERE age > ?" {28} db_query_params
-- {#{"id" => 1, "name" => "Alice", "age" => 30}}

db_close
```

### Serialization (JSON, YAML, TOML, Protobuf)

Multi-format serialization with automatic type mapping between Sabo values and text/binary formats:

```sabo
-- JSON
#{"name" => "sabo", "version" => 4} json_encode    -- compact JSON string
#{"name" => "sabo"} json_pretty                     -- pretty-printed JSON
"{\"name\": \"sabo\"}" json_parse                   -- parse JSON to Sabo values

-- YAML
#{"color" => "red", "count" => 5} yaml_encode       -- YAML string
"name: sabo\nversion: 4" yaml_parse                 -- parse YAML

-- TOML
#{"key" => "value", "num" => 42} toml_encode        -- TOML string
"name = \"sabo\"\nversion = 4" toml_parse            -- parse TOML

-- Binary (protobuf-style TLV encoding)
#{"data" => {1, 2, 3}} proto_encode                 -- encode to list of bytes
proto_decode                                         -- decode back to Sabo values
```

All text formats use serde under the hood. The binary format is a custom self-describing TLV (type-length-value) encoding that roundtrips all Sabo value types. See [LANGUAGE.md](docs/LANGUAGE.md#serialization-json-yaml-toml-protobuf) for the full type mapping and wire format spec.

### Reactive Cells

Spreadsheet-style reactive programming. Cells hold values; computed cells recompute automatically when dependencies change; watchers fire on edge transitions:

```sabo
-- Create cells
100 "price" cell
0.08 "tax_rate" cell

-- Computed cell: auto-recomputes when price or tax_rate changes
["price" get_cell "tax_rate" get_cell *] "tax" computed
["price" get_cell "tax" get_cell +] "total" computed

"total" get_cell println    -- 108.0

-- Update price -> tax and total recompute automatically
200 "price" set_cell
"total" get_cell println    -- 216.0

-- Edge-triggered watcher (fires once on false->true transition)
["total" get_cell 500 >] ["Warning: expensive!" println] when
```

### Concurrency

Sabo supports structured concurrency with spawn/await, typed channels, cancellation, timeouts, and select:

```sabo
-- Spawn tasks
[42 2 *] spawn let t1 =
["hello" " world" +] spawn let t2 =

t1 await    -- {:ok, 84}
t2 await    -- {:ok, "hello world"}

-- Typed channels
channel let ch =
[100 ch ch_send] spawn drop
ch ch_recv println    -- 100
ch ch_close

-- Await multiple tasks
{t1, t2, t3} await_all    -- list of {:ok, val} or {:error, msg}
{t1, t2} await_any         -- {winning_task_id, {:ok, val}}

-- Cancellation
task_id cancel        -- signal cancellation
cancelled?            -- check if current task is cancelled

-- Timeouts
[long_computation] 1000 timeout    -- {:ok, result} or {:error, :timeout}

-- Select (wait on multiple channels)
{ch1, ch2, ch3} select    -- {channel_id, value}
```

See [LANGUAGE.md](docs/LANGUAGE.md#concurrency) for the full concurrency documentation.

### Observability (Tracing, Metrics, Logging)

Built-in OpenTelemetry-style observability with zero external dependencies:

```sabo
-- Tracing: nested spans with attributes and events
"http-request" [
  "http.method" "GET" span_attr
  "http.url" "/api/users" span_attr
  do_work
  "cache-hit" span_event
] span

-- Metrics: counters, gauges, histograms
"http.requests" counter_inc
42 "active.connections" gauge_set
123.4 "response.time_ms" histogram_record

-- Structured logging with levels and fields
"Server started" log_info
"Request failed" #{"status" => 500, "path" => "/api"} :error log_with
:warn log_level    -- suppress debug/info output

-- Export everything
otel_dump    -- #{"spans" => [...], "metrics" => #{...}, "logs" => [...]}
```

Logs print to stderr with timestamps. All telemetry data is queryable as Sabo values. See [LANGUAGE.md](docs/LANGUAGE.md#observability-tracing-metrics-logging) for the full API.

### Memoization

Cache the results of any word by name:

```sabo
: fib
  [0] -> 0
  [1] -> 1
  [n] -> n 1 - fib n 2 - fib +
;

"fib" memo        -- enable caching for 'fib'
35 fib            -- computed once (fast)
35 fib            -- instant cache hit
"fib" memo_clear  -- clear the cache
```

Arity is auto-detected from the word's pattern. Cache keys are built from the top N stack values.

---

## CLI Tools

### Test Runner

Sabo has a built-in test runner with assertions:

```sabo
-- test_math.sabo
5 factorial 120 assert_eq
-3 abs 3 assert_eq
0 is_even true assert_eq
"Math tests passed!" println
```

Run tests:

```bash
sabo test lib/                      # Run all test files in a directory
sabo test lib/test_stdlib.sabo      # Run a single test file
```

The test runner looks for files matching `*test*` in the directory. Each file runs independently. Results are reported with pass/fail counts and timing:

```
Running 7 test file(s) in lib/

  PASS  lib/test_concurrency.sabo
  PASS  lib/test_destructure.sabo
  PASS  lib/test_errors.sabo
  PASS  lib/test_otel.sabo
  PASS  lib/test_regex.sabo
  PASS  lib/test_serial.sabo
  PASS  lib/test_stdlib.sabo

--- Results ---
Passed: 7
Failed: 0
Time:   77ms
```

Available assertions:

| Word         | Stack Effect         | Description                    |
|--------------|----------------------|--------------------------------|
| `assert`     | `( bool -- )`       | Fail if not true               |
| `assert_eq`  | `( a b -- )`        | Fail if a != b                 |
| `assert_neq` | `( a b -- )`        | Fail if a == b                 |

### Formatter

Format Sabo source code with consistent style:

```bash
sabo fmt program.sabo          # Print formatted output to stdout
sabo fmt -w program.sabo       # Format in place
sabo fmt -w *.sabo             # Format multiple files
```

The formatter normalizes indentation, collapses blank lines, and formats word definitions with consistent arm alignment. Single-arm words that fit on one line are kept compact; multi-arm words get indented. Comments are preserved. The formatter is idempotent -- running it twice produces the same output.

### Profiler

Run any Sabo program with execution profiling:

```bash
sabo profile program.sabo
```

The profiler reports per-word call counts, total time, self time (excluding time in callees), and average time per call. It also highlights the hottest words by self time. Zero overhead when not enabled.

```
=== Profiler Report ===
Word                  Calls     Total(ms)  Self(ms)   Avg(ms)
fib                   242,785   1,203      1,203      0.005
factorial             100       12         12         0.120
...

--- Top 5 by self time ---
  fib              1,203ms
  ...
```

### Benchmarks

A benchmark suite is included at `tools/bench.sabo`:

```bash
sabo tools/bench.sabo
```

Covers compute (fibonacci, factorial), collections (list building, sorting), strings (concatenation, splitting), regex, concurrency, and map operations.

---

## Editor Support

### VS Code

A syntax highlighting extension is included in `editors/vscode/`:

```bash
# Option 1: Symlink (recommended for development)
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/sabo-lang

# Option 2: Copy
cp -r editors/vscode ~/.vscode/extensions/sabo-lang
```

Reload VS Code (`Cmd+Shift+P` -> "Reload Window").

Features:
- Full syntax highlighting for `.sabo` files
- Comment toggling (`Cmd+/`)
- Bracket matching and auto-closing for `[]`, `{}`, `()`
- String interpolation highlighting (`{variable}` inside strings)
- Distinct colors for: comments, strings, numbers, symbols, keywords, word definitions, named stacks, all builtin categories, operators

---

## Example Programs

The repository includes several complete example programs demonstrating Sabo's capabilities:

### `programs/wordcount.sabo` -- Word Count Utility
A `wc`-like tool that counts lines, words, characters, and unique words. Displays the top 10 most frequent words with frequency counts.

```bash
sabo programs/wordcount.sabo README.md
```

### `programs/todo.sabo` -- SQLite Todo App
A CLI task manager backed by SQLite. Supports add, list, done, undo, remove, clear, and stats commands.

```bash
sabo programs/todo.sabo add "Write documentation"
sabo programs/todo.sabo list
sabo programs/todo.sabo done 1
sabo programs/todo.sabo stats
```

### `programs/budget.sabo` -- Reactive Budget Tracker
An interactive budget tracker using reactive cells. Income and expenses are cells; totals and balance recompute automatically. Includes an overspend warning via edge-triggered watcher.

```bash
sabo programs/budget.sabo
```

### `programs/server.sabo` -- HTTP Server
A demo web server with 8 routes, static file serving, REST API endpoints, and JSON responses. Demonstrates the built-in HTTP server with path parameters and content negotiation.

```bash
sabo programs/server.sabo
# Visit http://localhost:8080
```

### `programs/github_stats.sabo` -- GitHub Profile Fetcher
Fetches GitHub user data via the API, parses JSON, and displays profile statistics.

```bash
sabo programs/github_stats.sabo
```

### `games/the_vault.sabo` -- Text Adventure
A fully playable text adventure game with 6 rooms, inventory system, puzzle mechanics, reactive game state, and pattern-matched command parsing. Demonstrates reactive cells, maps, pattern matching, and interactive I/O working together.

```bash
sabo games/the_vault.sabo
```

### Example Scripts (examples/)

| File                            | Demonstrates                                          |
|---------------------------------|-------------------------------------------------------|
| `examples/hello.sabo`           | Stack basics, word definitions, pattern matching      |
| `examples/lists.sabo`           | List patterns, recursive sum, fibonacci               |
| `examples/tier1.sabo`           | Let bindings, string interpolation, maps, try/error   |
| `examples/tier2.sabo`           | File I/O, HTTP requests, JSON parsing                 |
| `examples/tier3_reactive.sabo`  | Reactive cells, computed values, watchers             |
| `examples/tier3_concurrency.sabo` | Spawn/await, channels, concurrent tasks             |
| `examples/tier3_stacktrace.sabo` | Stack traces, error context, hot reload              |
| `examples/tier4_tests.sabo`     | Full test suite using assert/assert_eq                |
| `examples/tier4_modules.sabo`   | Module imports, dotted name access                    |

---

## Project Structure

```
sabo/
├── Cargo.toml                  # Rust project manifest
├── .saborc                     # REPL startup file
│
├── src/                        # Rust source (the implementation)
│   ├── main.rs                 # Entry point: REPL, file runner, CLI commands
│   ├── token.rs                # Token types
│   ├── lexer.rs                # Tokenizer (source text -> tokens)
│   ├── ast.rs                  # Abstract syntax tree nodes
│   ├── parser.rs               # Parser (tokens -> AST)
│   ├── opcode.rs               # Bytecode instruction set
│   ├── compiler.rs             # Compiler (AST -> bytecode)
│   ├── value.rs                # Runtime value types
│   ├── vm.rs                   # Virtual machine (bytecode interpreter)
│   ├── builtins_io.rs          # File I/O, shell, HTTP client, env, time
│   ├── builtins_http.rs        # HTTP server (routing, request handling)
│   ├── builtins_db.rs          # SQLite database
│   ├── builtins_serial.rs      # JSON, YAML, TOML, protobuf serialization
│   ├── builtins_otel.rs        # Tracing, metrics, structured logging
│   ├── formatter.rs            # Source code formatter
│   └── profiler.rs             # Execution profiler
│
├── lib/                        # Standard library (written in Sabo)
│   ├── math.sabo               # Math utilities
│   ├── list.sabo               # List utilities
│   ├── string.sabo             # String utilities
│   ├── functional.sabo         # Functional programming utilities
│   ├── map.sabo                # Map utilities
│   ├── test_stdlib.sabo        # Standard library tests
│   ├── test_regex.sabo         # Regex tests
│   ├── test_errors.sabo        # Error handling tests
│   ├── test_destructure.sabo   # Destructuring tests
│   ├── test_concurrency.sabo   # Concurrency tests
│   ├── test_serial.sabo        # Serialization tests
│   └── test_otel.sabo          # Observability tests
│
├── examples/                   # Example scripts
├── programs/                   # Complete programs
│   └── public/                 # Static files for the server demo
├── games/                      # Games
├── tools/                      # Development tools
│   ├── bench.sabo              # Benchmark suite
│   └── run_tests.sabo          # Test runner
│
├── editors/                    # Editor integrations
│   └── vscode/                 # VS Code extension
│       ├── package.json
│       ├── language-configuration.json
│       └── syntaxes/sabo.tmLanguage.json
│
└── docs/                       # Documentation
    ├── LANGUAGE.md             # Comprehensive language reference
    └── SYNTAX.md               # Quick syntax reference card
```

---

## Architecture

Sabo follows a classic compilation pipeline:

```
Source Code (.sabo)
       │
       ▼
   ┌────────┐
   │ Lexer  │   Tokenizes source text into a token stream.
   └───┬────┘   Handles string interpolation, multi-line detection, dotted identifiers.
       │
       ▼
   ┌────────┐
   │ Parser │   Builds an abstract syntax tree from tokens.
   └───┬────┘   Parses word definitions, patterns, guards, let bindings, expressions.
       │
       ▼
   ┌──────────┐
   │ Compiler │  Compiles AST to bytecode (Op enum).
   └───┬──────┘  Handles pattern match compilation, tail-call detection, closure capture.
       │
       ▼
   ┌─────┐
   │ VM  │      Executes bytecode on an operand stack with call frames.
   └─────┘      Manages stack, globals, cells, channels, builtins, and all I/O.
```

The VM is a stack machine with:
- **Operand stack** -- the main data stack
- **Call frames** -- each with local variables, instruction pointer, and code block
- **Global state** -- let bindings, reactive cells, channels, task registry, memoization cache
- **Builtin registry** -- native Rust functions callable from Sabo

All concurrency uses Rust's `std::thread` with `Arc<Mutex<...>>` for shared state. Each spawned task gets its own VM (via `spawn_child`) that shares words, builtins, globals, channels, and routes, but has an independent stack and reactive cell state.

---

## Documentation

| Document | Description |
|----------|-------------|
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | **Comprehensive language reference.** Every builtin, every type, every feature -- with examples. Includes VM internals, compilation pipeline details, and architecture overview. |
| [docs/SYNTAX.md](docs/SYNTAX.md) | **Quick syntax reference card.** Grammar and examples for every construct, no prose. Useful as a cheat sheet while coding. |

---

## Dependencies

Sabo uses a minimal set of Rust crates:

| Crate        | Version | Purpose                                      |
|--------------|---------|----------------------------------------------|
| `ureq`       | 3       | HTTP client (GET/POST)                       |
| `rusqlite`   | 0.31    | SQLite database (bundled, no system install)  |
| `regex`      | 1       | Regular expression support                   |
| `serde_json` | 1       | JSON serialization/deserialization            |
| `serde_yaml` | 0.9     | YAML serialization/deserialization            |
| `toml`       | 0.8     | TOML serialization/deserialization            |

Everything else -- the lexer, parser, compiler, VM, reactive system, concurrency, binary encoding, HTTP server, formatter, profiler, and test runner -- is implemented from scratch.
