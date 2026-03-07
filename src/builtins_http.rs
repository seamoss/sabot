//! HTTP server support for Sabo.
//! Provides request parsing, response serialization, route matching,
//! static file serving, and the connection handler.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::opcode::Op;
use crate::value::Value;

// ---- Route ----

#[derive(Clone)]
pub struct Route {
    pub method: String,
    pub pattern: String,
    pub segments: Vec<Segment>,
    pub handler: Vec<Op>,
}

#[derive(Clone)]
pub enum Segment {
    Literal(String),
    Param(String),
}

pub fn compile_pattern(pattern: &str) -> Vec<Segment> {
    pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(name) = s.strip_prefix(':') {
                Segment::Param(name.to_string())
            } else {
                Segment::Literal(s.to_string())
            }
        })
        .collect()
}

fn match_route(segments: &[Segment], path_parts: &[&str]) -> Option<Vec<(String, String)>> {
    if segments.len() != path_parts.len() {
        return None;
    }
    let mut params = Vec::new();
    for (seg, part) in segments.iter().zip(path_parts.iter()) {
        match seg {
            Segment::Literal(s) => {
                if s != part {
                    return None;
                }
            }
            Segment::Param(name) => {
                params.push((name.clone(), part.to_string()));
            }
        }
    }
    Some(params)
}

// ---- Static dir ----

#[derive(Clone)]
pub struct StaticDir {
    pub url_prefix: String,
    pub fs_path: String,
}

// ---- Shared types ----

pub type Routes = Arc<Mutex<Vec<Route>>>;
pub type StaticDirs = Arc<Mutex<Vec<StaticDir>>>;

