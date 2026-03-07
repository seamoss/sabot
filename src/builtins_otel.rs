// OpenTelemetry-style observability: tracing, metrics, and structured logging.
// All state lives in `OtelState` on the VM -- no external dependencies.

use crate::value::Value;
use crate::vm::VM;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================
// Data types
// ============================================================

#[derive(Clone, Debug)]
pub struct Span {
    pub id: u64,
    pub name: String,
    pub parent_id: Option<u64>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub attributes: HashMap<String, Value>,
    pub events: Vec<SpanEvent>,
    pub status: SpanStatus,
}

#[derive(Clone, Debug)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpanStatus {
    Ok,
    Error(String),
    Unset,
}

#[derive(Clone, Debug)]
pub struct MetricEntry {
    pub kind: MetricKind,
}

#[derive(Clone, Debug)]
pub enum MetricKind {
    Counter(f64),
    Gauge(f64),
    Histogram(HistogramData),
}

#[derive(Clone, Debug)]
pub struct HistogramData {
    pub values: Vec<f64>,
    pub sum: f64,
    pub count: u64,
    pub min: f64,
    pub max: f64,
}

impl HistogramData {
    fn new() -> Self {
        HistogramData {
            values: Vec::new(),
            sum: 0.0,
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    fn record(&mut self, val: f64) {
        self.values.push(val);
        self.sum += val;
        self.count += 1;
        if val < self.min {
            self.min = val;
        }
        if val > self.max {
            self.max = val;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    fn from_value(val: &Value) -> Result<LogLevel, String> {
        match val {
            Value::Symbol(s) | Value::Str(s) => match s.as_str() {
                "debug" => Ok(LogLevel::Debug),
                "info" => Ok(LogLevel::Info),
                "warn" => Ok(LogLevel::Warn),
                "error" => Ok(LogLevel::Error),
                _ => Err(format!(
                    "Unknown log level '{}' (use :debug, :info, :warn, :error)",
                    s
                )),
            },
            _ => Err(format!(
                "Log level must be a symbol, got {}",
                val.type_name()
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp_ms: i64,
    pub level: LogLevel,
    pub message: String,
    pub fields: HashMap<String, Value>,
    pub span_id: Option<u64>,
}

// ============================================================
// OtelState — lives on the VM
// ============================================================

#[derive(Clone, Debug)]
pub struct OtelState {
    // Tracing
    pub spans: Vec<Span>,
    pub span_stack: Vec<u64>, // active span context stack
    next_span_id: u64,

    // Metrics
    pub metrics: HashMap<String, MetricEntry>,

    // Logging
    pub logs: Vec<LogEntry>,
    pub min_log_level: LogLevel,
}

impl OtelState {
    pub fn new() -> Self {
        OtelState {
            spans: Vec::new(),
            span_stack: Vec::new(),
            next_span_id: 1,
            metrics: HashMap::new(),
            logs: Vec::new(),
            min_log_level: LogLevel::Debug,
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

// ============================================================
// Tracing builtins
// ============================================================

// "name" span_start -> span_id
fn span_start(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        Value::Symbol(s) => s,
        _ => {
            return Err(format!(
                "'span_start' expects string/symbol name, got {}",
                name.type_name()
            ));
        }
    };

    let otel = vm.otel_mut();
    let id = otel.next_span_id;
    otel.next_span_id += 1;
    let parent_id = otel.span_stack.last().copied();

    otel.spans.push(Span {
        id,
        name: name_str,
        parent_id,
        start_ms: now_ms(),
        end_ms: None,
        attributes: HashMap::new(),
        events: Vec::new(),
        status: SpanStatus::Unset,
    });
    otel.span_stack.push(id);

    vm.push_val(Value::Int(id as i64));
    Ok(())
}

// span_end -> ()
fn span_end(vm: &mut VM) -> Result<(), String> {
    let otel = vm.otel_mut();
    let id = otel
        .span_stack
        .pop()
        .ok_or("'span_end' called with no active span")?;
    let end = now_ms();
    if let Some(span) = otel.spans.iter_mut().find(|s| s.id == id) {
        span.end_ms = Some(end);
        if span.status == SpanStatus::Unset {
            span.status = SpanStatus::Ok;
        }
    }
    Ok(())
}

// "name" [body] span -> results (auto start/end, catches errors)
fn span_scoped(vm: &mut VM) -> Result<(), String> {
    let body = vm.pop_val()?;
    let name = vm.pop_val()?;
    let name_str = match &name {
        Value::Str(s) => s.clone(),
        Value::Symbol(s) => s.clone(),
        _ => {
            return Err(format!(
                "'span' expects string/symbol name, got {}",
                name.type_name()
            ));
        }
    };

    // Start span
    let otel = vm.otel_mut();
    let id = otel.next_span_id;
    otel.next_span_id += 1;
    let parent_id = otel.span_stack.last().copied();
    otel.spans.push(Span {
        id,
        name: name_str,
        parent_id,
        start_ms: now_ms(),
        end_ms: None,
        attributes: HashMap::new(),
        events: Vec::new(),
        status: SpanStatus::Unset,
    });
    otel.span_stack.push(id);

    // Execute body
    let result = match body {
        Value::Quotation(ops) => vm.run_quotation(&ops),
        _ => Err(format!(
            "'span' expects quotation body, got {}",
            body.type_name()
        )),
    };

    // End span
    let end = now_ms();
    let otel = vm.otel_mut();
    otel.span_stack.pop();
    if let Some(span) = otel.spans.iter_mut().find(|s| s.id == id) {
        span.end_ms = Some(end);
        match &result {
            Ok(()) => {
                if span.status == SpanStatus::Unset {
                    span.status = SpanStatus::Ok;
                }
            }
            Err(e) => {
                span.status = SpanStatus::Error(e.clone());
            }
        }
    }

    result
}

// "key" value span_attr -> ()
fn span_attr(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let key = vm.pop_val()?;
    let key_str = match key {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'span_attr' expects string key, got {}",
                key.type_name()
            ));
        }
    };

    let otel = vm.otel_mut();
    let id = otel
        .span_stack
        .last()
        .ok_or("'span_attr' called with no active span")?;
    let id = *id;
    if let Some(span) = otel.spans.iter_mut().find(|s| s.id == id) {
        span.attributes.insert(key_str, val);
    }
    Ok(())
}

// "name" span_event -> ()
fn span_event_fn(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        Value::Symbol(s) => s,
        _ => {
            return Err(format!(
                "'span_event' expects string name, got {}",
                name.type_name()
            ));
        }
    };

    let otel = vm.otel_mut();
    let id = otel
        .span_stack
        .last()
        .ok_or("'span_event' called with no active span")?;
    let id = *id;
    if let Some(span) = otel.spans.iter_mut().find(|s| s.id == id) {
        span.events.push(SpanEvent {
            name: name_str,
            timestamp_ms: now_ms(),
        });
    }
    Ok(())
}

// "msg" span_error -> () (marks current span as error)
fn span_error(vm: &mut VM) -> Result<(), String> {
    let msg = vm.pop_val()?;
    let msg_str = match msg {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'span_error' expects string, got {}",
                msg.type_name()
            ));
        }
    };

    let otel = vm.otel_mut();
    let id = otel
        .span_stack
        .last()
        .ok_or("'span_error' called with no active span")?;
    let id = *id;
    if let Some(span) = otel.spans.iter_mut().find(|s| s.id == id) {
        span.status = SpanStatus::Error(msg_str.clone());
        span.events.push(SpanEvent {
            name: format!("exception: {}", msg_str),
            timestamp_ms: now_ms(),
        });
    }
    Ok(())
}

// spans_dump -> list of span maps
fn spans_dump(vm: &mut VM) -> Result<(), String> {
    let otel = vm.otel_mut();
    let spans: Vec<Value> = otel
        .spans
        .iter()
        .map(|s| {
            let mut map = HashMap::new();
            map.insert(Value::Str("id".into()), Value::Int(s.id as i64));
            map.insert(Value::Str("name".into()), Value::Str(s.name.clone()));
            map.insert(
                Value::Str("parent_id".into()),
                match s.parent_id {
                    Some(pid) => Value::Int(pid as i64),
                    None => Value::Symbol("null".into()),
                },
            );
            map.insert(Value::Str("start_ms".into()), Value::Int(s.start_ms));
            map.insert(
                Value::Str("end_ms".into()),
                match s.end_ms {
                    Some(t) => Value::Int(t),
                    None => Value::Symbol("null".into()),
                },
            );
            map.insert(
                Value::Str("duration_ms".into()),
                match s.end_ms {
                    Some(t) => Value::Int(t - s.start_ms),
                    None => Value::Symbol("null".into()),
                },
            );
            map.insert(
                Value::Str("status".into()),
                match &s.status {
                    SpanStatus::Ok => Value::Symbol("ok".into()),
                    SpanStatus::Error(msg) => Value::Str(format!("error: {}", msg)),
                    SpanStatus::Unset => Value::Symbol("unset".into()),
                },
            );
            // Attributes as a map
            let attrs: HashMap<Value, Value> = s
                .attributes
                .iter()
                .map(|(k, v)| (Value::Str(k.clone()), v.clone()))
                .collect();
            map.insert(Value::Str("attributes".into()), Value::Map(attrs));
            // Events as a list
            let events: Vec<Value> = s
                .events
                .iter()
                .map(|e| {
                    let mut em = HashMap::new();
                    em.insert(Value::Str("name".into()), Value::Str(e.name.clone()));
                    em.insert(
                        Value::Str("timestamp_ms".into()),
                        Value::Int(e.timestamp_ms),
                    );
                    Value::Map(em)
                })
                .collect();
            map.insert(Value::Str("events".into()), Value::List(events));
            Value::Map(map)
        })
        .collect();
    vm.push_val(Value::List(spans));
    Ok(())
}

// spans_reset -> ()
fn spans_reset(vm: &mut VM) -> Result<(), String> {
    let otel = vm.otel_mut();
    otel.spans.clear();
    otel.span_stack.clear();
    Ok(())
}

// ============================================================
// Metrics builtins
// ============================================================

// "name" counter_inc -> ()
fn counter_inc(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'counter_inc' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Counter(0.0),
        });
    match &mut entry.kind {
        MetricKind::Counter(v) => {
            *v += 1.0;
            Ok(())
        }
        _ => Err(format!("'{}' is not a counter", name_str)),
    }
}

// n "name" counter_add -> ()
fn counter_add(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let n = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'counter_add' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let amount = match n {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(format!(
                "'counter_add' expects number, got {}",
                n.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Counter(0.0),
        });
    match &mut entry.kind {
        MetricKind::Counter(v) => {
            *v += amount;
            Ok(())
        }
        _ => Err(format!("'{}' is not a counter", name_str)),
    }
}

// value "name" gauge_set -> ()
fn gauge_set(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let val = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'gauge_set' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let amount = match val {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(format!(
                "'gauge_set' expects number, got {}",
                val.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Gauge(0.0),
        });
    match &mut entry.kind {
        MetricKind::Gauge(v) => {
            *v = amount;
            Ok(())
        }
        _ => Err(format!("'{}' is not a gauge", name_str)),
    }
}

// "name" gauge_inc -> ()
fn gauge_inc(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'gauge_inc' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Gauge(0.0),
        });
    match &mut entry.kind {
        MetricKind::Gauge(v) => {
            *v += 1.0;
            Ok(())
        }
        _ => Err(format!("'{}' is not a gauge", name_str)),
    }
}

// "name" gauge_dec -> ()
fn gauge_dec(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'gauge_dec' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Gauge(0.0),
        });
    match &mut entry.kind {
        MetricKind::Gauge(v) => {
            *v -= 1.0;
            Ok(())
        }
        _ => Err(format!("'{}' is not a gauge", name_str)),
    }
}

