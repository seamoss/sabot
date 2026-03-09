// WebSocket support for Sabot (RFC 6455).
// Client and server-side WebSocket connections over plain TCP.
// No TLS (wss://) support — only ws:// connections.

use crate::opcode::Op;
use crate::value::Value;
use crate::vm::VM;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

// ============================================================
// Data types
// ============================================================

#[derive(Debug)]
pub struct WsConn {
    pub stream: TcpStream,
    pub is_client: bool, // clients mask frames per RFC 6455
}

#[derive(Clone, Debug)]
pub struct WsRoute {
    pub path: String,
    pub handler: Vec<Op>, // handler quotation ops
}

/// Shared connection registry type alias (avoids clippy::type_complexity).
pub type WsConnMap = Arc<Mutex<HashMap<u64, Arc<Mutex<WsConn>>>>>;

// ============================================================
// SHA-1 (RFC 3174) — inline, no dependencies
// ============================================================

fn sha1(data: &[u8]) -> [u8; 20] {
    fn left_rotate(value: u32, count: u32) -> u32 {
        value.rotate_left(count)
    }

    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let orig_len_bits = (data.len() as u64) * 8;

    // Pre-processing: pad message
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&orig_len_bits.to_be_bytes());

    // Process each 512-bit (64-byte) chunk
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = left_rotate(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], 1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);

        #[allow(clippy::needless_range_loop)]
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };

            let temp = left_rotate(a, 5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = left_rotate(b, 30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut result = [0u8; 20];
    result[0..4].copy_from_slice(&h0.to_be_bytes());
    result[4..8].copy_from_slice(&h1.to_be_bytes());
    result[8..12].copy_from_slice(&h2.to_be_bytes());
    result[12..16].copy_from_slice(&h3.to_be_bytes());
    result[16..20].copy_from_slice(&h4.to_be_bytes());
    result
}

// ============================================================
// Base64 encoding
// ============================================================

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }
    result
}

// ============================================================
// WebSocket key generation and validation
// ============================================================

const WS_MAGIC: &str = "258EAFA5-E914-47DA-95CA-5AB9AA10ABE4";

/// Simple xorshift64 PRNG seeded from system time nanoseconds.
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn generate_ws_key() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut state = nanos ^ 0xDEAD_BEEF_CAFE_BABEu64;
    let mut key_bytes = [0u8; 16];
    for chunk in key_bytes.chunks_mut(8) {
        let val = xorshift64(&mut state);
        let bytes = val.to_le_bytes();
        for (i, b) in chunk.iter_mut().enumerate() {
            *b = bytes[i];
        }
    }
    base64_encode(&key_bytes)
}

fn ws_accept_key(client_key: &str) -> String {
    let combined = format!("{}{}", client_key.trim(), WS_MAGIC);
    let hash = sha1(combined.as_bytes());
    base64_encode(&hash)
}

// ============================================================
// WebSocket frame codec
// ============================================================

#[derive(Debug, PartialEq)]
enum WsOpcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl WsOpcode {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x0 => Some(WsOpcode::Continuation),
            0x1 => Some(WsOpcode::Text),
            0x2 => Some(WsOpcode::Binary),
            0x8 => Some(WsOpcode::Close),
            0x9 => Some(WsOpcode::Ping),
            0xA => Some(WsOpcode::Pong),
            _ => None,
        }
    }

    fn to_byte(&self) -> u8 {
        match self {
            WsOpcode::Continuation => 0x0,
            WsOpcode::Text => 0x1,
            WsOpcode::Binary => 0x2,
            WsOpcode::Close => 0x8,
            WsOpcode::Ping => 0x9,
            WsOpcode::Pong => 0xA,
        }
    }
}

struct WsFrame {
    fin: bool,
    opcode: WsOpcode,
    payload: Vec<u8>,
}

/// Read a single WebSocket frame from a stream.
fn read_frame(stream: &mut TcpStream) -> Result<WsFrame, String> {
    let mut header = [0u8; 2];
    stream
        .read_exact(&mut header)
        .map_err(|e| format!("ws read error: {}", e))?;

    let fin = (header[0] & 0x80) != 0;
    let opcode_byte = header[0] & 0x0F;
    let opcode = WsOpcode::from_byte(opcode_byte)
        .ok_or_else(|| format!("ws unknown opcode: 0x{:02x}", opcode_byte))?;

    let masked = (header[1] & 0x80) != 0;
    let len_byte = header[1] & 0x7F;

    let payload_len: usize = if len_byte < 126 {
        len_byte as usize
    } else if len_byte == 126 {
        let mut buf = [0u8; 2];
        stream
            .read_exact(&mut buf)
            .map_err(|e| format!("ws read error: {}", e))?;
        u16::from_be_bytes(buf) as usize
    } else {
        // len_byte == 127
        let mut buf = [0u8; 8];
        stream
            .read_exact(&mut buf)
            .map_err(|e| format!("ws read error: {}", e))?;
        u64::from_be_bytes(buf) as usize
    };

    let mask_key = if masked {
        let mut buf = [0u8; 4];
        stream
            .read_exact(&mut buf)
            .map_err(|e| format!("ws read error: {}", e))?;
        Some(buf)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream
            .read_exact(&mut payload)
            .map_err(|e| format!("ws read error: {}", e))?;
    }

    // Unmask if needed
    if let Some(key) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }

    Ok(WsFrame {
        fin,
        opcode,
        payload,
    })
}

