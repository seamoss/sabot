# Sabot Syntax Reference

Quick reference for all Sabot syntax. For full documentation see [LANGUAGE.md](LANGUAGE.md).

---

## Comments

```sabot
-- This is a line comment (everything after -- is ignored)
```

---

## Literals

```sabot
42              -- integer
-7              -- negative integer
3.14            -- float
"hello"         -- string
:ok             -- symbol (atom)
true            -- boolean
false           -- boolean
```

### Escape Sequences (in strings)

```
\n   newline
\t   tab
\\   backslash
\"   quote
\{   literal brace (prevents interpolation)
```

---

## String Interpolation

```sabot
"hello {name}, you are {age}!"
```

Variables are looked up (locals → globals) and converted via `to_str`. Only simple names — no expressions inside `{}`.

---

## Collections

### Lists

```sabot
{1, 2, 3}           -- list literal
{}                   -- empty list
{1, "two", :three}   -- mixed types ok
```

### Maps

```sabot
#{"key" => "val", "n" => 42}   -- map literal
#{}                             -- empty map

-- Map literals only support simple values.
-- For computed values, use set:
#{} "key" some_expression set
```

---

## Stack Operations

```
dup     ( a -- a a )         duplicate top
drop    ( a -- )             discard top
swap    ( a b -- b a )       swap top two
over    ( a b -- a b a )     copy second-from-top
rot     ( a b c -- b c a )   rotate top three
```

---

## Operators

### Arithmetic

```
+   ( a b -- a+b )      addition / string concat
-   ( a b -- a-b )       subtraction
*   ( a b -- a*b )       multiplication
/   ( a b -- a/b )       division
%   ( a b -- a%b )       modulo
```

### Comparison

```
==   ( a b -- bool )     equal
!=   ( a b -- bool )     not equal
<    ( a b -- bool )     less than
>    ( a b -- bool )     greater than
<=   ( a b -- bool )     less or equal
>=   ( a b -- bool )     greater or equal
```

### Logic

```
and   ( a b -- bool )    logical and
or    ( a b -- bool )    logical or
not   ( a -- bool )      logical not
```

---

## Word Definitions

```sabot
: word_name
  [pattern] -> body_expressions
;
```

Words are Sabot's functions. Every word must have at least one pattern-matching arm.

Identifiers may end with `?` or `!` (convention: `?` for predicates, `!` for side-effects):

```sabot
: even? [n] -> n 2 % 0 == ;
: save! [data] -> data "out.txt" write_file ;
```

### Zero Arguments

```sabot
: greet [] -> "hello" println ;
```

### Single Argument

```sabot
: double [n] -> n 2 * ;
```

### Multiple Arguments

```sabot
: add [a, b] -> a b + ;
```

### Multi-Arm (Pattern Matching)

```sabot
: factorial
  [0] -> 1
  [n] -> n n 1 - factorial *
;
```

Arms are tried top-to-bottom. First match wins.

### Pattern Types

```sabot
[n]           -- bind to variable
[42]          -- match literal int
[3.14]        -- match literal float
["hello"]     -- match literal string
[:ok]         -- match literal symbol
[true]        -- match literal bool
[_]           -- wildcard (match anything, don't bind)
[]            -- empty pattern (match without consuming)
```

### List Patterns

```sabot
{[]}          -- match empty list
{[h | t]}     -- match head and tail (cons destructure)
```

### Guards

```sabot
: classify
  [n] where n > 0 -> "positive"
  [n] where n < 0 -> "negative"
  [n] -> "zero"
;

: in_range
  [n] where n >= 0 and n <= 100 -> true
  [_] -> false
;
```

Guard operators: `==`, `!=`, `<`, `>`, `<=`, `>=`, `and`, `or`, `not`

Guards can only reference pattern-bound variables and literals — no word calls or stack ops.

### Tail-Call Optimization

The last call in an arm body is optimized if it's a self-recursive call:

```sabot
: count_down
  [0] -> "done"
  [n] -> n 1 - count_down    -- tail call, O(1) stack space
;
```

---

## Let Bindings

```sabot
let x = 42
let msg = "hello " "world" +
let nums = 1 10 range
```

Global scope. Body is everything until newline/semicolon. Top of stack is bound.

### Destructuring

```sabot
let {a, b, c} = {10, 20, 30}           -- list positional
let {h | t} = {1, 2, 3}                -- head/tail (h=1, t={2,3})
let #{"name" => n, "age" => a} = map   -- map keys
```

---

## Quotations

```sabot
[1 2 +]           -- quotation (first-class code block)
[dup *]            -- can contain any expressions
```

### Execution

```sabot
[1 2 +] ~          -- apply: execute quotation (~)
[1 2 +] apply      -- same thing
```

### Closures

Quotations inside word bodies capture enclosing pattern-bound variables:

```sabot
: make_adder [n] -> [n +] ;
5 make_adder >add5     -- captures n=5
3 add5> ~              -- 8
```

### Composition

```sabot
[2 *] [1 +] .      -- compose: creates [2 * 1 +]
```

---

## Named Stacks

```sabot
42 >aux        -- push 42 onto named stack "aux"
aux>           -- pop from "aux" back to main stack
```

Syntax: `>name` (push), `name>` (pop). Any alphanumeric name works.

---

## Conditional Execution

```sabot
-- if_else: condition [then] [else] if_else
true ["yes"] ["no"] if_else    -- "yes"

-- if_true: condition [then] if_true
true ["yes" println] if_true
```