// value "name" histogram_record -> ()
fn histogram_record(vm: &mut VM) -> Result<(), String> {
    let name = vm.pop_val()?;
    let val = vm.pop_val()?;
    let name_str = match name {
        Value::Str(s) => s,
        _ => {
            return Err(format!(
                "'histogram_record' expects string name, got {}",
                name.type_name()
            ));
        }
    };
    let amount = match val {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(format!(
                "'histogram_record' expects number, got {}",
                val.type_name()
            ));
        }
    };
    let otel = vm.otel_mut();
    let entry = otel
        .metrics
        .entry(name_str.clone())
        .or_insert_with(|| MetricEntry {
            kind: MetricKind::Histogram(HistogramData::new()),
        });
    match &mut entry.kind {
        MetricKind::Histogram(h) => {
            h.record(amount);
            Ok(())
        }
        _ => Err(format!("'{}' is not a histogram", name_str)),
    }
}

// metrics_dump -> map of metric maps
fn metrics_dump(vm: &mut VM) -> Result<(), String> {
    let otel = vm.otel_mut();
    let mut result = HashMap::new();
    for (name, entry) in &otel.metrics {
        let mut m = HashMap::new();
        match &entry.kind {
            MetricKind::Counter(v) => {
                m.insert(Value::Str("type".into()), Value::Symbol("counter".into()));
                m.insert(Value::Str("value".into()), Value::Float(*v));
            }
            MetricKind::Gauge(v) => {
                m.insert(Value::Str("type".into()), Value::Symbol("gauge".into()));
                m.insert(Value::Str("value".into()), Value::Float(*v));
            }
            MetricKind::Histogram(h) => {
                m.insert(Value::Str("type".into()), Value::Symbol("histogram".into()));
                m.insert(Value::Str("count".into()), Value::Int(h.count as i64));
                m.insert(Value::Str("sum".into()), Value::Float(h.sum));
                m.insert(
                    Value::Str("min".into()),
                    Value::Float(if h.count == 0 { 0.0 } else { h.min }),
                );
                m.insert(
                    Value::Str("max".into()),
                    Value::Float(if h.count == 0 { 0.0 } else { h.max }),
                );
                m.insert(
                    Value::Str("avg".into()),
                    Value::Float(if h.count == 0 {
                        0.0
                    } else {
                        h.sum / h.count as f64
                    }),
                );
            }
        }
        result.insert(Value::Str(name.clone()), Value::Map(m));
    }
    vm.push_val(Value::Map(result));
    Ok(())
}

