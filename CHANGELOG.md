# Changelog

All notable changes to Sabo will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-03-06

### Added

#### Language Core
- Stack operations: `dup`, `drop`, `swap`, `over`, `rot`, `depth`
- Arithmetic, comparison, and logic operators
- Pattern matching with literal, variable, wildcard, and list destructuring patterns
- Guards on pattern arms (`where` clauses with `and`/`or`/`not`)
- Tail-call optimization for self-recursive calls
- Quotations (first-class code blocks) with closure capture
- Quotation composition (`.` operator)
- Let bindings with destructuring (positional, head/tail, map keys)
- String interpolation (`"hello {name}"`)
- Named stacks (`>name` push, `name>` pop)
- Symbols (`:ok`, `:error`, etc.)
- Maps with hash-based O(1) lookup (`#{"key" => val}`)
- Conditional execution (`if_else`, `if_true`)
- Error handling (`try`, `throw`, `error`) with source line numbers in errors

#### Standard Library
- `lib/math.sabo` -- 18 math utilities (factorial, fib, gcd, lcm, pow, etc.)
- `lib/list.sabo` -- 16 list utilities (take, drop, flatten, zip, chunk, etc.)
- `lib/string.sabo` -- 15 string utilities (pad, center, surround, etc.)
- `lib/functional.sabo` -- 10 functional utilities (compose, pipe, iterate, etc.)
- `lib/map.sabo` -- map predicates and sizing

#### Built-in I/O & Networking
- File I/O: `read_file`, `read_lines`, `write_file`, `append_file`, `file_exists`, `ls`
- Shell execution: `exec`, `exec_full`
- HTTP client: `http_get`, `http_post`
- HTTP server: `route`, `static_dir`, `serve` with path parameters and static file serving
- Stdin: `stdin_read`, `stdin_lines`, `input`
- Environment: `env_get`, `env_all`, `args`
- Time: `time_ms`, `sleep_ms`

#### Database
- SQLite support: `db_open`, `db_close`, `db_exec`, `db_exec_params`, `db_query`, `db_query_params`

#### Serialization
- JSON: `json_parse`, `json_encode`, `json_pretty` (via serde_json)
- YAML: `yaml_parse`, `yaml_encode` (via serde_yaml)
- TOML: `toml_parse`, `toml_encode` (via toml crate)
- Binary: `proto_encode`, `proto_decode` (custom TLV format)

#### Concurrency
- Basic: `spawn`, `await`, `send`, `recv`
- Typed channels: `channel`, `ch_send`, `ch_recv`, `ch_try_recv`, `ch_close`
- Structured: `spawn_linked`, `await_all`, `await_any`
- Cancellation: `cancel`, `cancelled?`
- Scheduling: `timeout`, `select`

#### Reactive System
- Cells: `cell`, `get_cell`, `set_cell`
- Computed cells with automatic dependency tracking
- Edge-triggered watchers (`when`)

#### Observability
- Tracing: `span_start`, `span_end`, `span`, `span_attr`, `span_event`, `span_error`, `spans_dump`, `spans_reset`
- Metrics: `counter_inc`, `counter_add`, `gauge_set`, `gauge_inc`, `gauge_dec`, `histogram_record`, `metrics_dump`, `metrics_reset`
- Structured logging: `log_debug`, `log_info`, `log_warn`, `log_error`, `log_with`, `log_level`, `logs_dump`, `logs_reset`
- Combined export: `otel_dump`, `otel_reset`

#### Tooling
- Interactive REPL with multi-line input and `.saborc` startup file
- Test runner (`sabo test`) with `assert`, `assert_eq`, `assert_neq`
- Source formatter (`sabo fmt`) with idempotent output
- Execution profiler (`sabo profile`) with per-word timing
- Memoization: `memo`, `memo_clear`
- Module system: `import` with dotted name access
- Regular expressions: `regex_match?`, `regex_find`, `regex_find_all`, `regex_replace`, `regex_split`, `regex_captures`
- Type introspection: `type_of`, `words`, `globals`
- String utilities: `split`, `join`, `trim`, `contains`, `starts_with`, `lowercase`, `uppercase`, `replace`, `chars`, `reverse`, `len`
- Conversions: `to_str`, `to_int`, `to_float`
- Hot reload: `load`, `watch`, `poll_watch`
- Stack traces: `backtrace`, `depth`

#### Editor Support
- VS Code extension with syntax highlighting, bracket matching, comment toggling, and string interpolation support

#### Example Programs
- `programs/wordcount.sabo` -- wc-like word frequency counter
- `programs/todo.sabo` -- SQLite-backed CLI task manager
- `programs/budget.sabo` -- reactive budget tracker
- `programs/server.sabo` -- HTTP server with REST API
- `programs/github_stats.sabo` -- GitHub profile fetcher
- `games/the_vault.sabo` -- text adventure game

[Unreleased]: https://github.com/seamoss/sabo/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/seamoss/sabo/releases/tag/v0.4.0
