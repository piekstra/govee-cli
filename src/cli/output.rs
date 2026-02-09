use tabled::{Table, Tabled};

use crate::config::OutputMode;

pub fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    );
}

pub fn print_error(error: &crate::error::AppError) {
    eprintln!(
        "{}",
        serde_json::to_string_pretty(&error.to_json()).unwrap_or_else(|_| error.to_string())
    );
}

pub fn print_output(value: &serde_json::Value, mode: OutputMode) {
    match mode {
        OutputMode::Json => print_json(value),
        OutputMode::Table => print_as_table(value),
    }
}

/// Print a table from a JSON value using tabled crate.
/// Handles arrays of objects (row-per-item) and single objects (key-value pairs).
fn print_as_table(value: &serde_json::Value) {
    // If the value is an array, render each element as a table row
    if let Some(arr) = value.as_array() {
        print_array_table(arr);
        return;
    }

    // If the value is an object, check for a list-like field to render as table
    if let Some(obj) = value.as_object() {
        // Look for the first array-typed field (e.g., "scenes", "devices", "toggles")
        for (key, val) in obj {
            if let Some(arr) = val.as_array() {
                if !arr.is_empty() {
                    // Print header context (e.g., device name)
                    for (k, v) in obj {
                        if k != key {
                            if let Some(s) = v.as_str() {
                                eprintln!("{}: {}", k, s);
                            }
                        }
                    }
                    print_array_table(arr);
                    return;
                }
            }
        }

        // No array field found — render as key-value pairs
        let rows: Vec<KeyValue> = obj
            .iter()
            .map(|(k, v)| KeyValue {
                key: k.clone(),
                value: format_value(v),
            })
            .collect();
        if !rows.is_empty() {
            println!("{}", Table::new(rows));
        }
        return;
    }

    // Fallback to JSON for other types
    print_json(value);
}

fn print_array_table(arr: &[serde_json::Value]) {
    if arr.is_empty() {
        println!("(no results)");
        return;
    }

    // Collect all unique keys from all objects to form column headers
    let mut columns: Vec<String> = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
    }

    if columns.is_empty() {
        // Array of primitives
        for item in arr {
            println!("{}", format_value(item));
        }
        return;
    }

    // Build rows
    let rows: Vec<Vec<String>> = arr
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|col| item.get(col).map(format_value).unwrap_or_default())
                .collect()
        })
        .collect();

    // Use tabled with dynamic columns
    let mut builder = tabled::builder::Builder::new();
    builder.push_record(&columns);
    for row in &rows {
        builder.push_record(row);
    }
    println!("{}", builder.build());
}

fn format_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "".to_string(),
        // For nested objects/arrays, use compact JSON
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[derive(Tabled)]
struct KeyValue {
    key: String,
    value: String,
}