// metrics_reset -> ()
fn metrics_reset(vm: &mut VM) -> Result<(), String> {
    vm.otel_mut().metrics.clear();
    Ok(())
}

// ============================================================
// Logging builtins
// ============================================================

fn log_at_level(vm: &mut VM, level: LogLevel) -> Result<(), String> {
    let msg = vm.pop_val()?;
    let msg_str = match msg {
        Value::Str(s) => s,
        other => format!("{}", other),
    };

    let otel = vm.otel_mut();
    if level < otel.min_log_level {
        return Ok(());
    }
    let span_id = otel.span_stack.last().copied();

    // Print to stderr
    let ts = now_ms();
    eprintln!("{} [{}] {}", ts, level.as_str(), msg_str);

    otel.logs.push(LogEntry {
        timestamp_ms: ts,
        level,
        message: msg_str,
        fields: HashMap::new(),
        span_id,
    });
    Ok(())
}

fn log_debug(vm: &mut VM) -> Result<(), String> {
    log_at_level(vm, LogLevel::Debug)
}
fn log_info(vm: &mut VM) -> Result<(), String> {
    log_at_level(vm, LogLevel::Info)
}
fn log_warn(vm: &mut VM) -> Result<(), String> {
    log_at_level(vm, LogLevel::Warn)
}
fn log_error(vm: &mut VM) -> Result<(), String> {
    log_at_level(vm, LogLevel::Error)
}

