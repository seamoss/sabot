# Sabot Language Reference v0.4.1

**Sabot** is a stack-based, pattern-matching scripting language. Think Forth meets Erlang — postfix evaluation, quotations, reactive cells, channels, and hot reload, all compiled to bytecode and run on a custom Rust VM.

---

## Table of Contents

1. [Philosophy & Design](#philosophy--design)
2. [Running Sabot](#running-sabot)
3. [Syntax Overview](#syntax-overview)
4. [Type System](#type-system)
5. [Stack Operations](#stack-operations)
6. [Arithmetic & Comparison](#arithmetic--comparison)
7. [Logic](#logic)
8. [String Operations](#string-operations)
9. [Lists](#lists)
10. [Maps](#maps)
11. [Word Definitions & Pattern Matching](#word-definitions--pattern-matching)
12. [Guards](#guards)
13. [Let Bindings](#let-bindings)
14. [String Interpolation](#string-interpolation)
15. [Quotations & Higher-Order Functions](#quotations--higher-order-functions)
16. [Named Stacks](#named-stacks)
17. [Conditional Execution](#conditional-execution)
18. [Error Handling](#error-handling)
19. [Reactive Cells](#reactive-cells)
20. [Concurrency](#concurrency)
21. [File I/O](#file-io)
22. [Shell Execution](#shell-execution)
23. [HTTP](#http)
24. [Serialization (JSON, YAML, TOML, Protobuf)](#serialization-json-yaml-toml-protobuf)
25. [Observability (Tracing, Metrics, Logging)](#observability-tracing-metrics-logging)
26. [Environment & System](#environment--system)
27. [Module System](#module-system)
28. [Testing](#testing)
29. [Introspection & Debugging](#introspection--debugging)
30. [Hot Reload](#hot-reload)
31. [HTTP Server](#http-server)
32. [REPL Commands](#repl-commands)
33. [Complete Builtin Reference](#complete-builtin-reference)

---

## Philosophy & Design

Sabot is intentionally weird. It combines:

- **Stack-based evaluation** (Forth): Everything is postfix. `2 3 +` pushes 2, pushes 3, adds them. No parentheses, no precedence rules. The stack *is* the program state.
- **Pattern matching** (Erlang/Haskell): Word definitions match on the shape of the stack. `[0] -> 1` matches when the top of stack is zero. `[n] where n > 0 -> ...` adds guard conditions.
- **Quotations** (Factor/Joy): Code is data. `[2 *]` is a first-class quotation you can pass around, compose, and apply.
- **Reactive dataflow** (spreadsheet-style): Cells, computed cells, and edge-triggered watchers create live data dependencies.
- **Channels** (Go/Erlang): Named channels for cross-thread message passing via `send` and `recv`.

The implementation is a bytecode compiler + virtual machine written in Rust. Source code goes through: **Lexer → Parser → Compiler → Bytecode → VM execution**.

---

## Running Sabot

```bash
# REPL (interactive)
sabot

# Run a file
sabot examples/hello.sabot

# Run tests
sabot test examples/tier4_tests.sabot
```

---

## Syntax Overview

Sabot is **whitespace-delimited** and **postfix** (reverse Polish notation). Every token is either a literal that gets pushed to the stack, or an operation that consumes/produces stack values.

```sabot
-- This is a comment (double dash to end of line)

-- Literals push themselves onto the stack
42              -- integer
3.14            -- float
"hello"         -- string
:ok             -- symbol
true            -- boolean
{1, 2, 3}      -- list
#{"a" => 1}    -- map

-- Operations consume from and push to the stack
2 3 +           -- pushes 2, pushes 3, adds → 5 on stack
"hi" println    -- pushes "hi", prints it with newline
```

**Newlines** are significant — they separate statements at the top level and terminate `let` bindings. Inside word definitions (`: ... ;`), quotations (`[...]`), lists (`{...}`), and maps (`#{...}`), newlines are treated as whitespace.

**Semicolons** (`;`) end word definitions and can optionally terminate `let` bindings.

**Identifiers** start with a letter or `_`, followed by alphanumerics, `_`, or `.` (for module-qualified names). An identifier may end with `?` or `!` — by convention, `?` marks predicates (return bool) and `!` marks words with side effects. Examples: `even?`, `empty?`, `save!`, `math.square`.

---

## Type System

Sabot has 8 value types:

| Type | Examples | Truthy? |
|------|----------|---------|
| `int` | `42`, `-7`, `0` | nonzero |
| `float` | `3.14`, `-0.5` | nonzero |
| `string` | `"hello"`, `""` | non-empty |
| `symbol` | `:ok`, `:error` | always |
| `bool` | `true`, `false` | true |
| `list` | `{1, 2, 3}`, `{}` | non-empty |
| `map` | `#{"a" => 1}`, `#{}` | non-empty |
| `quotation` | `[2 *]`, `[dup +]` | always |

Types are **dynamic**. Values are **immutable** — operations produce new values rather than mutating existing ones. The exception is reactive cells, which hold mutable state.

**Type coercion**: Arithmetic between `int` and `float` promotes to `float`. String `+` concatenates two strings. No other implicit coercions exist.

---

## Stack Operations

The operand stack is the primary data structure. All computation flows through it.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `dup` | `( a -- a a )` | Duplicate top of stack |
| `drop` | `( a -- )` | Discard top of stack |
| `swap` | `( a b -- b a )` | Swap top two values |
| `over` | `( a b -- a b a )` | Copy second value to top |
| `rot` | `( a b c -- b c a )` | Rotate third value to top |
| `depth` | `( -- n )` | Push current stack depth as int |
| `stack` | `( -- )` | Print the entire stack (for debugging) |

**Example:**
```sabot
1 2 3 rot      -- stack: 2 3 1
drop            -- stack: 2 3
dup             -- stack: 2 3 3
swap            -- stack: 2 3 3 (wait, let me redo)
```

```sabot
10 20 swap      -- stack: 20 10
over            -- stack: 20 10 20
```

---

## Arithmetic & Comparison

All arithmetic is **postfix** (operands pushed first, then the operator).

### Arithmetic Operators

| Operator | Types | Description |
|----------|-------|-------------|
| `+` | int, float, string | Add numbers or concatenate strings |
| `-` | int, float | Subtract |
| `*` | int, float | Multiply |
| `/` | int, float | Divide (integer division for ints) |
| `%` | int, float | Modulo |

```sabot
10 3 /          -- 3 (integer division)
10 3 %          -- 1
10.0 3 /        -- 3.3333... (float)
"hello " "world" + -- "hello world"
```

**Division by zero** produces a runtime error: `"Division by zero"`.

**Mixed int/float**: If either operand is float, the result is float. `5 2.0 *` → `10.0`.

### Comparison Operators

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less than or equal |
| `>=` | Greater than or equal |

All comparisons push a `bool` onto the stack.

```sabot
5 3 >           -- true
"abc" "def" <   -- true (lexicographic)
1 1.0 ==        -- true (int/float cross-comparison)
```

---

## Logic

| Operator | Stack Effect | Description |
|----------|-------------|-------------|
| `and` | `( a b -- bool )` | Logical AND (truthiness) |
| `or` | `( a b -- bool )` | Logical OR (truthiness) |
| `not` | `( a -- bool )` | Logical NOT (truthiness) |

These operate on **truthiness**, not just booleans. `0 not` → `true`. `"" not` → `true`. `42 not` → `false`.

---

## String Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `+` | `( str str -- str )` | Concatenate two strings |
| `len` | `( str -- int )` | String length |
| `split` | `( str delim -- list )` | Split string by delimiter |
| `join` | `( list delim -- str )` | Join list with delimiter |
| `trim` | `( str -- str )` | Remove leading/trailing whitespace |
| `contains` | `( str substr -- bool )` | Check if string contains substring |
| `starts_with` | `( str prefix -- bool )` | Check if string starts with prefix |
| `lowercase` | `( str -- str )` | Convert to lowercase |
| `uppercase` | `( str -- str )` | Convert to uppercase |
| `replace` | `( str pattern repl -- str )` | Replace all occurrences of pattern |
| `chars` | `( str -- list )` | Split string into list of single characters |
| `reverse` | `( str -- str )` | Reverse a string |
| `to_str` | `( any -- str )` | Convert any value to its string representation |
| `to_int` | `( str -- int )` | Parse string as integer |
| `to_float` | `( str -- float )` | Parse string as float |
| `int` | `( any -- int )` | Convert to int (alias) |
| `float` | `( any -- float )` | Convert to float (alias) |

## Regular Expressions

Powered by the Rust `regex` crate. Patterns use standard regex syntax.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `regex_match?` | `( str pattern -- bool )` | Test if string matches pattern |
| `regex_find` | `( str pattern -- result )` | Find first match: `{:ok, match}` or `:none` |
| `regex_find_all` | `( str pattern -- list )` | Find all matches as list of strings |
| `regex_replace` | `( str pattern repl -- str )` | Replace all matches with replacement |
| `regex_split` | `( str pattern -- list )` | Split string by regex pattern |
| `regex_captures` | `( str pattern -- result )` | Capture groups: `{full, group1, ...}` or `:none` |

```sabot
"hello123" "\\d+" regex_match?         -- true
"a1 b2 c3" "\\d+" regex_find_all      -- {"1", "2", "3"}
"foo  bar" "\\s+" " " regex_replace    -- "foo bar"
```

**Note:** Use `\{` and `\}` inside strings to prevent interpolation when your regex needs literal braces (e.g., `"\\d\{4\}"` for `\d{4}`).

```sabot
"hello world" " " split     -- {"hello", "world"}
{"a", "b", "c"} ", " join   -- "a, b, c"
"  spaces  " trim            -- "spaces"
42 to_str                    -- "42"
"123" to_int                 -- 123
```

---

## Lists

Lists are ordered, heterogeneous collections enclosed in `{...}`.

```sabot
{1, 2, 3}               -- a list of ints
{1, "hello", true}       -- mixed types
{}                       -- empty list
```

### List Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `len` | `( list -- int )` | Number of elements |
| `head` | `( list -- val )` | First element (error if empty) |
| `tail` | `( list -- list )` | All elements except first |
| `cons` | `( val list -- list )` | Prepend value to list |
| `append` | `( list val -- list )` | Append value to end |
| `empty` | `( list -- bool )` | Is the list empty? |
| `nth` | `( list int -- val )` | Get element at index (0-based) |
| `reverse` | `( list -- list )` | Reverse the list |
| `sort` | `( list -- list )` | Sort (comparable values) |
| `contains` | `( list val -- bool )` | Does list contain value? |

```sabot
{3, 1, 2} sort          -- {1, 2, 3}
{1, 2, 3} head          -- 1
{1, 2, 3} tail          -- {2, 3}
0 {1, 2, 3} cons        -- {0, 1, 2, 3}
{1, 2} 3 append         -- {1, 2, 3}
{1, 2, 3} 2 contains    -- true
```

### Higher-Order List Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `map` | `( list quot -- list )` | Apply quotation to each element, collect results |
| `map_list` | `( list quot -- list )` | Same as `map` |
| `filter` | `( list quot -- list )` | Keep elements where quotation returns truthy |
| `fold` | `( list init quot -- val )` | Left fold / reduce |
| `each` | `( list quot -- )` | Apply quotation to each element (no result collection) |
| `range` | `( start end -- list )` | Create list `{start, start+1, ..., end-1}` |

```sabot
{1, 2, 3, 4, 5} [2 *] map           -- {2, 4, 6, 8, 10}
{1, 2, 3, 4, 5} [3 >] filter        -- {4, 5}
{1, 2, 3, 4, 5} 0 [+] fold          -- 15
1 6 range                             -- {1, 2, 3, 4, 5}
{1, 2, 3} [println] each             -- prints 1, 2, 3 on separate lines
```

**How fold works**: The accumulator starts at `init`. For each element, both the accumulator and element are on the stack, and the quotation is applied. The result becomes the new accumulator.

```sabot
-- Fold trace for {1, 2, 3} 0 [+] fold:
-- stack: 0 1 → [+] → 1
-- stack: 1 2 → [+] → 3
-- stack: 3 3 → [+] → 6
-- result: 6
```

---

## Maps

Maps are ordered collections of key-value pairs, enclosed in `#{...}`.

```sabot
#{"name" => "Sabot", "version" => 4}
#{}                                    -- empty map
#{1 => "one", 2 => "two"}             -- non-string keys are fine
```

### Map Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `get` | `( map key -- val )` | Get value by key (error if missing) |
| `get_or` | `( map key default -- val )` | Get value or return default |
| `set` | `( map key val -- map )` | Set/update a key-value pair |
| `has` | `( map key -- bool )` | Does the map contain this key? |
| `keys` | `( map -- list )` | List of all keys |
| `values` | `( map -- list )` | List of all values |
| `merge` | `( map1 map2 -- map )` | Merge maps (map2 values override) |
| `len` | `( map -- int )` | Number of key-value pairs |
| `empty` | `( map -- bool )` | Is the map empty? |

```sabot
let m = #{"a" => 1, "b" => 2}
m "a" get                        -- 1
m "c" 0 get_or                   -- 0
m "c" 3 set                      -- #{"a" => 1, "b" => 2, "c" => 3}
m keys                           -- {"a", "b"}
```

**Implementation note**: Maps are stored as `Vec<(Value, Value)>`, preserving insertion order. Lookups are linear. This is fine for small maps; Sabot is not designed for giant data structures.

---

## Word Definitions & Pattern Matching

Words are Sabot's functions. They are defined with `: name ... ;` and must contain at least one pattern-matching arm.

```sabot
: double
  [n] -> n 2 *
;
```

This defines a word `double` that:
1. Takes one value from the stack, binds it to `n`
2. Pushes `n`, pushes `2`, multiplies

**Multi-arm definitions** try each arm top-to-bottom until one matches:

```sabot
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;

5 factorial   -- 120
```

### Pattern Types

| Pattern | Matches | Example |
|---------|---------|---------|
| `[n]` | Any value, binds to `n` | `[x] -> x x *` |
| `[42]` | Literal int 42 | `[0] -> 1` |
| `[3.14]` | Literal float | `[0.0] -> "zero"` |
| `["hello"]` | Literal string | `["quit"] -> exit` |
| `[:ok]` | Literal symbol | `[:ok] -> "success"` |
| `[true]` | Literal bool | `[true] -> "yes"` |
| `[_]` | Wildcard (matches anything, doesn't bind) | `[_] -> "default"` |
| `[]` | Empty pattern (matches without consuming) | `[] -> 42` |

### List Pattern Matching

List destructuring uses `{[...]}` syntax:

```sabot
: sum_list
  {[]} -> 0
  {[h | t]} -> h t sum_list +
;

{1, 2, 3} sum_list   -- 6
```

- `{[]}` matches an empty list
- `{[h | t]}` matches a non-empty list, binding the first element to `h` and the rest to `t`

### How Pattern Matching Compiles

Each arm compiles to:
1. `MatchBegin(n)` — copy top `n` stack values to the match stack
2. A sequence of `MatchLit*` / `MatchBind` / `MatchWildcard` ops
3. Optional guard check with `GuardFail(label)`
4. `MatchPop(n)` — remove the matched values from the main stack
5. Body code
6. `Jump(end_label)` — skip remaining arms

If a match fails, execution jumps to the next arm's label. If all arms fail, the word errors with `"No matching pattern in 'word_name'"`.

### Tail-Call Optimization

When the **last expression** in a word arm is a call to the same word (self-recursion), the compiler emits a `TailCall` instead of `Call`. The VM reuses the current call frame instead of pushing a new one, so tail-recursive words run in constant stack space.

```sabot
-- This runs in O(1) stack space thanks to TCO:
: count_down
  [0] -> "done"
  [n] -> n 1 - count_down     -- tail call: last thing in the arm
;

1000000 count_down   -- works fine, no stack overflow

-- Accumulator pattern for tail-recursive algorithms:
: fact_acc
  [0, acc] -> acc
  [n, acc] -> n 1 - n acc * fact_acc
;

: factorial [n] -> n 1 fact_acc ;
```

**What qualifies**: Only direct self-calls in tail position (the very last op before the arm ends). Calls that aren't last (e.g., `n 1 - fib n 2 - fib +` — the `+` comes after) are regular calls.

---

## Guards

Guards add conditions to pattern arms using `where`:

```sabot
: classify
  [n] where n > 0 -> "positive"
  [n] where n < 0 -> "negative"
  [n] -> "zero"
;
```

### Guard Syntax

```
[pattern] where <condition> -> body
```

**Conditions** are comparisons: `<expr> <op> <expr>`

- Comparison ops: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Boolean combinators: `and`, `or`, `not`
- Guard expressions can reference bound variables from the pattern

```sabot
: in_range
  [n] where n >= 0 and n <= 100 -> true
  [_] -> false
;

: is_special
  [n] where not n == 0 -> "not zero"
  [_] -> "zero"
;
```

**Note**: Guard expressions are limited to simple comparisons and references to pattern-bound variables or literals. They are **not** arbitrary Sabot expressions — you cannot call words or use stack operations inside guards.

---

## Let Bindings

`let` creates global variables by evaluating an expression and storing the result:

```sabot
let x = 42
let greeting = "hello " "world" +
let nums = 1 10 range
```

The body of a `let` is everything up to the next newline (or semicolon). The body is evaluated, and the **top of stack** is bound to the name.

```sabot
let task_id = [42] spawn    -- spawn returns task_id, stored in task_id
```

**Scope**: Let bindings are **global** — they persist for the lifetime of the VM. In the REPL, they carry across lines.

**Resolution order** when a name is used: builtins → words → globals → cells.

### Destructuring Let

`let` supports destructuring lists and maps:

```sabot
-- List positional: bind elements by index
let {a, b, c} = {10, 20, 30}     -- a=10, b=20, c=30

-- Head/tail: bind first element and rest
let {h | t} = {1, 2, 3, 4}       -- h=1, t={2,3,4}

-- Map: bind values by key
let #{"name" => n, "age" => a} = #{"name" => "Alice", "age" => 30}
-- n="Alice", a=30
```

List destructuring uses `nth` internally, head/tail uses `head`/`tail`, and map destructuring uses `get`. If the index or key doesn't exist, you'll get a runtime error.

---

## String Interpolation

Strings can embed variable references with `{name}`:

```sabot
let name = "Sabot"
let version = 4
"Welcome to {name} v{version}!" println
-- prints: Welcome to Sabot v4!
```

Interpolation works by:
1. Looking up the variable (globals or locals)
2. Converting it to a string via `to_str`
3. Concatenating the pieces

**Escaping**: Use `\{` to include a literal `{` in a string.

**Other escape sequences**: `\n` (newline), `\t` (tab), `\\` (backslash), `\"` (quote).

**Note**: Only simple variable names work in interpolation — no expressions. `"{x + 1}"` will try to look up a variable literally named `x + 1`.

---

## Quotations & Higher-Order Functions

Quotations are **first-class code blocks** enclosed in `[...]`. When defined inside word bodies, they act as **closures** — they capture enclosing pattern-bound variables.

```sabot
[2 *]             -- a quotation that doubles
[dup +]           -- a quotation that doubles differently
[println]         -- a quotation that prints
```

### Apply (`~`)

Execute a quotation on the current stack:

```sabot
5 [2 *] ~         -- 10
```

The `~` operator (or the word `apply`) pops a quotation and runs it.

### Compose (`.`)

Combine two quotations into one:

```sabot
[2 *] [1 +] .     -- [2 * 1 +]
5 swap ~           -- 11 (5 * 2 + 1)
```

### Closures

When quotations appear inside word definitions, they capture enclosing pattern-bound variables:

```sabot
: make_adder
  [n] -> [n +]      -- the quotation [n +] captures `n`
;

5 make_adder        -- pushes quotation [5 +]
10 swap ~           -- 15
```

This works because the compiler emits `LoadLocal` for pattern variables inside quotations, and `run_quotation_internal` inherits the parent frame's locals.

### Quotations as Arguments

Quotations are how Sabot does higher-order programming:

```sabot
{1, 2, 3} [2 *] map         -- {2, 4, 6}
{1, 2, 3} [3 >] filter      -- {}
{1, 2, 3} 0 [+] fold        -- 6
```

### Quotations in Reactive Cells

Quotations define the computation for computed cells and watchers:

```sabot
10 "x" cell
[x 2 *] "double_x" computed    -- auto-recomputes when x changes
```

---

## Named Stacks

Sabot supports **auxiliary named stacks** for temporary storage, useful when you need to get values "out of the way":

```sabot
42 >aux          -- push 42 to the "aux" stack
aux>             -- pop from "aux" back to main stack
```

**Syntax**: `>name` pushes to the named stack, `name>` pops from it. Any alphanumeric name works.

```sabot
-- Example: swap using a named stack
1 2 >temp temp> swap  -- equivalent to just 'swap', but demonstrates the concept

-- More practical: accumulating while iterating
{1, 2, 3, 4, 5} [>acc] each
acc> acc> acc> acc> acc>   -- 5 4 3 2 1 on stack
```

---

## Conditional Execution

Sabot provides `if_else` and `if_true` for branching with quotations:

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `if_else` | `( bool quot quot -- ... )` | If true, run first quotation; otherwise run second |
| `if_true` | `( bool quot -- ... )` | If true, run the quotation; otherwise do nothing |

```sabot
true  ["yes" println] ["no" println] if_else   -- prints "yes"
false ["yes" println] ["no" println] if_else   -- prints "no"

5 3 > ["big" println] if_true                  -- prints "big"
```

These are cleaner than the classic Forth pattern of `swap ~ drop ~` and allow readable nested branching:

```sabot
: classify
  [n] ->
    n 0 >
    ["positive" println]
    [
      n 0 <
      ["negative" println]
      ["zero" println]
      if_else
    ]
    if_else
;
```

---

## Error Handling

Sabot uses `try` for error handling, inspired by Erlang's `{:ok, value}` / `{:error, reason}` convention.

### Try

```sabot
[1 0 /] try                 -- {:error, Division by zero}
[42] try                     -- {:ok, 42}
[unknown_word] try           -- {:error, Unknown word 'unknown_word'}
```

`try` pops a quotation, executes it, and:
- **On success**: pushes `{:ok, <top-of-stack-result>}`
- **On error**: restores the stack to its state before the quotation and pushes `{:error, <error-message>}`

### Error Messages with Line Numbers

Runtime errors include the source line number and word name when available:

```
[line 4] in 'my_word': Unknown word 'foo_bar'
[line 11] in 'add_two': Cannot apply '+' to string and int
[line 7] Stack underflow: not enough values for 'drop'
```

Line numbers are tracked through the compilation pipeline (lexer → parser → compiler → VM) via a parallel line-number table stored alongside bytecode.

### Structured Errors (throw / error)

`throw` takes any value from the stack and throws it as an error. When caught by `try`, the thrown value is preserved:

```sabot
[:not_found throw] try           -- {:error, :not_found}
[42 throw] try                   -- {:error, 42}
[#{"code" => 404} throw] try    -- {:error, #{"code" => 404}}
```

`error` is a convenience that builds a typed error tuple and throws it:

```sabot
-- "message" :type error -> throws {:error, :type, "message"}
["file missing" :io_error error] try
-- {:error, {:error, :io_error, "file missing"}}
```

Without `try`, thrown errors propagate like any other runtime error.

### Pattern Matching on Results

```sabot
: safe_divide
  [a b] where b == 0 -> {:error, "division by zero"}
  [a b] -> {:ok, a b /}
;

-- Or with try:
[10 0 /] try
-- then match on the result...
```

---

## Reactive Cells

Reactive cells bring spreadsheet-like live computation to Sabot. When a cell changes, all dependent computed cells automatically recompute, and edge-triggered watchers fire.

### Cell Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `cell` | `( value name -- )` | Create a reactive cell |
| `get_cell` | `( name -- value )` | Get cell value explicitly |
| `set_cell` | `( value name -- )` | Update cell, triggers recomputation |
| `computed` | `( quot name -- )` | Register a computed cell |
| `when` | `( cond_quot action_quot -- )` | Register an edge-triggered watcher |

### Basic Cells

```sabot
10 "x" cell          -- create cell "x" with value 10
20 "y" cell          -- create cell "y" with value 20
x                    -- pushes 10 (cells are resolved by name)
"x" get_cell         -- also pushes 10
35 "x" set_cell      -- update x to 35, triggers recomputation
```

**Cell name resolution**: When the VM encounters a name during `Call`, it checks: builtins → words → globals → **cells**. So you can use a cell name directly as if it were a variable.

### Computed Cells

Computed cells are quotations that auto-recompute whenever *any* cell is updated via `set_cell`:

```sabot
10 "x" cell
20 "y" cell
[x y +] "sum" computed      -- initially evaluates to 30

sum                          -- 30

35 "x" set_cell              -- triggers recomputation
sum                          -- 55
```

**How recomputation works**: When `set_cell` is called, the VM:
1. Saves the current stack
2. For each computed cell: clears the stack, runs its quotation, stores the result
3. Restores the original stack

This means computed cell quotations run in **isolation** — they don't see or affect the main stack.

**Limitation**: Currently, ALL computed cells recompute on ANY `set_cell` call, regardless of whether they depend on the changed cell. There is no dependency tracking.

### Watchers

Watchers fire an action when a condition **becomes** true (edge-triggered, false→true transition):

```sabot
[sum 50 >] ["ALERT: sum exceeds 50!!" println] when
```

- The condition quotation is evaluated after each `set_cell`
- The action only fires on **false→true transitions** — not when the condition stays true
- Like computed cells, watchers save/restore the stack

```sabot
10 "x" cell
20 "y" cell
[x y +] "sum" computed
[sum 50 >] ["Alert! sum=" print sum println] when

35 "x" set_cell    -- sum becomes 55, watcher fires: "Alert! sum=55"
100 "y" set_cell   -- sum becomes 135, watcher does NOT fire (already true)
5 "x" set_cell     -- sum becomes 105, watcher does NOT fire (still true)
0 "x" set_cell     -- sum becomes 100, watcher does NOT fire (still true)
-50 "y" set_cell   -- sum becomes -50, condition becomes false
10 "y" set_cell    -- sum becomes 10, still false
60 "y" set_cell    -- sum becomes 60, watcher fires again! (false→true)
```

---

## Concurrency

Sabot supports lightweight concurrency via spawned tasks and named channels.

### Spawn & Await

```sabot
-- Spawn a task (runs in a new thread with its own VM)
[42] spawn              -- pushes task_id (an int)

-- Await the result
let tid = [42] spawn
tid await               -- pushes 42 (the task's final stack top)
```

`spawn` creates a **child VM** that:
- Shares words, builtins, and channels with the parent
- Has its own stack, globals, and frames
- Returns the top-of-stack value as its result

`await` blocks until the task completes, then pushes its result. If the task errored, `await` propagates the error.

### Channels

Channels are named, thread-safe FIFO queues for message passing:

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `send` | `( value name -- )` | Send value to named channel |
| `recv` | `( name -- value )` | Blocking receive from channel |
| `try_recv` | `( name -- result )` | Non-blocking receive |

Channels are **auto-created** on first `send` — there's no `channel` creation step.

```sabot
-- Producer/consumer pattern
[
  "hello" "work" send
  "world" "work" send
] spawn drop

50 sleep_ms

"work" recv println    -- hello
"work" recv println    -- world
```

`recv` **blocks** until a message is available (polling at 1ms intervals).

`try_recv` returns immediately:
- `{:ok, value}` if a message was available
- `{:error, "empty"}` if the channel is empty

### Parallel Computation

```sabot
let t1 = [1 1000000 range 0 [+] fold] spawn
let t2 = [1 500000 range 0 [+] fold] spawn

t1 await println    -- 499999500000
t2 await println    -- 124999750000
```

### Fan-Out Pattern

```sabot
[10 20 * "results" send] spawn drop
[30 40 * "results" send] spawn drop
[50 60 * "results" send] spawn drop

100 sleep_ms

"results" recv println   -- 200
"results" recv println   -- 1200
"results" recv println   -- 3000
```

**Thread safety**: Channels are `Arc<Mutex<HashMap<String, VecDeque<Value>>>>` — shared across all VMs (parent and children). Task results are stored in a separate `Arc<Mutex<...>>` keyed by task ID.

### Typed Channels

In addition to named (string-keyed) channels, Sabot has **typed channels** — integer-keyed, first-class channels that can be closed:

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `channel` | `( -- ch_id )` | Create new typed channel (returns int ID) |
| `ch_send` | `( value ch_id -- )` | Send value to channel |
| `ch_recv` | `( ch_id -- value )` | Blocking receive (returns `:closed` if closed+empty) |
| `ch_try_recv` | `( ch_id -- result )` | Non-blocking: `{:ok, val}`, `:empty`, or `:closed` |
| `ch_close` | `( ch_id -- )` | Close channel (no more sends allowed) |

```sabot
channel
let ch = dup

42 ch ch_send
ch ch_recv       -- 42

ch ch_close
ch ch_recv       -- :closed
```

### Structured Concurrency

Sabot supports structured concurrency with linked tasks, cancellation propagation, and task groups.

#### spawn_linked

Like `spawn`, but tracked in the current task group. Returns a task ID.

```sabot
[10 20 +] spawn_linked await    -- 30
```

#### await_all / await_any

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `await_all` | `( {ids} -- {results} )` | Wait for all tasks; returns list of `{:ok, val}` or `{:error, msg}` |
| `await_any` | `( {ids} -- {id, result} )` | Wait for first; cancels rest; returns `{task_id, {:ok, val}}` |

```sabot
[10] spawn_linked
let t1 = dup
[20] spawn_linked
let t2 = dup
{t1, t2} await_all    -- {{:ok, 10}, {:ok, 20}}
```

#### Cancellation

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `cancel` | `( task_id -- )` | Signal cancellation (propagates to children) |
| `cancelled?` | `( -- bool )` | Check if current task is cancelled |

Cancellation is **cooperative**: tasks must check `cancelled?` or be in a blocking operation (`ch_recv`, `await_all`, etc.) to respond. Cancelling a parent also cancels all its linked children.

### Scheduling Primitives

#### timeout

Runs a quotation with a time limit. Returns `{:ok, result}` on success or `{:error, :timeout}` if time runs out.

```sabot
[42] 1000 timeout            -- {:ok, 42}
[500 sleep_ms 42] 10 timeout -- {:error, :timeout}
```

#### select

Waits on multiple typed channels, returning the first available message. Returns `{channel_id, value}`. If a channel is closed, returns `{channel_id, :closed}`.

```sabot
channel
let ch1 = dup
channel
let ch2 = dup
"hello" ch1 ch_send
{ch1, ch2} select     -- {ch1_id, "hello"}
```

---

## File I/O

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `read_file` | `( path -- string )` | Read entire file as string |
| `read_lines` | `( path -- list )` | Read file as list of line strings |
| `write_file` | `( content path -- )` | Write string to file (creates/overwrites) |
| `append_file` | `( content path -- )` | Append string to file |
| `file_exists` | `( path -- bool )` | Does the file exist? |
| `ls` | `( path -- list )` | List directory entries (sorted) |

```sabot
"Hello!\nLine 2" "/tmp/test.txt" write_file
"/tmp/test.txt" read_file println       -- Hello!\nLine 2
"/tmp/test.txt" read_lines println      -- {Hello!, Line 2}
"/tmp/test.txt" file_exists println     -- true
"." ls println                          -- {Cargo.toml, src, examples, ...}
```

---

## Shell Execution

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `exec` | `( cmd -- stdout )` | Run shell command, return stdout |
| `exec_full` | `( cmd -- map )` | Run command, return `#{"stdout" => ..., "stderr" => ..., "code" => ...}` |

```sabot
"echo hello" exec println              -- hello
"ls -la" exec_full "code" get println  -- 0
```

Commands are executed via `sh -c "<cmd>"`.

---

## HTTP

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `http_get` | `( url -- body )` | HTTP GET, returns response body as string |
| `http_post` | `( body url -- response )` | HTTP POST (Content-Type: application/json) |

```sabot
"https://httpbin.org/get" http_get json_parse "headers" get println
"{\"key\": \"value\"}" "https://httpbin.org/post" http_post println
```

HTTP is implemented via `ureq` v3.

---

## Serialization (JSON, YAML, TOML, Protobuf)

All text-format serializers use `serde` under the hood. The binary format uses a custom TLV encoding.

### JSON

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `json_parse` | `( string -- value )` | Parse JSON string into Sabot values |
| `json_encode` | `( value -- string )` | Convert Sabot value to compact JSON string |
| `json_pretty` | `( value -- string )` | Convert Sabot value to pretty-printed JSON |

```sabot
#{"a" => 1, "b" => {2, 3}} json_encode  -- {"a":1,"b":[2,3]}
#{"a" => 1} json_pretty                  -- multi-line formatted
```

### YAML

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `yaml_parse` | `( string -- value )` | Parse YAML string into Sabot values |
| `yaml_encode` | `( value -- string )` | Convert Sabot value to YAML string |

```sabot
"name: sabot\nversion: 4" yaml_parse  -- #{"name" => "sabot", "version" => 4}
#{"color" => "red"} yaml_encode      -- "color: red\n"
```

### TOML

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `toml_parse` | `( string -- value )` | Parse TOML string into Sabot values |
| `toml_encode` | `( value -- string )` | Convert Sabot value to TOML string |

```sabot
"name = \"sabot\"\nversion = 4" toml_parse  -- #{"name" => "sabot", "version" => 4}
#{"key" => "value"} toml_encode            -- "key = \"value\"\n"
```

### Binary Encoding (Protobuf-style)

Self-describing TLV (type-length-value) binary format. Encodes to/from a list of byte values (0-255).

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `proto_encode` | `( value -- byte-list )` | Encode any Sabot value to binary |
| `proto_decode` | `( byte-list -- value )` | Decode binary back to Sabot value |

```sabot
#{"name" => "sabot", "version" => 4} proto_encode proto_decode
-- roundtrips perfectly
```

Wire format type tags: `0x00`=null, `0x01`=bool, `0x02`=int(i64), `0x03`=float(f64), `0x04`=string, `0x05`=symbol, `0x06`=list, `0x07`=map.

### Type mapping (all text formats)

| Format value | Sabot value |
|------|------|
| `"string"` | `Str` |
| `42` / `3.14` | `Int` / `Float` |
| `true`/`false` | `Bool` |
| `null` | `Symbol(:null)` |
| arrays/sequences | `List` |
| objects/mappings/tables | `Map` |

---

## Observability (Tracing, Metrics, Logging)

Built-in OpenTelemetry-style observability. All state lives in the VM — no external dependencies or exporters required.

### Tracing (Spans)

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `span_start` | `( name -- span_id )` | Start a new span, push onto context stack |
| `span_end` | `( -- )` | End the current (topmost) span |
| `span` | `( name [body] -- results )` | Scoped span: auto start/end around quotation |
| `span_attr` | `( key value -- )` | Add attribute to current span |
| `span_event` | `( name -- )` | Add timestamped event to current span |
| `span_error` | `( msg -- )` | Mark current span as error with message |
| `spans_dump` | `( -- list )` | Export all spans as list of maps |
| `spans_reset` | `( -- )` | Clear all span data |

```sabot
-- Manual span management
"db-query" span_start drop
  "sql" "SELECT * FROM users" span_attr
  "cache-miss" span_event
span_end

-- Scoped span (preferred)
"process-request" [
  "http.method" "GET" span_attr
  do_work
] span
```

Spans nest automatically — child spans record their parent's ID. Each span includes: `id`, `name`, `parent_id`, `start_ms`, `end_ms`, `duration_ms`, `status`, `attributes`, `events`.

### Metrics

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `counter_inc` | `( name -- )` | Increment counter by 1 |
| `counter_add` | `( n name -- )` | Increment counter by n |
| `gauge_set` | `( value name -- )` | Set gauge to value |
| `gauge_inc` | `( name -- )` | Increment gauge by 1 |
| `gauge_dec` | `( name -- )` | Decrement gauge by 1 |
| `histogram_record` | `( value name -- )` | Record observation in histogram |
| `metrics_dump` | `( -- map )` | Export all metrics as map of maps |
| `metrics_reset` | `( -- )` | Clear all metrics |

```sabot
-- Counters (monotonically increasing)
"http.requests" counter_inc
5 "cache.hits" counter_add

-- Gauges (point-in-time values)
42 "active.connections" gauge_set
"active.connections" gauge_dec

-- Histograms (distributions)
123.4 "response.time_ms" histogram_record
```

`metrics_dump` returns each metric with `type` (`:counter`/`:gauge`/`:histogram`) and relevant fields. Histograms include `count`, `sum`, `min`, `max`, `avg`.

### Structured Logging

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `log_debug` | `( msg -- )` | Log at DEBUG level |
| `log_info` | `( msg -- )` | Log at INFO level |
| `log_warn` | `( msg -- )` | Log at WARN level |
| `log_error` | `( msg -- )` | Log at ERROR level |
| `log_with` | `( msg fields level -- )` | Log with structured fields |
| `log_level` | `( level -- )` | Set minimum log level |
| `logs_dump` | `( -- list )` | Export all log entries as list of maps |
| `logs_reset` | `( -- )` | Clear all log entries |

```sabot
"Server started on port 8080" log_info
"Request failed" #{"status" => 500, "path" => "/api"} :error log_with
:warn log_level  -- suppress debug/info output
```

Logs are printed to stderr with timestamp and level, and stored for later export via `logs_dump`. Each entry includes `timestamp_ms`, `level`, `message`, `fields`, and `span_id` (if logged within an active span).

### Combined Export

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `otel_dump` | `( -- map )` | Export all telemetry: `#{"spans" => [...], "metrics" => #{...}, "logs" => [...]}` |
| `otel_reset` | `( -- )` | Reset all spans, metrics, and logs |

---

## SQLite Database

Built-in SQLite support via `rusqlite` (bundled). One database connection per VM.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `db_open` | `( path -- )` | Open or create a SQLite database |
| `db_close` | `( -- )` | Close the database |
| `db_exec` | `( sql -- rows_affected )` | Execute DDL/DML statement |
| `db_exec_params` | `( sql params -- rows_affected )` | Execute with bound parameters |
| `db_query` | `( sql -- results )` | Query, returns list of maps |
| `db_query_params` | `( sql params -- results )` | Query with bound parameters |

**Type mapping (Sabot → SQLite)**:
| Sabot | SQLite |
|------|--------|
| `Int` | INTEGER |
| `Float` | REAL |
| `Str` | TEXT |
| `Bool` | INTEGER (0/1) |
| `:null` | NULL |

**Query results** come back as a list of maps, where each map represents a row with column names as keys:

```sabot
":memory:" db_open

"CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)" db_exec drop
"INSERT INTO users (name, age) VALUES (?, ?)" {"Alice", 30} db_exec_params drop

"SELECT * FROM users WHERE age > ?" {25} db_query_params
-- {#{id => 1, name => Alice, age => 30}}

db_close
```

Use `":memory:"` for in-memory databases, or a file path for persistent storage.

---

## Environment & System

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `input` | `( prompt -- string )` | Print prompt, read one line from stdin (errors on EOF) |
| `env_get` | `( name -- string )` | Get environment variable (empty string if unset) |
| `env_all` | `( -- map )` | All environment variables as a map |
| `args` | `( -- list )` | Command-line arguments |
| `time_ms` | `( -- int )` | Current time in milliseconds since Unix epoch |
| `sleep_ms` | `( int -- )` | Sleep for N milliseconds |

```sabot
"HOME" env_get println       -- /Users/username
let start = time_ms
-- ... do work ...
let elapsed = time_ms start -
"Took {elapsed}ms" println
```

---

## Module System

Modules let you organize code across files and import definitions.

### Import

```sabot
"path/to/module.sabot" import
```

`import`:
1. Derives the module name from the filename (e.g., `lib/math.sabot` → `math`)
2. Compiles the file
3. Registers all word definitions both **unqualified** (`square`) and **qualified** (`math.square`)
4. Runs the module's top-level code

```sabot
-- examples/lib/math.sabot
: square
  [n] -> n n *
;

-- main.sabot
"examples/lib/math.sabot" import
5 square println         -- 25
7 math.square println    -- 49
```

### Load (non-module)

```sabot
"path/to/file.sabot" load
```

`load` evaluates a file in the **current** VM context, as if the code were inlined. No module prefixing. Useful for configuration files, REPL includes, or hot-reload scenarios.

---

## Testing

Sabot has built-in assertion words and a `test` runner.

### Assertion Words

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `assert` | `( value -- )` | Fails if value is not truthy |
| `assert_eq` | `( a b -- )` | Fails if `a != b` |
| `assert_neq` | `( a b -- )` | Fails if `a == b` |

```sabot
true assert                   -- passes
2 3 + 5 assert_eq             -- passes
1 2 assert_neq                -- passes

false assert                  -- ERROR: Assertion failed: got false
1 2 assert_eq                 -- ERROR: Assertion failed: 1 != 2
5 5 assert_neq                -- ERROR: Assertion failed: 5 == 5 (expected different)
```

### Running Tests

```bash
sabot test examples/tier4_tests.sabot
```

The `test` command runs the file and reports pass/fail. If any assertion fails, execution stops and the error is printed.

A test file is just a normal Sabot file that uses `assert` / `assert_eq` / `assert_neq`:

```sabot
-- test_math.sabot

: double [n] -> n 2 * ;

5 double 10 assert_eq
0 double 0 assert_eq
-3 double -6 assert_eq
"double tests: OK" println

: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;

5 factorial 120 assert_eq
0 factorial 1 assert_eq
"factorial tests: OK" println
```

---

## Memoization

Memoization caches a word's results so repeated calls with the same arguments return instantly.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `memo` | `( name -- )` | Enable memoization for a word |
| `memo_clear` | `( name -- )` | Clear the memo cache for a word |

```sabot
: fib
  [0] -> 0
  [1] -> 1
  [n] -> n 1 - fib n 2 - fib +
;

"fib" memo          -- enable memoization

35 fib              -- 9227465, computed in ~7ms (vs ~15s without memo)
35 fib              -- instant cache hit (0ms)

"fib" memo_clear    -- clear cache if needed
```

**How it works**: When a memoized word is called, the VM builds a cache key from the top N stack values (where N is the word's arity, detected from its first `MatchBegin` op). On cache hit, it pops the args and pushes the cached result without executing the word. On cache miss, it runs the word to completion and caches the result.

**Best for**: Pure functions with expensive recursive computation (like `fib`). Not suitable for words with side effects (I/O, cells, etc.) since cached results skip execution.

---

## Introspection & Debugging

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `type_of` | `( value -- string )` | Type name as string (`"int"`, `"float"`, etc.) |
| `type` | `( value -- symbol )` | Type name as symbol (`:int`, `:float`, etc.) |
| `backtrace` | `( -- list )` | Current call stack as list of word names |
| `depth` | `( -- int )` | Current stack depth |
| `words` | `( -- list )` | List of all defined word names |
| `globals` | `( -- list )` | List of all global variable names |

```sabot
42 type_of println           -- int
3.14 type println            -- :float

: inner
  [] -> backtrace println
;
: outer
  [] -> inner
;
outer                         -- {inner, outer}

1 2 3 depth println           -- 3
```

---

## Hot Reload

Sabot supports live code reloading for interactive development.

### Watch & Poll

```sabot
"my_module.sabot" watch       -- loads file and starts watching for changes
-- ... later ...
poll_watch                    -- reloads if file changed since last check
```

`watch`:
1. Loads the file initially (like `load`)
2. Spawns a background thread that monitors the file's modification time (polling every 500ms)
3. When a change is detected, sends the path on an internal `__watch__` channel

`poll_watch`:
1. Drains the `__watch__` channel
2. Reloads any changed files in the current VM context

This is designed for the REPL workflow: you edit a file in your editor, then call `poll_watch` (or set up a loop) to pick up changes.

---

## HTTP Server

Sabot includes a built-in HTTP/1.1 server. No external dependencies — raw TCP, hand-rolled request parsing, threaded request handling.

### Server Builtins

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `route` | `( quot path method -- )` | Register a route handler |
| `static_dir` | `( fs_path url_prefix -- )` | Serve static files from a directory |
| `serve` | `( port -- )` | Start the server (blocks forever) |

### Defining Routes

Routes map HTTP method + URL pattern to a handler. The handler is a quotation (or word reference) that receives a **request map** on the stack and must leave a **response map**:

```sabot
-- Handler as a named word
: handle_hello
  [req] -> #{"status" => 200, "body" => "Hello!"}
;
[handle_hello] "/hello" "GET" route

-- Multiple routes
[handle_users] "/users"     "GET"  route
[handle_user]  "/users/:id" "GET"  route
[create_user]  "/users"     "POST" route
```

### Path Parameters

URL patterns support `:name` segments for path parameters:

```sabot
: handle_greet
  [req] ->
    req "params" get "name" get
    "Hello, " swap + "!" +
    #{} "status" 200 set "body" swap set
;
[handle_greet] "/greet/:name" "GET" route
-- GET /greet/world -> "Hello, world!"
```

### Request Map

The handler receives a map with these keys:

| Key | Type | Description |
|-----|------|-------------|
| `"method"` | string | HTTP method (`"GET"`, `"POST"`, etc.) |
| `"path"` | string | Request path (`"/users/42"`) |
| `"query"` | map | Query parameters (`#{"page" => "1"}`) |
| `"headers"` | map | Request headers (lowercase keys) |
| `"body"` | string | Request body (empty for GET) |
| `"params"` | map | Path parameters (`#{"id" => "42"}`) |

### Response Map

The handler must leave a map on the stack:

| Key | Required? | Default | Description |
|-----|-----------|---------|-------------|
| `"status"` | yes | — | HTTP status code (int) |
| `"body"` | yes | — | Response body (string) |
| `"content_type"` | no | `"text/plain; charset=utf-8"` | Content-Type header |
| `"headers"` | no | `#{}` | Additional response headers |

If the handler returns a plain string instead of a map, it's treated as a 200 response with that string as the body.

### Static File Serving

Serve files from a directory with automatic MIME type detection:

```sabot
"./public" "/static" static_dir
-- GET /static/style.css -> serves ./public/style.css with Content-Type: text/css
-- GET /static/ -> serves ./public/index.html (if it exists)
```

Supported MIME types: html, css, js, json, xml, txt, csv, png, jpg, gif, svg, ico, webp, woff/woff2, ttf, pdf, zip, wasm.

Directory traversal (`..`) is blocked.

### Request Logging

All requests are logged to stdout with method, path, status, and timing:

```
  GET /hello -> 200 [0.3ms] 16 bytes
  GET /api/fib/10 -> 200 [2.0ms] 20 bytes
  GET /static/index.html -> 200 [0.5ms] static
  GET /missing -> 404 [0.1ms]
```

### Complete Example

```sabot
-- Response helper
: json_ok
  [data] ->
    #{} "status" 200 set
    "content_type" "application/json; charset=utf-8" set
    "body" data json_encode set
;

-- Routes
: handle_hello
  [req] -> #{} "status" 200 set "body" "Hello!" set
;

: handle_greet
  [req] ->
    req "params" get "name" get
    "Hello, " swap + "!" +
    #{} "status" 200 set "body" swap set
;

: handle_time
  [req] -> #{} "time_ms" time_ms set json_ok
;

[handle_hello] "/hello"       "GET" route
[handle_greet] "/greet/:name" "GET" route
[handle_time]  "/api/time"    "GET" route

"./public" "/static" static_dir

8080 serve
```

### Architecture Notes

- Each request runs in its own **thread** with a fresh child VM
- Child VMs share words, builtins, and globals with the parent
- Child VMs do **not** share reactive cells (cells are per-VM)
- Connections are close-after-response (no keep-alive)
- The server handles malformed requests gracefully (400)
- Handler errors are caught and returned as 500 responses
- `serve` blocks forever — place it at the end of your script

---

## REPL Commands

The REPL supports dot-commands for introspection:

| Command | Description |
|---------|-------------|
| `.help` | Show available commands |
| `.stack` | Display the current stack |
| `.words` | List all defined words |
| `.globals` | List all global variables and their values |
| `.cells` | List all reactive cells and their values |
| `.clear` | Clear the stack |
| `.reset` | Reset the entire VM to initial state |
| `stack` | Same as `.stack` |
| `quit` / `exit` | Exit the REPL |

The REPL supports **multi-line input** — if your input has unclosed delimiters (`:` without `;`, `[` without `]`, etc.), the REPL shows `...` and waits for more input.

After each evaluation, the REPL displays the stack (if non-empty) in `<val1 val2 ...>` format.

---

## Complete Builtin Reference

### Core Stack
`dup`, `drop`, `swap`, `over`, `rot`, `depth`, `stack`

### Arithmetic
`+`, `-`, `*`, `/`, `%`

### Comparison
`==`, `!=`, `<`, `>`, `<=`, `>=`

### Logic
`and`, `or`, `not`

### I/O
`print`, `println`

### Type Conversion
`to_str`, `to_int`, `to_float`, `int`, `float`, `type`, `type_of`

### String
`split`, `join`, `trim`, `contains`, `starts_with`, `lowercase`, `uppercase`, `replace`, `chars`, `reverse`, `len`

### List
`head`, `tail`, `cons`, `append`, `nth`, `empty`, `reverse`, `sort`, `contains`, `len`, `range`

### Higher-Order
`map` / `map_list`, `filter`, `fold`, `each`

### Map
`get`, `get_or`, `set`, `has`, `keys`, `values`, `merge`

### Control Flow
`if_else`, `if_true`

### Error Handling
`try`

### Reactive
`cell`, `get_cell`, `set_cell`, `computed`, `when`

### Concurrency
`spawn`, `await`, `send`, `recv`, `try_recv`, `channel`, `ch_send`, `ch_recv`, `ch_try_recv`, `ch_close`, `spawn_linked`, `await_all`, `await_any`, `cancel`, `cancelled?`, `timeout`, `select`

### File I/O
`read_file`, `read_lines`, `write_file`, `append_file`, `file_exists`, `ls`

### Shell
`exec`, `exec_full`

### HTTP
`http_get`, `http_post`

### Serialization
`json_parse`, `json_encode`, `json_pretty`, `yaml_parse`, `yaml_encode`, `toml_parse`, `toml_encode`, `proto_encode`, `proto_decode`

### Observability
`span_start`, `span_end`, `span`, `span_attr`, `span_event`, `span_error`, `spans_dump`, `spans_reset`, `counter_inc`, `counter_add`, `gauge_set`, `gauge_inc`, `gauge_dec`, `histogram_record`, `metrics_dump`, `metrics_reset`, `log_debug`, `log_info`, `log_warn`, `log_error`, `log_with`, `log_level`, `logs_dump`, `logs_reset`, `otel_dump`, `otel_reset`

### Stdin
`stdin_read`, `stdin_lines`, `input`

### Environment
`env_get`, `env_all`, `args`

### Time
`time_ms`, `sleep_ms`

### Testing
`assert`, `assert_eq`, `assert_neq`

### Memoization
`memo`, `memo_clear`

### Introspection
`type_of`, `type`, `backtrace`, `depth`, `words`, `globals`

### Modules
`import`, `load`

### Hot Reload
`watch`, `poll_watch`

### HTTP Server
`route`, `static_dir`, `serve`

### SQLite Database
`db_open`, `db_close`, `db_exec`, `db_exec_params`, `db_query`, `db_query_params`

### Named Stacks
`>name` (push), `name>` (pop) — where `name` is any identifier

---

## Implementation Architecture

### Pipeline

```
Source Code (.sabot)
    ↓
Lexer (src/lexer.rs)
    → Vec<Spanned<Token>>
    ↓
Parser (src/parser.rs)
    → Program (Vec<Item>)
    ↓
Compiler (src/compiler.rs)
    → CompiledProgram { words: HashMap<String, Vec<Op>>, main: Vec<Op> }
    ↓
VM (src/vm.rs)
    → Execution on operand stack
```

### Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/token.rs` | ~81 | Token enum (30+ variants), StringPart, Span |
| `src/lexer.rs` | ~437 | Character-by-character tokenizer, `is_incomplete()` for REPL |
| `src/ast.rs` | ~136 | AST types: Pattern, Guard, Expr, MatchArm, WordDef, LetBinding |
| `src/parser.rs` | ~451 | Recursive descent parser with pattern/quotation disambiguation |
| `src/compiler.rs` | ~319 | AST→bytecode compiler with label-based control flow |
| `src/opcode.rs` | ~91 | 40+ bytecode instructions |
| `src/value.rs` | ~106 | Runtime value type with Display, PartialEq, PartialOrd |
| `src/vm.rs` | ~1741 | Core VM: stack, frames, 80+ builtins, reactive system, concurrency |
| `src/builtins_io.rs` | ~310 | External builtins: file I/O, shell, HTTP, env, time |
| `src/builtins_serial.rs` | ~437 | Serialization: JSON, YAML, TOML, protobuf-style binary |
| `src/builtins_otel.rs` | ~680 | Observability: tracing spans, metrics, structured logging |
| `src/main.rs` | ~170 | Entry point: REPL, file runner, test runner |

### Key VM Data Structures

```rust
pub struct VM {
    stack: Vec<Value>,                    // Main operand stack
    named_stacks: HashMap<String, Vec<Value>>,  // Auxiliary stacks (>name / name>)
    globals: HashMap<String, Value>,      // Let bindings
    words: HashMap<String, Vec<Op>>,      // Word definitions (compiled)
    frames: Vec<CallFrame>,               // Call stack
    builtins: HashMap<String, fn(&mut VM) -> Result<(), String>>,

    // Reactive cells
    cells: HashMap<String, Value>,        // Cell values
    computed_cells: Vec<ComputedCell>,    // Quotation + name pairs
    watchers: Vec<Watcher>,               // Condition + action + edge state

    // Concurrency
    channels: Arc<Mutex<HashMap<String, VecDeque<Value>>>>,
    next_task_id: u64,
    task_results: Arc<Mutex<HashMap<u64, Option<Result<Value, String>>>>>,
}

struct CallFrame {
    code: Vec<Op>,           // Bytecode being executed
    ip: usize,               // Instruction pointer
    locals: Vec<Value>,      // Pattern-bound variables
    match_stack: Vec<Value>, // Values being pattern-matched
    word_name: Option<String>, // For backtrace display
}
```

### Name Resolution Order (Op::Call)

When the VM encounters `Op::Call("name")`:

1. **Builtins** — built-in functions (print, dup, map, etc.)
2. **Words** — user-defined word definitions
3. **Globals** — let bindings
4. **Cells** — reactive cell values

First match wins. This means a builtin cannot be shadowed by a word, but a word can shadow a global.

### Pattern Match Compilation Example

```sabot
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;
```

Compiles to:
```
MatchBegin(1)              -- save top 1 stack value
MatchLitInt(0, label_1)    -- if top != 0, jump to label_1
MatchPop(1)                -- remove matched value from stack
PushInt(1)                 -- push 1
Jump(label_end)            -- done
Label(label_1)             -- second arm
MatchBegin(1)
MatchBind(0)               -- bind top to local slot 0 (n)
MatchPop(1)
LoadLocal(0)               -- push n
LoadLocal(0)               -- push n
PushInt(1)
Sub                        -- n - 1
Call("factorial")          -- recursive call
Mul                        -- n * factorial(n-1)
Jump(label_end)
Label(label_end)
Return
```

### Dependency

External dependencies: `ureq` for HTTP, `serde_json`/`serde_yaml`/`toml` for text serialization, `rusqlite` for SQLite, `regex` for regular expressions. Everything else — the lexer, parser, compiler, VM, reactive system, concurrency, and binary encoding — is implemented from scratch.