/// Write a WebSocket frame to a stream.
/// If `mask` is true, applies random masking (required for client->server frames).
fn write_frame(
    stream: &mut TcpStream,
    opcode: &WsOpcode,
    payload: &[u8],
    mask: bool,
) -> Result<(), String> {
    let mut header = Vec::new();

    // FIN=1, opcode
    header.push(0x80 | opcode.to_byte());

    // Mask bit + payload length
    let mask_bit: u8 = if mask { 0x80 } else { 0x00 };
    let len = payload.len();
    if len < 126 {
        header.push(mask_bit | (len as u8));
    } else if len <= 0xFFFF {
        header.push(mask_bit | 126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(mask_bit | 127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Masking key + masked payload
    if mask {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut state = nanos ^ 0xF00D_FACE_1234_5678u64;
        let key_val = xorshift64(&mut state);
        let mask_key = (key_val as u32).to_be_bytes();

        header.extend_from_slice(&mask_key);

        stream
            .write_all(&header)
            .map_err(|e| format!("ws write error: {}", e))?;

        let mut masked_payload = payload.to_vec();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }
        stream
            .write_all(&masked_payload)
            .map_err(|e| format!("ws write error: {}", e))?;
    } else {
        stream
            .write_all(&header)
            .map_err(|e| format!("ws write error: {}", e))?;
        stream
            .write_all(payload)
            .map_err(|e| format!("ws write error: {}", e))?;
    }

    stream
        .flush()
        .map_err(|e| format!("ws flush error: {}", e))?;
    Ok(())
}

/// Send a pong frame in response to a ping.
fn send_pong(stream: &mut TcpStream, payload: &[u8], mask: bool) -> Result<(), String> {
    write_frame(stream, &WsOpcode::Pong, payload, mask)
}

// ============================================================
// URL parsing
// ============================================================

struct WsUrl {
    host: String,
    port: u16,
    path: String,
}

fn parse_ws_url(url: &str) -> Result<WsUrl, String> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| format!("'ws_connect' expects ws:// URL, got \"{}\"", url))?;

    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(idx) => {
            let h = &host_port[..idx];
            let p = host_port[idx + 1..]
                .parse::<u16>()
                .map_err(|_| format!("'ws_connect' invalid port in URL: \"{}\"", url))?;
            (h.to_string(), p)
        }
        None => (host_port.to_string(), 80),
    };

    Ok(WsUrl {
        host,
        port,
        path: path.to_string(),
    })
}

// ============================================================
// Connection registry helpers
// ============================================================

fn get_conn(conns: &WsConnMap, id: u64) -> Result<Arc<Mutex<WsConn>>, String> {
    let map = conns
        .lock()
        .map_err(|_| "ws connection registry lock poisoned".to_string())?;
    map.get(&id)
        .cloned()
        .ok_or_else(|| format!("ws connection {} not found", id))
}

fn alloc_id(next_id: &Arc<Mutex<u64>>) -> Result<u64, String> {
    let mut id = next_id
        .lock()
        .map_err(|_| "ws id lock poisoned".to_string())?;
    let current = *id;
    *id += 1;
    Ok(current)
}

fn register_conn(conns: &WsConnMap, id: u64, conn: WsConn) -> Result<(), String> {
    let mut map = conns
        .lock()
        .map_err(|_| "ws connection registry lock poisoned".to_string())?;
    map.insert(id, Arc::new(Mutex::new(conn)));
    Ok(())
}

fn remove_conn(conns: &WsConnMap, id: u64) -> Result<(), String> {
    let mut map = conns
        .lock()
        .map_err(|_| "ws connection registry lock poisoned".to_string())?;
    map.remove(&id);
    Ok(())
}

// ============================================================
// Client builtins
// ============================================================