// "msg" #{fields} :level log_with -> ()
fn log_with(vm: &mut VM) -> Result<(), String> {
    let level_val = vm.pop_val()?;
    let fields_val = vm.pop_val()?;
    let msg = vm.pop_val()?;

    let level = LogLevel::from_value(&level_val)?;
    let msg_str = match msg {
        Value::Str(s) => s,
        other => format!("{}", other),
    };
    let fields: HashMap<String, Value> = match fields_val {
        Value::Map(m) => m
            .into_iter()
            .map(|(k, v)| {
                (
                    match k {
                        Value::Str(s) => s,
                        other => format!("{}", other),
                    },
                    v,
                )
            })
            .collect(),
        _ => {
            return Err(format!(
                "'log_with' expects map of fields, got {}",
                fields_val.type_name()
            ));
        }
    };

    let otel = vm.otel_mut();
    if level < otel.min_log_level {
        return Ok(());
    }
    let span_id = otel.span_stack.last().copied();
    let ts = now_ms();

    // Print structured log to stderr
    let field_strs: Vec<String> = fields.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
    let fields_part = if field_strs.is_empty() {
        String::new()
    } else {
        format!(" {}", field_strs.join(" "))
    };
    eprintln!("{} [{}] {}{}", ts, level.as_str(), msg_str, fields_part);

    otel.logs.push(LogEntry {
        timestamp_ms: ts,
        level,
        message: msg_str,
        fields,
        span_id,
    });
    Ok(())
}