// ---- HTTP Request Parsing ----

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub fn parse_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(reader_stream);

    // Request line
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| e.to_string())?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("Malformed request line".into());
    }

    let method = parts[0].to_uppercase();
    let raw_path = parts[1];

    let (path, query) = if let Some(idx) = raw_path.find('?') {
        (
            raw_path[..idx].to_string(),
            parse_query_string(&raw_path[idx + 1..]),
        )
    } else {
        (raw_path.to_string(), HashMap::new())
    };

    // Headers
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(idx) = trimmed.find(':') {
            let key = trimmed[..idx].trim().to_lowercase();
            let val = trimmed[idx + 1..].trim().to_string();
            headers.insert(key, val);
        }
    }

    // Body
    let body = if let Some(len_str) = headers.get("content-length") {
        if let Ok(len) = len_str.parse::<usize>() {
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn parse_query_string(qs: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in qs.split('&') {
        if let Some(idx) = pair.find('=') {
            let key = url_decode(&pair[..idx]);
            let val = url_decode(&pair[idx + 1..]);
            map.insert(key, val);
        } else if !pair.is_empty() {
            map.insert(url_decode(pair), String::new());
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                result.push(byte as char);
                continue;
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

// ---- HTTP Response ----

pub fn write_response(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    content_type: &str,
    extra_headers: &[(String, String)],
    body: &[u8],
) {
    let mut resp = format!("HTTP/1.1 {} {}\r\n", status, status_text);
    resp.push_str(&format!("Content-Type: {}\r\n", content_type));
    resp.push_str(&format!("Content-Length: {}\r\n", body.len()));
    resp.push_str("Connection: close\r\n");
    for (k, v) in extra_headers {
        resp.push_str(&format!("{}: {}\r\n", k, v));
    }
    resp.push_str("\r\n");
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

pub fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

// ---- MIME Types ----

fn mime_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

// ---- Convert request to Sabo value map ----

fn request_to_value(req: &HttpRequest, params: &[(String, String)]) -> Value {
    let mut map: HashMap<Value, Value> = HashMap::new();

    map.insert(Value::Str("method".into()), Value::Str(req.method.clone()));
    map.insert(Value::Str("path".into()), Value::Str(req.path.clone()));
    map.insert(Value::Str("body".into()), Value::Str(req.body.clone()));

    let query_map: HashMap<Value, Value> = req
        .query
        .iter()
        .map(|(k, v)| (Value::Str(k.clone()), Value::Str(v.clone())))
        .collect();
    map.insert(Value::Str("query".into()), Value::Map(query_map));

    let header_map: HashMap<Value, Value> = req
        .headers
        .iter()
        .map(|(k, v)| (Value::Str(k.clone()), Value::Str(v.clone())))
        .collect();
    map.insert(Value::Str("headers".into()), Value::Map(header_map));

    let param_map: HashMap<Value, Value> = params
        .iter()
        .map(|(k, v)| (Value::Str(k.clone()), Value::Str(v.clone())))
        .collect();
    map.insert(Value::Str("params".into()), Value::Map(param_map));

    Value::Map(map)
}

// ---- Extract response from Sabo value ----

struct SaboResponse {
    status: u16,
    content_type: String,
    headers: Vec<(String, String)>,
    body: String,
}

fn value_to_response(val: &Value) -> SaboResponse {
    let mut resp = SaboResponse {
        status: 200,
        content_type: "text/plain; charset=utf-8".into(),
        headers: Vec::new(),
        body: String::new(),
    };

    match val {
        Value::Map(pairs) => {
            for (k, v) in pairs {
                if let Value::Str(key) = k {
                    match key.as_str() {
                        "status" => {
                            if let Value::Int(n) = v {
                                resp.status = *n as u16;
                            }
                        }
                        "body" => {
                            resp.body = match v {
                                Value::Str(s) => s.clone(),
                                other => format!("{}", other),
                            };
                        }
                        "content_type" => {
                            if let Value::Str(s) = v {
                                resp.content_type = s.clone();
                            }
                        }
                        "headers" => {
                            if let Value::Map(hpairs) = v {
                                for (hk, hv) in hpairs {
                                    if let (Value::Str(k), Value::Str(v)) = (hk, hv) {
                                        resp.headers.push((k.clone(), v.clone()));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Value::Str(s) => {
            resp.body = s.clone();
        }
        other => {
            resp.body = format!("{}", other);
        }
    }

    resp
}

// ---- Static file serving ----

fn try_serve_static(req: &HttpRequest, static_dirs: &[StaticDir], stream: &mut TcpStream) -> bool {
    if req.method != "GET" {
        return false;
    }

    for sd in static_dirs {
        let prefix = sd.url_prefix.trim_end_matches('/');
        if let Some(rest) = req.path.strip_prefix(prefix) {
            let rest = if rest.is_empty() { "/" } else { rest };
            let rest = rest.trim_start_matches('/');

            // Prevent directory traversal
            if rest.contains("..") {
                continue;
            }

            let file_path = if rest.is_empty() {
                format!("{}/index.html", sd.fs_path)
            } else {
                format!("{}/{}", sd.fs_path, rest)
            };

            if let Ok(contents) = std::fs::read(&file_path) {
                let ct = mime_type(&file_path);
                write_response(stream, 200, "OK", ct, &[], &contents);
                return true;
            }

            // Try index.html for directory paths
            let index_path = format!("{}/index.html", file_path.trim_end_matches('/'));
            if let Ok(contents) = std::fs::read(&index_path) {
                write_response(
                    stream,
                    200,
                    "OK",
                    "text/html; charset=utf-8",
                    &[],
                    &contents,
                );
                return true;
            }
        }
    }
    false
}

// ---- Public: handle a single HTTP connection ----

pub fn handle_connection(
    mut stream: TcpStream,
    routes: &[Route],
    static_dirs: &[StaticDir],
    vm_template: &crate::vm::VM,
) {
    let start = Instant::now();

    let req = match parse_request(&mut stream) {
        Ok(r) => r,
        Err(_) => {
            write_response(
                &mut stream,
                400,
                "Bad Request",
                "text/plain",
                &[],
                b"Bad Request",
            );
            return;
        }
    };

    let method = req.method.clone();
    let path = req.path.clone();

    // Static files first
    if try_serve_static(&req, static_dirs, &mut stream) {
        log_request(&method, &path, 200, "static", start);
        return;
    }

    // Route matching
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for route in routes {
        if route.method != method && route.method != "*" {
            continue;
        }
        if let Some(params) = match_route(&route.segments, &path_parts) {
            let req_val = request_to_value(&req, &params);

            let mut child = vm_template.spawn_child();
            child.push_val(req_val);

            match child.run(route.handler.clone()) {
                Ok(()) => match child.pop_val() {
                    Ok(val) => {
                        let resp = value_to_response(&val);
                        let body_len = resp.body.len();
                        write_response(
                            &mut stream,
                            resp.status,
                            status_text(resp.status),
                            &resp.content_type,
                            &resp.headers,
                            resp.body.as_bytes(),
                        );
                        log_request(
                            &method,
                            &path,
                            resp.status,
                            &format!("{} bytes", body_len),
                            start,
                        );
                    }
                    Err(_) => {
                        write_response(
                            &mut stream,
                            500,
                            "Internal Server Error",
                            "text/plain",
                            &[],
                            b"Handler returned no response",
                        );
                        log_request(&method, &path, 500, "no response", start);
                    }
                },
                Err(e) => {
                    let msg = format!("500 Internal Server Error\n\n{}", e);
                    write_response(
                        &mut stream,
                        500,
                        "Internal Server Error",
                        "text/plain",
                        &[],
                        msg.as_bytes(),
                    );
                    log_request(&method, &path, 500, &e, start);
                }
            }
            return;
        }
    }

    // 404
    write_response(
        &mut stream,
        404,
        "Not Found",
        "text/plain; charset=utf-8",
        &[],
        b"404 Not Found",
    );
    log_request(&method, &path, 404, "", start);
}

fn log_request(method: &str, path: &str, status: u16, detail: &str, start: Instant) {
    let us = start.elapsed().as_micros();
    let ms = us / 1000;
    let frac = (us % 1000) / 100;
    if detail.is_empty() {
        println!("  {} {} -> {} [{}.{}ms]", method, path, status, ms, frac);
    } else {
        println!(
            "  {} {} -> {} [{}.{}ms] {}",
            method, path, status, ms, frac, detail
        );
    }
}