/// `"ws://host:port/path" ws_connect` -> conn_id (int)
fn ws_connect(vm: &mut VM) -> Result<(), String> {
    let url_val = vm.pop_val()?;
    let url = match &url_val {
        Value::Str(s) => s.clone(),
        other => {
            return Err(format!(
                "'ws_connect' expects a string URL, got {}",
                other.type_name()
            ));
        }
    };

    let parsed = parse_ws_url(&url)?;
    let addr = format!("{}:{}", parsed.host, parsed.port);

    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("'ws_connect' TCP connect to {} failed: {}", addr, e))?;

    // Send HTTP upgrade request
    let key = generate_ws_key();
    let request = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n",
        parsed.path, parsed.host, key
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("'ws_connect' write handshake failed: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("'ws_connect' flush failed: {}", e))?;

    // Read HTTP response (read until \r\n\r\n)
    let mut response_buf = Vec::new();
    let mut tmp = [0u8; 1];
    loop {
        stream
            .read_exact(&mut tmp)
            .map_err(|e| format!("'ws_connect' read handshake response failed: {}", e))?;
        response_buf.push(tmp[0]);
        if response_buf.len() >= 4
            && response_buf[response_buf.len() - 4..] == [b'\r', b'\n', b'\r', b'\n']
        {
            break;
        }
        if response_buf.len() > 8192 {
            return Err("'ws_connect' handshake response too large".to_string());
        }
    }

    let response_str = String::from_utf8_lossy(&response_buf);

    // Verify 101 status
    let first_line = response_str
        .lines()
        .next()
        .ok_or("'ws_connect' empty handshake response")?;
    if !first_line.contains("101") {
        return Err(format!(
            "'ws_connect' expected 101 Switching Protocols, got: {}",
            first_line
        ));
    }

    // Verify Sec-WebSocket-Accept
    let expected_accept = ws_accept_key(&key);
    let mut found_accept = false;
    for line in response_str.lines() {
        if let Some(val) = line.strip_prefix("Sec-WebSocket-Accept:") {
            if val.trim() == expected_accept {
                found_accept = true;
                break;
            } else {
                return Err(format!(
                    "'ws_connect' accept key mismatch: expected {}, got {}",
                    expected_accept,
                    val.trim()
                ));
            }
        }
    }
    if !found_accept {
        return Err("'ws_connect' missing Sec-WebSocket-Accept header".to_string());
    }

    // Register connection
    let id = alloc_id(&vm.next_ws_id)?;
    let conn = WsConn {
        stream,
        is_client: true,
    };
    register_conn(&vm.ws_connections, id, conn)?;

    vm.push_val(Value::Int(id as i64));
    Ok(())
}

/// `"message" conn_id ws_send` -> ()
fn ws_send(vm: &mut VM) -> Result<(), String> {
    let id_val = vm.pop_val()?;
    let msg_val = vm.pop_val()?;

    let id = match &id_val {
        Value::Int(n) => *n as u64,
        other => {
            return Err(format!(
                "'ws_send' expects an int conn_id, got {}",
                other.type_name()
            ));
        }
    };

    let msg = match &msg_val {
        Value::Str(s) => s.clone(),
        other => {
            return Err(format!(
                "'ws_send' expects a string message, got {}",
                other.type_name()
            ));
        }
    };

    let conn_arc = get_conn(&vm.ws_connections, id)?;
    let mut conn = conn_arc
        .lock()
        .map_err(|_| "ws connection lock poisoned".to_string())?;

    let mask = conn.is_client;
    write_frame(&mut conn.stream, &WsOpcode::Text, msg.as_bytes(), mask)?;

    Ok(())
}

/// `conn_id ws_recv` -> string message or :close symbol
fn ws_recv(vm: &mut VM) -> Result<(), String> {
    let id_val = vm.pop_val()?;
    let id = match &id_val {
        Value::Int(n) => *n as u64,
        other => {
            return Err(format!(
                "'ws_recv' expects an int conn_id, got {}",
                other.type_name()
            ));
        }
    };

    let conn_arc = get_conn(&vm.ws_connections, id)?;

    loop {
        let mut conn = conn_arc
            .lock()
            .map_err(|_| "ws connection lock poisoned".to_string())?;

        let frame = read_frame(&mut conn.stream)?;

        match frame.opcode {
            WsOpcode::Text | WsOpcode::Binary => {
                if frame.fin {
                    let text = String::from_utf8(frame.payload)
                        .map_err(|e| format!("ws_recv invalid UTF-8: {}", e))?;
                    vm.push_val(Value::Str(text));
                    return Ok(());
                }
                // Non-FIN text/binary: accumulate fragments
                let mut payload = frame.payload;
                loop {
                    let cont = read_frame(&mut conn.stream)?;
                    payload.extend_from_slice(&cont.payload);
                    if cont.fin {
                        break;
                    }
                }
                let text = String::from_utf8(payload)
                    .map_err(|e| format!("ws_recv invalid UTF-8: {}", e))?;
                vm.push_val(Value::Str(text));
                return Ok(());
            }
            WsOpcode::Ping => {
                // Auto-respond with pong, then continue reading
                let mask = conn.is_client;
                send_pong(&mut conn.stream, &frame.payload, mask)?;
                continue;
            }
            WsOpcode::Pong => {
                // Ignore unsolicited pongs, continue reading
                continue;
            }
            WsOpcode::Close => {
                vm.push_val(Value::Symbol("close".to_string()));
                return Ok(());
            }
            WsOpcode::Continuation => {
                // Stray continuation without a starting frame — skip
                continue;
            }
        }
    }
}

