// Serialization builtins: JSON, YAML, TOML, and binary (protobuf-style) encoding.
// All text formats share a common Value <-> serde conversion layer.

use std::collections::HashMap;
use crate::value::Value;
use crate::vm::VM;

// ============================================================
// Shared: Sabo Value <-> serde_json::Value conversion
// ============================================================

fn sabo_to_serde(val: &Value) -> serde_json::Value {
    match val {
        Value::Int(n) => serde_json::Value::Number((*n).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Symbol(s) if s == "null" => serde_json::Value::Null,
        Value::Symbol(s) => serde_json::Value::String(s.clone()),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(items) => {
            serde_json::Value::Array(items.iter().map(sabo_to_serde).collect())
        }
        Value::Map(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map.iter()
                .map(|(k, v)| (key_to_string(k), sabo_to_serde(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::Quotation(_) => serde_json::Value::Null,
    }
}

fn serde_to_sabo(val: serde_json::Value) -> Value {
    match val {
        serde_json::Value::Null => Value::Symbol("null".into()),
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        serde_json::Value::String(s) => Value::Str(s),
        serde_json::Value::Array(arr) => {
            Value::List(arr.into_iter().map(serde_to_sabo).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: HashMap<Value, Value> = obj.into_iter()
                .map(|(k, v)| (Value::Str(k), serde_to_sabo(v)))
                .collect();
            Value::Map(map)
        }
    }
}

fn key_to_string(val: &Value) -> String {
    match val {
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Symbol(s) => s.clone(),
        other => format!("{}", other),
    }
}

// ============================================================
// JSON
// ============================================================

fn json_parse(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    match val {
        Value::Str(s) => {
            let parsed: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| format!("JSON parse error: {}", e))?;
            vm.push_val(serde_to_sabo(parsed));
            Ok(())
        }
        _ => Err(format!("'json_parse' expects string, got {}", val.type_name())),
    }
}

fn json_encode(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let json = sabo_to_serde(&val);
    let s = serde_json::to_string(&json)
        .map_err(|e| format!("JSON encode error: {}", e))?;
    vm.push_val(Value::Str(s));
    Ok(())
}

fn json_pretty(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let json = sabo_to_serde(&val);
    let s = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("JSON encode error: {}", e))?;
    vm.push_val(Value::Str(s));
    Ok(())
}

// ============================================================
// YAML
// ============================================================

fn yaml_value_to_sabo(val: serde_yaml::Value) -> Value {
    match val {
        serde_yaml::Value::Null => Value::Symbol("null".into()),
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        serde_yaml::Value::String(s) => Value::Str(s),
        serde_yaml::Value::Sequence(arr) => {
            Value::List(arr.into_iter().map(yaml_value_to_sabo).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let m: HashMap<Value, Value> = map.into_iter()
                .map(|(k, v)| (yaml_value_to_sabo(k), yaml_value_to_sabo(v)))
                .collect();
            Value::Map(m)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_value_to_sabo(tagged.value),
    }
}

fn sabo_to_yaml(val: &Value) -> serde_yaml::Value {
    match val {
        Value::Int(n) => serde_yaml::Value::Number(serde_yaml::Number::from(*n)),
        Value::Float(f) => serde_yaml::Value::Number(serde_yaml::Number::from(*f)),
        Value::Str(s) => serde_yaml::Value::String(s.clone()),
        Value::Symbol(s) if s == "null" => serde_yaml::Value::Null,
        Value::Symbol(s) => serde_yaml::Value::String(s.clone()),
        Value::Bool(b) => serde_yaml::Value::Bool(*b),
        Value::List(items) => {
            serde_yaml::Value::Sequence(items.iter().map(sabo_to_yaml).collect())
        }
        Value::Map(map) => {
            let m: serde_yaml::Mapping = map.iter()
                .map(|(k, v)| (sabo_to_yaml(k), sabo_to_yaml(v)))
                .collect();
            serde_yaml::Value::Mapping(m)
        }
        Value::Quotation(_) => serde_yaml::Value::Null,
    }
}

fn yaml_parse(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    match val {
        Value::Str(s) => {
            let parsed: serde_yaml::Value = serde_yaml::from_str(&s)
                .map_err(|e| format!("YAML parse error: {}", e))?;
            vm.push_val(yaml_value_to_sabo(parsed));
            Ok(())
        }
        _ => Err(format!("'yaml_parse' expects string, got {}", val.type_name())),
    }
}

fn yaml_encode(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let yaml = sabo_to_yaml(&val);
    let s = serde_yaml::to_string(&yaml)
        .map_err(|e| format!("YAML encode error: {}", e))?;
    vm.push_val(Value::Str(s));
    Ok(())
}

// ============================================================
// TOML
// ============================================================

fn toml_value_to_sabo(val: toml::Value) -> Value {
    match val {
        toml::Value::String(s) => Value::Str(s),
        toml::Value::Integer(n) => Value::Int(n),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(dt) => Value::Str(dt.to_string()),
        toml::Value::Array(arr) => {
            Value::List(arr.into_iter().map(toml_value_to_sabo).collect())
        }
        toml::Value::Table(table) => {
            let map: HashMap<Value, Value> = table.into_iter()
                .map(|(k, v)| (Value::Str(k), toml_value_to_sabo(v)))
                .collect();
            Value::Map(map)
        }
    }
}

fn sabo_to_toml(val: &Value) -> Result<toml::Value, String> {
    match val {
        Value::Int(n) => Ok(toml::Value::Integer(*n)),
        Value::Float(f) => Ok(toml::Value::Float(*f)),
        Value::Str(s) => Ok(toml::Value::String(s.clone())),
        Value::Symbol(s) => Ok(toml::Value::String(s.clone())),
        Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        Value::List(items) => {
            let arr: Result<Vec<toml::Value>, String> = items.iter().map(sabo_to_toml).collect();
            Ok(toml::Value::Array(arr?))
        }
        Value::Map(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                table.insert(key_to_string(k), sabo_to_toml(v)?);
            }
            Ok(toml::Value::Table(table))
        }
        Value::Quotation(_) => Err("Cannot encode quotation to TOML".into()),
    }
}

fn toml_parse(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    match val {
        Value::Str(s) => {
            let parsed: toml::Value = s.parse()
                .map_err(|e| format!("TOML parse error: {}", e))?;
            vm.push_val(toml_value_to_sabo(parsed));
            Ok(())
        }
        _ => Err(format!("'toml_parse' expects string, got {}", val.type_name())),
    }
}

fn toml_encode(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let toml_val = sabo_to_toml(&val)?;
    // toml::to_string requires a table at the top level
    let s = match &toml_val {
        toml::Value::Table(_) => toml::to_string(&toml_val)
            .map_err(|e| format!("TOML encode error: {}", e))?,
        _ => toml::to_string(&toml_val)
            .map_err(|e| format!("TOML encode error: {}", e))?,
    };
    vm.push_val(Value::Str(s));
    Ok(())
}

// ============================================================
// Binary encoding (protobuf-style, self-describing)
//
// Wire format (tag-length-value):
//   type_byte | field_data
//
// Type bytes:
//   0x00 = null
//   0x01 = bool (1 byte: 0/1)
//   0x02 = int (8 bytes, big-endian i64)
//   0x03 = float (8 bytes, big-endian f64)
//   0x04 = string (4-byte length + utf8 bytes)
//   0x05 = symbol (4-byte length + utf8 bytes)
//   0x06 = list (4-byte count + elements)
//   0x07 = map (4-byte count + key-value pairs)
// ============================================================

fn proto_encode_value(val: &Value, buf: &mut Vec<u8>) {
    match val {
        Value::Symbol(s) if s == "null" => buf.push(0x00),
        Value::Bool(b) => {
            buf.push(0x01);
            buf.push(if *b { 1 } else { 0 });
        }
        Value::Int(n) => {
            buf.push(0x02);
            buf.extend_from_slice(&n.to_be_bytes());
        }
        Value::Float(f) => {
            buf.push(0x03);
            buf.extend_from_slice(&f.to_be_bytes());
        }
        Value::Str(s) => {
            buf.push(0x04);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::Symbol(s) => {
            buf.push(0x05);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::List(items) => {
            buf.push(0x06);
            buf.extend_from_slice(&(items.len() as u32).to_be_bytes());
            for item in items {
                proto_encode_value(item, buf);
            }
        }
        Value::Map(map) => {
            buf.push(0x07);
            buf.extend_from_slice(&(map.len() as u32).to_be_bytes());
            for (k, v) in map {
                proto_encode_value(k, buf);
                proto_encode_value(v, buf);
            }
        }
        Value::Quotation(_) => buf.push(0x00), // encode as null
    }
}

fn proto_decode_value(bytes: &[u8], pos: usize) -> Result<(Value, usize), String> {
    if pos >= bytes.len() {
        return Err("Unexpected end of binary data".into());
    }
    let tag = bytes[pos];
    let mut i = pos + 1;

    match tag {
        0x00 => Ok((Value::Symbol("null".into()), i)),
        0x01 => {
            if i >= bytes.len() { return Err("Truncated bool".into()); }
            let val = bytes[i] != 0;
            Ok((Value::Bool(val), i + 1))
        }
        0x02 => {
            if i + 8 > bytes.len() { return Err("Truncated int".into()); }
            let n = i64::from_be_bytes(bytes[i..i+8].try_into().unwrap());
            Ok((Value::Int(n), i + 8))
        }
        0x03 => {
            if i + 8 > bytes.len() { return Err("Truncated float".into()); }
            let f = f64::from_be_bytes(bytes[i..i+8].try_into().unwrap());
            Ok((Value::Float(f), i + 8))
        }
        0x04 => {
            if i + 4 > bytes.len() { return Err("Truncated string length".into()); }
            let len = u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;
            i += 4;
            if i + len > bytes.len() { return Err("Truncated string data".into()); }
            let s = String::from_utf8(bytes[i..i+len].to_vec())
                .map_err(|e| format!("Invalid UTF-8 in string: {}", e))?;
            Ok((Value::Str(s), i + len))
        }
        0x05 => {
            if i + 4 > bytes.len() { return Err("Truncated symbol length".into()); }
            let len = u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;
            i += 4;
            if i + len > bytes.len() { return Err("Truncated symbol data".into()); }
            let s = String::from_utf8(bytes[i..i+len].to_vec())
                .map_err(|e| format!("Invalid UTF-8 in symbol: {}", e))?;
            Ok((Value::Symbol(s), i + len))
        }
        0x06 => {
            if i + 4 > bytes.len() { return Err("Truncated list length".into()); }
            let count = u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;
            i += 4;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let (val, next) = proto_decode_value(bytes, i)?;
                items.push(val);
                i = next;
            }
            Ok((Value::List(items), i))
        }
        0x07 => {
            if i + 4 > bytes.len() { return Err("Truncated map length".into()); }
            let count = u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;
            i += 4;
            let mut map = HashMap::with_capacity(count);
            for _ in 0..count {
                let (k, next) = proto_decode_value(bytes, i)?;
                i = next;
                let (v, next) = proto_decode_value(bytes, i)?;
                i = next;
                map.insert(k, v);
            }
            Ok((Value::Map(map), i))
        }
        _ => Err(format!("Unknown type tag: 0x{:02x}", tag)),
    }
}

fn proto_encode(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    let mut buf = Vec::new();
    proto_encode_value(&val, &mut buf);
    // Return as a list of ints (bytes)
    let byte_list: Vec<Value> = buf.iter().map(|&b| Value::Int(b as i64)).collect();
    vm.push_val(Value::List(byte_list));
    Ok(())
}

fn proto_decode(vm: &mut VM) -> Result<(), String> {
    let val = vm.pop_val()?;
    match val {
        Value::List(byte_list) => {
            let bytes: Result<Vec<u8>, String> = byte_list.iter().map(|v| {
                match v {
                    Value::Int(n) if *n >= 0 && *n <= 255 => Ok(*n as u8),
                    _ => Err("'proto_decode' expects list of byte values (0-255)".into()),
                }
            }).collect();
            let bytes = bytes?;
            let (val, _) = proto_decode_value(&bytes, 0)?;
            vm.push_val(val);
            Ok(())
        }
        Value::Str(s) => {
            // Also accept raw byte strings
            let (val, _) = proto_decode_value(s.as_bytes(), 0)?;
            vm.push_val(val);
            Ok(())
        }
        _ => Err(format!("'proto_decode' expects list of bytes, got {}", val.type_name())),
    }
}

// ============================================================
// Registration
// ============================================================

pub fn register(vm: &mut VM) {
    vm.register_builtin("json_parse", json_parse);
    vm.register_builtin("json_encode", json_encode);
    vm.register_builtin("json_pretty", json_pretty);
    vm.register_builtin("yaml_parse", yaml_parse);
    vm.register_builtin("yaml_encode", yaml_encode);
    vm.register_builtin("toml_parse", toml_parse);
    vm.register_builtin("toml_encode", toml_encode);
    vm.register_builtin("proto_encode", proto_encode);
    vm.register_builtin("proto_decode", proto_decode);
}
