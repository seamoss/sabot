use crate::value::Value;
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use std::collections::HashMap;

// Convert a Sabot Value to a rusqlite parameter
fn sabot_to_sql(val: &Value) -> SqlValue {
    match val {
        Value::Int(n) => SqlValue::Integer(*n),
        Value::Float(f) => SqlValue::Real(*f),
        Value::Str(s) => SqlValue::Text(s.clone()),
        Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        Value::Symbol(s) if s == "null" => SqlValue::Null,
        _ => SqlValue::Text(format!("{}", val)),
    }
}

// Convert a rusqlite value to a Sabot Value
fn sql_to_sabot(val: &rusqlite::types::ValueRef) -> Value {
    match val {
        rusqlite::types::ValueRef::Null => Value::Symbol("null".to_string()),
        rusqlite::types::ValueRef::Integer(n) => Value::Int(*n),
        rusqlite::types::ValueRef::Real(f) => Value::Float(*f),
        rusqlite::types::ValueRef::Text(s) => Value::Str(String::from_utf8_lossy(s).to_string()),
        rusqlite::types::ValueRef::Blob(b) => Value::Str(format!("<blob:{} bytes>", b.len())),
    }
}

/// Execute a query and return results as a list of maps
pub fn query(conn: &Connection, sql: &str, params: &[Value]) -> Result<Value, String> {
    let sql_params: Vec<SqlValue> = params.iter().map(sabot_to_sql).collect();
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("SQL prepare error: {}", e))?;

    let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

    let rows = stmt
        .query_map(params_from_iter(sql_params), |row| {
            let mut map = HashMap::new();
            for (i, name) in column_names.iter().enumerate() {
                let val = row.get_ref(i).unwrap();
                map.insert(Value::Str(name.clone()), sql_to_sabot(&val));
            }
            Ok(Value::Map(map))
        })
        .map_err(|e| format!("SQL query error: {}", e))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| format!("SQL row error: {}", e))?);
    }
    Ok(Value::List(results))
}

/// Execute a statement (INSERT, UPDATE, DELETE, CREATE, etc.)
pub fn exec(conn: &Connection, sql: &str, params: &[Value]) -> Result<usize, String> {
    let sql_params: Vec<SqlValue> = params.iter().map(sabot_to_sql).collect();
    conn.execute(sql, params_from_iter(sql_params))
        .map_err(|e| format!("SQL exec error: {}", e))
}