/// `conn_id ws_close` -> ()
fn ws_close(vm: &mut VM) -> Result<(), String> {
    let id_val = vm.pop_val()?;
    let id = match &id_val {
        Value::Int(n) => *n as u64,
        other => {
            return Err(format!(
                "'ws_close' expects an int conn_id, got {}",
                other.type_name()
            ));
        }
    };

    // Send close frame
    let conn_arc = get_conn(&vm.ws_connections, id);
    if let Ok(arc) = conn_arc
        && let Ok(mut conn) = arc.lock()
    {
        let mask = conn.is_client;
        let _ = write_frame(&mut conn.stream, &WsOpcode::Close, &[], mask);
    }

    remove_conn(&vm.ws_connections, id)?;
    Ok(())
}

/// `conn_id ws_status` -> :open or :closed
fn ws_status(vm: &mut VM) -> Result<(), String> {
    let id_val = vm.pop_val()?;
    let id = match &id_val {
        Value::Int(n) => *n as u64,
        other => {
            return Err(format!(
                "'ws_status' expects an int conn_id, got {}",
                other.type_name()
            ));
        }
    };

    let is_open = {
        let conns = vm
            .ws_connections
            .lock()
            .map_err(|_| "ws connection registry lock poisoned".to_string())?;
        conns.contains_key(&id)
    };

    if is_open {
        vm.push_val(Value::Symbol("open".to_string()));
    } else {
        vm.push_val(Value::Symbol("closed".to_string()));
    }
    Ok(())
}

// ============================================================
// Server-side support
// ============================================================

/// `[handler] "/ws/path" ws_route` -> registers a WebSocket route
fn ws_route_builtin(vm: &mut VM) -> Result<(), String> {
    let path_val = vm.pop_val()?;
    let handler_val = vm.pop_val()?;

    let path = match &path_val {
        Value::Str(s) => s.clone(),
        other => {
            return Err(format!(
                "'ws_route' expects a string path, got {}",
                other.type_name()
            ));
        }
    };

    let handler = match handler_val {
        Value::Quotation(ops) => ops,
        other => {
            return Err(format!(
                "'ws_route' expects a quotation handler, got {}",
                other.type_name()
            ));
        }
    };

    let route = WsRoute { path, handler };
    let mut routes = vm
        .ws_routes
        .lock()
        .map_err(|_| "ws routes lock poisoned".to_string())?;
    routes.push(route);
    Ok(())
}

/// Check if an HTTP request is a WebSocket upgrade request.
/// Returns the Sec-WebSocket-Key if it is.
pub fn check_ws_upgrade(headers: &HashMap<String, String>) -> Option<String> {
    // Check for Upgrade: websocket header (case-insensitive values)
    let has_upgrade = headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("upgrade") && v.eq_ignore_ascii_case("websocket"));

    if !has_upgrade {
        return None;
    }

    // Check Connection: Upgrade
    let has_connection = headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("connection") && v.to_lowercase().contains("upgrade"));

    if !has_connection {
        return None;
    }

    // Extract Sec-WebSocket-Key
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-key"))
        .map(|(_, v)| v.clone())
}

/// Perform server-side WebSocket handshake on an existing TCP stream.
/// Sends the 101 response, registers the connection, returns the conn_id.
pub fn ws_upgrade(
    mut stream: TcpStream,
    key: &str,
    ws_conns: &WsConnMap,
    next_id: &Arc<Mutex<u64>>,
) -> Result<u64, String> {
    let accept = ws_accept_key(key);

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        accept
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("ws_upgrade write failed: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("ws_upgrade flush failed: {}", e))?;

    let id = alloc_id(next_id)?;
    let conn = WsConn {
        stream,
        is_client: false, // server side: don't mask frames
    };
    register_conn(ws_conns, id, conn)?;

    Ok(id)
}

// ============================================================
// Registration
// ============================================================

pub fn register(vm: &mut VM) {
    vm.register_builtin("ws_connect", ws_connect);
    vm.register_builtin("ws_send", ws_send);
    vm.register_builtin("ws_recv", ws_recv);
    vm.register_builtin("ws_close", ws_close);
    vm.register_builtin("ws_status", ws_status);
    vm.register_builtin("ws_route", ws_route_builtin);
}