---

## Error Handling

```sabot
[1 0 /] try         -- {:error, "Division by zero"}
[42] try             -- {:ok, 42}
```

### Structured Errors

```sabot
-- throw: throw any value as an error (caught by try)
[:not_found throw] try           -- {:error, :not_found}
[42 throw] try                   -- {:error, 42}

-- error: convenience to build typed error tuples
["file missing" :io_error error] try  -- {:error, {:error, :io_error, "file missing"}}
```

### Error Messages

Runtime errors include source location:
```
[line 4] in 'my_word': Unknown word 'foo_bar'
[line 11] Cannot apply '+' to string and int
```

---

## Regex

```sabot
"hello123" "\\d+" regex_match?         -- true
"hello 42 world" "\\d+" regex_find     -- {:ok, "42"}
"a1 b2 c3" "\\d+" regex_find_all      -- {"1", "2", "3"}
"foo  bar" "\\s+" " " regex_replace    -- "foo bar"
"a,b,,c" "," regex_split              -- {"a", "b", "", "c"}
"2026-03-06" "(\\d+)-(\\d+)-(\\d+)" regex_captures  -- {"2026-03-06", "2026", "03", "06"}
```

Note: Use `\{` and `\}` in strings to avoid interpolation when regex needs literal braces.

---

## Reactive Cells

```sabot
100 "balance" cell                      -- create cell
"balance" get_cell                      -- read cell
200 "balance" set_cell                  -- update (triggers recomputation)

["balance" get_cell 1.1 *] "tax" computed   -- auto-recomputes
["balance" get_cell 0 <] ["OVERDRAWN!" println] when  -- edge-triggered watcher
```

---

## Concurrency

### Basic Spawn/Await

```sabot
[42 2 *] spawn       -- spawn task, returns task_id
await                 -- wait for result

"hello" "chan" send   -- send to named channel
"chan" recv            -- receive from channel (blocking)
```

### Typed Channels

```sabot
channel              -- create channel, returns channel_id (int)
let ch = dup

42 ch ch_send        -- send value to channel
ch ch_recv           -- blocking receive (returns value or :closed)
ch ch_try_recv       -- non-blocking: {:ok, val}, :empty, or :closed
ch ch_close          -- close channel (no more sends)
```

### Structured Concurrency

```sabot
-- spawn_linked: spawn tracked in current task group
[10 20 +] spawn_linked    -- returns task_id

-- await_all: wait for all tasks, returns list of {:ok, val} or {:error, msg}
{t1, t2, t3} await_all

-- await_any: wait for first to finish, cancel rest
{t1, t2} await_any        -- returns {task_id, {:ok, val}}
```

### Cancellation

```sabot
task_id cancel       -- signal cancellation (propagates to children)
cancelled?           -- check if current task is cancelled (bool)
```

### Scheduling

```sabot
-- timeout: run with time limit
[long_computation] 1000 timeout   -- {:ok, result} or {:error, :timeout}

-- select: wait on multiple channels
{ch1, ch2} select                 -- {channel_id, value}
```

---

## Module System

```sabot
"examples/lib/math" import    -- load module

math.square          -- call with module prefix
square               -- unqualified also works
```

---

## HTTP Server

```sabot
-- Define handler words
: handle_hello [req] -> "Hello!" text_response ;

-- Register routes
[handle_hello] "/hello" "GET" route

-- Serve static files
"public" "/static" static_dir

-- Start server
8080 serve
```

### Request Map Keys

```
method    -- "GET", "POST", etc.
path      -- "/api/users/42"
query     -- #{"key" => "val"} from query string
params    -- #{"id" => "42"} from path params
headers   -- #{"content-type" => "..."}
body      -- raw body string
```

### Response Map

```sabot
#{} "status" 200 set "body" "hello" set
#{} "status" 200 set "body" data json_encode set "content_type" "application/json" set
#{} "status" 302 set "headers" #{} "Location" "/new" set set
```

---

## Memoization

```sabot
"fib" memo           -- cache results of fib by argument
35 fib               -- computed once, cached
35 fib               -- instant cache hit
"fib" memo_clear     -- clear cache
```

---

## SQLite Database

```sabot
":memory:" db_open                  -- or "data.db" db_open

"CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)" db_exec drop
"INSERT INTO t (name) VALUES (?)" {"Alice"} db_exec_params drop

"SELECT * FROM t" db_query          -- list of maps: {#{id => 1, name => Alice}}
"SELECT * FROM t WHERE id = ?" {1} db_query_params

db_close
```

---

## REPL Commands

```
.help      show commands
.stack     show current stack
.words     list defined words
.globals   list global variables
.cells     list reactive cells
.clear     clear the stack
.reset     reset the VM
quit       exit
```

---

## Operator Precedence

There is none. Sabot is **postfix** (reverse Polish notation). Everything evaluates left-to-right:

```sabot
3 4 + 2 *    -- (3+4)*2 = 14
3 4 2 * +    -- 3+(4*2) = 11
```

---

## CLI Tools

```
sabot                       -- start REPL
sabot file.sabot             -- run file
sabot test dir/             -- run test files in directory
sabot fmt file.sabot         -- print formatted source to stdout
sabot fmt -w file.sabot      -- format file in place
sabot profile file.sabot     -- run with profiling, print report
```

---

## Resolution Order

When a name is encountered at runtime:
1. Builtins (built-in words)
2. User-defined words
3. Global variables (let bindings)
4. Reactive cells
5. Error: "Unknown word"
