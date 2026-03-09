# Sabot

**A stack-based pattern matching language.**

Sabot is an expressive scripting language that fuses the compositional power of Forth-style stack programming with the clarity of Erlang-style pattern matching. It compiles to bytecode and runs on a purpose-built virtual machine, all implemented in Rust with zero unsafe code.

```sabot
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
- [Development Workflow](#development-workflow)
  - [Branching Strategy](#branching-strategy)
  - [Version Management](#version-management)
  - [Makefile Targets](#makefile-targets)
  - [Release Pipeline](#release-pipeline)
- [Dependencies](#dependencies)
- [License](#license)

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

Sabot is written in Rust. You need:

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
git clone https://github.com/seamoss/sabot.git
cd sabot
cargo build --release
```

The binary will be at `./target/release/sabot`.

To install it system-wide (copies to `~/.cargo/bin/`):

```bash
cargo install --path .
```

Or add the release binary to your PATH manually:

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="/path/to/sabot/target/release:$PATH"
```

### Verifying the Installation

```bash
sabot --help    # Currently just runs the REPL if no args
sabot           # Start the interactive REPL
```

You should see:

```
Sabot v0.4.1 -- stack-based pattern matching language
Type .help for commands, 'quit' to exit

sabot>
```

Type `2 3 +` and press Enter. You should see `5`. Type `quit` to exit.

---

## Quick Start

### Hello World

Create a file called `hello.sabot`:

```sabot
"Hello, World!" println
```

Run it:

```bash
sabot hello.sabot
```

### The REPL

Start the interactive REPL by running `sabot` with no arguments:

```
sabot> 2 3 + 4 *
  20
sabot> "hello" " " + "world" +
  "hello world"
sabot> : double [n] -> n 2 * ;
sabot> 21 double
  42
sabot> .stack
  42
sabot> .clear
  stack cleared
sabot> quit
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
sabot> : factorial
  ...   [0] -> 1
  ...   [n] -> n n 1 - factorial *
  ... ;
sabot> 10 factorial
  3628800
```

#### REPL Startup File (.sabotrc)

If a `.sabotrc` file exists in the current directory (or `~/.sabotrc`), it is automatically loaded when the REPL starts. This is useful for importing your standard library and defining aliases:

```sabot
-- .sabotrc example
"lib/math.sabot" import
"lib/list.sabot" import
"lib/string.sabot" import
"lib/functional.sabot" import
"lib/map.sabot" import

-- Handy REPL aliases
: pp  [x] -> x println ;
: clr [] -> depth [drop] times ;
```

### Running Files

```bash
sabot program.sabot              # Run a Sabot source file
sabot programs/todo.sabot list   # Run with arguments (access via `args` builtin)
```

---

## Language Tour

Sabot is a **postfix** language. Values are pushed onto a stack, and operations consume values from the stack and push results back. There is no operator precedence -- everything evaluates strictly left-to-right.

```sabot
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

Sabot has seven value types:

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

```sabot
42 type_of         -- "int"
"hello" type_of    -- "str"
{1,2,3} type_of    -- "list"
```

### Word Definitions and Pattern Matching

Words are Sabot's functions. Every word is defined with one or more pattern-matching arms. Patterns consume values from the stack and bind them to local variables:

```sabot
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

```sabot
: even? [n] -> n 2 % 0 == ;
: save! [data, path] -> data path write_file ;
```

**Tail-call optimization**: Self-recursive calls in the last position of an arm body are optimized to reuse the current stack frame, enabling deep recursion without stack overflow:

```sabot
: count_down
  [0] -> "done"
  [n] -> n 1 - count_down    -- tail call: O(1) stack space
;

1000000 count_down    -- works fine, no stack overflow
```

### Guards

Guards add boolean conditions to pattern arms:

```sabot
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

```sabot
42 let answer =
"hello" " " + "world" + let greeting =

answer println       -- 42
greeting println     -- "hello world"
```

Destructuring is supported for lists and maps:

```sabot
{10, 20, 30} let {a, b, c} =       -- positional: a=10, b=20, c=30
{1, 2, 3, 4} let {h | t} =         -- head/tail: h=1, t={2, 3, 4}
#{"name" => "sabot"} let #{"name" => n} =   -- map key: n="sabot"
```

### Lists

Lists are ordered collections that can contain mixed types:

```sabot
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

```sabot
#{"name" => "sabot", "version" => 4}   -- map literal
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

Quotations are first-class code blocks -- Sabot's equivalent of anonymous functions or lambdas. They are delimited by square brackets and executed with `~` or `apply`:

```sabot
[2 3 +] ~        -- 5 (execute the quotation)
[2 3 +] apply    -- same thing

-- Store in a variable
[dup *] let square =
7 square ~        -- 49

-- Pass as argument
{1, 2, 3} [2 *] map    -- {2, 4, 6}
```

Quotations inside word bodies capture enclosing pattern-bound variables (closures):

```sabot
: make_adder [n] -> [n +] ;

5 make_adder let add5 =
3 add5 ~    -- 8
10 add5 ~   -- 15
```

Compose quotations with `.`:

```sabot
[2 *] [1 +] .    -- creates a quotation equivalent to [2 * 1 +]
6 [2 *] [1 +] . ~   -- 13
```

### Higher-Order Functions

```sabot
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

```sabot
let name = "world"
let n = 42
"hello {name}, the answer is {n}!" println
-- hello world, the answer is 42!
```

Use `\{` for a literal brace. Only simple variable names are supported inside `{}` -- no expressions.

Escape sequences: `\n` (newline), `\t` (tab), `\\` (backslash), `\"` (quote), `\{` (literal brace).

### Named Stacks

Named stacks are secondary stacks for temporary storage:

```sabot
42 >temp       -- push 42 onto named stack "temp"
"hi" >temp     -- push "hi" onto "temp"
temp>          -- pop from "temp" -> "hi"
temp>          -- pop from "temp" -> 42
```

Syntax: `>name` (push), `name>` (pop). Any alphanumeric name works. Useful for keeping the main stack clean during complex computations.

### Error Handling

`try` wraps an expression and catches errors:

```sabot
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

```sabot
"hello123" "\\d+" regex_match?           -- true
"hello 42 world" "\\d+" regex_find       -- {:ok, "42"}
"a1 b2 c3" "\\d+" regex_find_all        -- {"1", "2", "3"}
"foo  bar" "\\s+" " " regex_replace      -- "foo bar"
"a,b,,c" "," regex_split                -- {"a", "b", "", "c"}
"2026-03-06" "(\\d+)-(\\d+)-(\\d+)" regex_captures
-- {"2026-03-06", "2026", "03", "06"}
```

### Modules

Import other Sabot files as modules:

```sabot
"lib/math.sabot" import

5 math.square    -- 25 (dotted access)
5 square         -- 25 (unqualified also works)
```

All words defined in the imported file become available, both with and without the module prefix.

---

## Standard Library

Sabot ships with a standard library written in Sabot itself, located in the `lib/` directory:

| Module               | Contents                                                                        |
|----------------------|---------------------------------------------------------------------------------|
| `lib/math.sabot`      | `abs`, `negate`, `sign`, `min`, `max`, `clamp`, `pow`, `gcd`, `lcm`, `sum`, `product`, `average`, `factorial`, `fib`, `is_even`, `is_odd`, `is_divisible`, `square`, `cube` |
| `lib/list.sabot`      | `take`, `drop_n`, `concat`, `flatten`, `zip`, `find`, `any_match`, `all_match`, `unique`, `last`, `init`, `repeat`, `index_of`, `sum_list`, `prod_list`, `chunk` |
| `lib/string.sabot`    | `str_repeat`, `pad_left`, `pad_right`, `center`, `split_words`, `lines`, `join_words`, `unlines`, `is_empty_str`, `is_blank`, `surround`, `quote`, `parens`, `brackets`, `braces` |
| `lib/functional.sabot` | `identity`, `const`, `flip`, `times`, `compose`, `when_true`, `when_false`, `pipe`, `iterate`, `until` |
| `lib/map.sabot`       | `is_empty_map`, `map_size`                                                      |

Import them in your scripts:

```sabot
"lib/math.sabot" import
"lib/list.sabot" import

25 square           -- 625
{1, 2, 3, 4} 2 take -- {1, 2}
```

Or load them all in your `.sabotrc` for REPL use.

---

## Built-in Capabilities

Sabot has an extensive set of builtins that require no imports. Everything below is available out of the box.

### File I/O

```sabot
"config.txt" read_file                -- read entire file as string
"data.csv" read_lines                 -- read file as list of lines
"hello" "out.txt" write_file          -- write string to file (creates/overwrites)
"\nnew line" "out.txt" append_file    -- append to file
"out.txt" file_exists                 -- true/false
"." ls                                -- list directory entries
```

### Shell Execution

```sabot
"ls -la" exec                         -- execute shell command, return stdout
"grep -c error log.txt" exec_full     -- returns #{"stdout" => ..., "stderr" => ..., "code" => 0}
```

### HTTP Client

```sabot
"https://httpbin.org/get" http_get    -- GET request, returns response body
"body" "https://httpbin.org/post" http_post   -- POST with body
```

### HTTP Server

Sabot includes a built-in threaded HTTP/1.1 server:

```sabot
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

```sabot
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

Multi-format serialization with automatic type mapping between Sabot values and text/binary formats:

```sabot
-- JSON
#{"name" => "sabot", "version" => 4} json_encode    -- compact JSON string
#{"name" => "sabot"} json_pretty                     -- pretty-printed JSON
"{\"name\": \"sabot\"}" json_parse                   -- parse JSON to Sabot values

-- YAML
#{"color" => "red", "count" => 5} yaml_encode       -- YAML string
"name: sabot\nversion: 4" yaml_parse                 -- parse YAML

-- TOML
#{"key" => "value", "num" => 42} toml_encode        -- TOML string
"name = \"sabot\"\nversion = 4" toml_parse            -- parse TOML

-- Binary (protobuf-style TLV encoding)
#{"data" => {1, 2, 3}} proto_encode                 -- encode to list of bytes
proto_decode                                         -- decode back to Sabot values
```

All text formats use serde under the hood. The binary format is a custom self-describing TLV (type-length-value) encoding that roundtrips all Sabot value types. See [LANGUAGE.md](docs/LANGUAGE.md#serialization-json-yaml-toml-protobuf) for the full type mapping and wire format spec.

### Reactive Cells

Spreadsheet-style reactive programming. Cells hold values; computed cells recompute automatically when dependencies change; watchers fire on edge transitions:

```sabot
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

Sabot supports structured concurrency with spawn/await, typed channels, cancellation, timeouts, and select:

```sabot
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

```sabot
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

Logs print to stderr with timestamps. All telemetry data is queryable as Sabot values. See [LANGUAGE.md](docs/LANGUAGE.md#observability-tracing-metrics-logging) for the full API.

### Memoization

Cache the results of any word by name:

```sabot
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

Sabot has a built-in test runner with assertions:

```sabot
-- test_math.sabot
5 factorial 120 assert_eq
-3 abs 3 assert_eq
0 is_even true assert_eq
"Math tests passed!" println
```

Run tests:

```bash
sabot test lib/                      # Run all test files in a directory
sabot test lib/test_stdlib.sabot      # Run a single test file
```

The test runner looks for files matching `*test*` in the directory. Each file runs independently. Results are reported with pass/fail counts and timing:

```
Running 7 test file(s) in lib/

  PASS  lib/test_concurrency.sabot
  PASS  lib/test_destructure.sabot
  PASS  lib/test_errors.sabot
  PASS  lib/test_otel.sabot
  PASS  lib/test_regex.sabot
  PASS  lib/test_serial.sabot
  PASS  lib/test_stdlib.sabot

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

Format Sabot source code with consistent style:

```bash
sabot fmt program.sabot          # Print formatted output to stdout
sabot fmt -w program.sabot       # Format in place
sabot fmt -w *.sabot             # Format multiple files
```

The formatter normalizes indentation, collapses blank lines, and formats word definitions with consistent arm alignment. Single-arm words that fit on one line are kept compact; multi-arm words get indented. Comments are preserved. The formatter is idempotent -- running it twice produces the same output.

### Profiler

Run any Sabot program with execution profiling:

```bash
sabot profile program.sabot
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

A benchmark suite is included at `tools/bench.sabot`:

```bash
sabot tools/bench.sabot
```

Covers compute (fibonacci, factorial), collections (list building, sorting), strings (concatenation, splitting), regex, concurrency, and map operations.

---

## Editor Support

### VS Code

A syntax highlighting extension is included in `editors/vscode/`:

```bash
# Option 1: Symlink (recommended for development)
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/sabot-lang

# Option 2: Copy
cp -r editors/vscode ~/.vscode/extensions/sabot-lang
```

Reload VS Code (`Cmd+Shift+P` -> "Reload Window").

Features:
- Full syntax highlighting for `.sabot` files
- Comment toggling (`Cmd+/`)
- Bracket matching and auto-closing for `[]`, `{}`, `()`
- String interpolation highlighting (`{variable}` inside strings)
- Distinct colors for: comments, strings, numbers, symbols, keywords, word definitions, named stacks, all builtin categories, operators

---

## Example Programs

The repository includes several complete example programs demonstrating Sabot's capabilities:

### `programs/wordcount.sabot` -- Word Count Utility
A `wc`-like tool that counts lines, words, characters, and unique words. Displays the top 10 most frequent words with frequency counts.

```bash
sabot programs/wordcount.sabot README.md
```

### `programs/todo.sabot` -- SQLite Todo App
A CLI task manager backed by SQLite. Supports add, list, done, undo, remove, clear, and stats commands.

```bash
sabot programs/todo.sabot add "Write documentation"
sabot programs/todo.sabot list
sabot programs/todo.sabot done 1
sabot programs/todo.sabot stats
```

### `programs/budget.sabot` -- Reactive Budget Tracker
An interactive budget tracker using reactive cells. Income and expenses are cells; totals and balance recompute automatically. Includes an overspend warning via edge-triggered watcher.

```bash
sabot programs/budget.sabot
```

### `programs/server.sabot` -- HTTP Server
A demo web server with 8 routes, static file serving, REST API endpoints, and JSON responses. Demonstrates the built-in HTTP server with path parameters and content negotiation.

```bash
sabot programs/server.sabot
# Visit http://localhost:8080
```

### `programs/github_stats.sabot` -- GitHub Profile Fetcher
Fetches GitHub user data via the API, parses JSON, and displays profile statistics.

```bash
sabot programs/github_stats.sabot
```

### `games/the_vault.sabot` -- Text Adventure
A fully playable text adventure game with 6 rooms, inventory system, puzzle mechanics, reactive game state, and pattern-matched command parsing. Demonstrates reactive cells, maps, pattern matching, and interactive I/O working together.

```bash
sabot games/the_vault.sabot
```

### Example Scripts (examples/)

| File                            | Demonstrates                                          |
|---------------------------------|-------------------------------------------------------|
| `examples/hello.sabot`           | Stack basics, word definitions, pattern matching      |
| `examples/lists.sabot`           | List patterns, recursive sum, fibonacci               |
| `examples/tier1.sabot`           | Let bindings, string interpolation, maps, try/error   |
| `examples/tier2.sabot`           | File I/O, HTTP requests, JSON parsing                 |
| `examples/tier3_reactive.sabot`  | Reactive cells, computed values, watchers             |
| `examples/tier3_concurrency.sabot` | Spawn/await, channels, concurrent tasks             |
| `examples/tier3_stacktrace.sabot` | Stack traces, error context, hot reload              |
| `examples/tier4_tests.sabot`     | Full test suite using assert/assert_eq                |
| `examples/tier4_modules.sabot`   | Module imports, dotted name access                    |

---

## Project Structure

```
sabot/
├── Cargo.toml                  # Rust project manifest
├── .sabotrc                     # REPL startup file
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
├── lib/                        # Standard library (written in Sabot)
│   ├── math.sabot               # Math utilities
│   ├── list.sabot               # List utilities
│   ├── string.sabot             # String utilities
│   ├── functional.sabot         # Functional programming utilities
│   ├── map.sabot                # Map utilities
│   ├── test_stdlib.sabot        # Standard library tests
│   ├── test_regex.sabot         # Regex tests
│   ├── test_errors.sabot        # Error handling tests
│   ├── test_destructure.sabot   # Destructuring tests
│   ├── test_concurrency.sabot   # Concurrency tests
│   ├── test_serial.sabot        # Serialization tests
│   └── test_otel.sabot          # Observability tests
│
├── examples/                   # Example scripts
├── programs/                   # Complete programs
│   └── public/                 # Static files for the server demo
├── games/                      # Games
├── tools/                      # Development tools
│   ├── bench.sabot              # Benchmark suite
│   └── run_tests.sabot          # Test runner
│
├── editors/                    # Editor integrations
│   └── vscode/                 # VS Code extension
│       ├── package.json
│       ├── language-configuration.json
│       └── syntaxes/sabot.tmLanguage.json
│
└── docs/                       # Documentation
    ├── LANGUAGE.md             # Comprehensive language reference
    └── SYNTAX.md               # Quick syntax reference card
```

---

## Architecture

Sabot follows a classic compilation pipeline:

```
Source Code (.sabot)
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
- **Builtin registry** -- native Rust functions callable from Sabot

All concurrency uses Rust's `std::thread` with `Arc<Mutex<...>>` for shared state. Each spawned task gets its own VM (via `spawn_child`) that shares words, builtins, globals, channels, and routes, but has an independent stack and reactive cell state.

---

## Documentation

| Document | Description |
|----------|-------------|
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | **Comprehensive language reference.** Every builtin, every type, every feature -- with examples. Includes VM internals, compilation pipeline details, and architecture overview. |
| [docs/SYNTAX.md](docs/SYNTAX.md) | **Quick syntax reference card.** Grammar and examples for every construct, no prose. Useful as a cheat sheet while coding. |

---

## Development Workflow

### Branching Strategy

Sabot uses a two-branch model:

```
dev (active development)
 │
 ├── feature work, bug fixes, version bumps
 │
 └── Pull Request ──▶ main (stable, tagged releases)
                        │
                        └── auto-tag ──▶ release builds
```

- **`dev`** -- all day-to-day work happens here. Push freely.
- **`main`** -- stable branch. Code arrives via pull request from `dev`.
- When a PR merges to `main`, the **auto-tag** workflow reads the `VERSION` file and creates an annotated git tag (e.g., `v0.4.0`) if one doesn't already exist.
- Tag creation triggers the **release** workflow, which cross-compiles binaries and publishes a GitHub Release.

### Version Management

The `VERSION` file at the project root is the **single source of truth** for the version number. A sync script propagates it to all other files:

```bash
# Show current version
make version

# Bump to a new version (updates VERSION, Cargo.toml, main.rs, README, LANGUAGE.md)
make bump V=0.5.0

# Verify all files are in sync (used by CI)
make version-check
```

Files kept in sync:
| File | What gets updated |
|------|-------------------|
| `VERSION` | Raw version string (e.g., `0.4.0`) |
| `Cargo.toml` | `version = "0.4.0"` |
| `src/main.rs` | `Sabot v0.4.1` banner |
| `README.md` | `Sabot v0.4.1` references |
| `docs/LANGUAGE.md` | `v0.4.0` in the title |

### Makefile Targets

The `Makefile` provides shortcuts for every common task:

```bash
make help          # show all targets with descriptions
```

| Category | Targets |
|----------|---------|
| **Build** | `build`, `release`, `clean` |
| **Test** | `test` (all), `test-rust`, `test-sabot` |
| **Lint** | `fmt`, `fmt-check`, `clippy`, `lint` (all lints) |
| **Format** | `fmt-sabot` (format all .sabot files) |
| **Tools** | `bench`, `profile FILE=...`, `repl` |
| **Version** | `version`, `version-check`, `bump V=x.y.z` |
| **Release** | `check` (full pre-release), `release-dry-run` |
| **Install** | `install`, `uninstall` |

### Release Pipeline

Releases are fully automated via three GitHub Actions workflows:

**1. CI** (`.github/workflows/ci.yml`) -- runs on every push to `dev`/`main` and every PR to `main`:
- Build and test on **Linux, macOS, and Windows**
- `cargo fmt --check` and `cargo clippy -- -D warnings`
- Version sync verification (`scripts/version-sync.sh --check`)

**2. Auto-Tag** (`.github/workflows/auto-tag.yml`) -- runs when code lands on `main`:
- Reads the `VERSION` file
- Creates an annotated git tag (`v0.4.0`) if it doesn't already exist
- The new tag triggers the release workflow

**3. Release** (`.github/workflows/release.yml`) -- runs when a `v*` tag is pushed:
- Cross-compiles for **5 targets**:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-pc-windows-msvc`
- Packages each binary with README, LICENSE, CHANGELOG, standard library, and docs
- Generates SHA-256 checksums
- Creates a GitHub Release with all assets and changelog excerpt

**Typical release flow:**

```bash
# 1. Bump version on dev
make bump V=0.5.0

# 2. Update CHANGELOG.md with release notes

# 3. Commit and push
git add -A && git commit -m "chore: bump version to v0.5.0"
git push origin dev

# 4. Open a PR from dev -> main, review, merge

# 5. Everything else is automatic:
#    main push -> auto-tag -> release builds -> GitHub Release
```

---

## Dependencies

Sabot uses a minimal set of Rust crates:

| Crate        | Version | Purpose                                      |
|--------------|---------|----------------------------------------------|
| `ureq`       | 3       | HTTP client (GET/POST)                       |
| `rusqlite`   | 0.31    | SQLite database (bundled, no system install)  |
| `regex`      | 1       | Regular expression support                   |
| `serde_json` | 1       | JSON serialization/deserialization            |
| `serde_yaml` | 0.9     | YAML serialization/deserialization            |
| `toml`       | 0.8     | TOML serialization/deserialization            |

Everything else -- the lexer, parser, compiler, VM, reactive system, concurrency, binary encoding, HTTP server, formatter, profiler, and test runner -- is implemented from scratch.

---

## License

Sabot is licensed under the [GNU General Public License v3.0](LICENSE).