// :level log_level -> ()
fn log_level(vm: &mut VM) -> Result<(), String> {
    let level_val = vm.pop_val()?;
    let level = LogLevel::from_value(&level_val)?;
    vm.otel_mut().min_log_level = level;
    Ok(())
}

// logs_dump -> list of log entry maps
fn logs_dump(vm: &mut VM) -> Result<(), String> {
    let otel = vm.otel_mut();
    let entries: Vec<Value> = otel
        .logs
        .iter()
        .map(|entry| {
            let mut m = HashMap::new();
            m.insert(
                Value::Str("timestamp_ms".into()),
                Value::Int(entry.timestamp_ms),
            );
            m.insert(
                Value::Str("level".into()),
                Value::Symbol(entry.level.as_str().into()),
            );
            m.insert(
                Value::Str("message".into()),
                Value::Str(entry.message.clone()),
            );
            m.insert(
                Value::Str("span_id".into()),
                match entry.span_id {
                    Some(id) => Value::Int(id as i64),
                    None => Value::Symbol("null".into()),
                },
            );
            let fields: HashMap<Value, Value> = entry
                .fields
                .iter()
                .map(|(k, v)| (Value::Str(k.clone()), v.clone()))
                .collect();
            m.insert(Value::Str("fields".into()), Value::Map(fields));
            Value::Map(m)
        })
        .collect();
    vm.push_val(Value::List(entries));
    Ok(())
}

// logs_reset -> ()
fn logs_reset(vm: &mut VM) -> Result<(), String> {
    vm.otel_mut().logs.clear();
    Ok(())
}

// ============================================================
// Combined export
// ============================================================

// otel_dump -> #{"spans" => [...], "metrics" => #{...}, "logs" => [...]}
fn otel_dump(vm: &mut VM) -> Result<(), String> {
    // Build spans
    spans_dump(vm)?;
    let spans = vm.pop_val()?;
    // Build metrics
    metrics_dump(vm)?;
    let metrics = vm.pop_val()?;
    // Build logs
    logs_dump(vm)?;
    let logs = vm.pop_val()?;

    let mut result = HashMap::new();
    result.insert(Value::Str("spans".into()), spans);
    result.insert(Value::Str("metrics".into()), metrics);
    result.insert(Value::Str("logs".into()), logs);
    vm.push_val(Value::Map(result));
    Ok(())
}

// otel_reset -> ()
fn otel_reset(vm: &mut VM) -> Result<(), String> {
    spans_reset(vm)?;
    metrics_reset(vm)?;
    logs_reset(vm)?;
    Ok(())
}

// ============================================================
// Registration
// ============================================================

pub fn register(vm: &mut VM) {
    // Tracing
    vm.register_builtin("span_start", span_start);
    vm.register_builtin("span_end", span_end);
    vm.register_builtin("span", span_scoped);
    vm.register_builtin("span_attr", span_attr);
    vm.register_builtin("span_event", span_event_fn);
    vm.register_builtin("span_error", span_error);
    vm.register_builtin("spans_dump", spans_dump);
    vm.register_builtin("spans_reset", spans_reset);

    // Metrics
    vm.register_builtin("counter_inc", counter_inc);
    vm.register_builtin("counter_add", counter_add);
    vm.register_builtin("gauge_set", gauge_set);
    vm.register_builtin("gauge_inc", gauge_inc);
    vm.register_builtin("gauge_dec", gauge_dec);
    vm.register_builtin("histogram_record", histogram_record);
    vm.register_builtin("metrics_dump", metrics_dump);
    vm.register_builtin("metrics_reset", metrics_reset);

    // Logging
    vm.register_builtin("log_debug", log_debug);
    vm.register_builtin("log_info", log_info);
    vm.register_builtin("log_warn", log_warn);
    vm.register_builtin("log_error", log_error);
    vm.register_builtin("log_with", log_with);
    vm.register_builtin("log_level", log_level);
    vm.register_builtin("logs_dump", logs_dump);
    vm.register_builtin("logs_reset", logs_reset);

    // Combined
    vm.register_builtin("otel_dump", otel_dump);
    vm.register_builtin("otel_reset", otel_reset);
}
