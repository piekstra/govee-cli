//! Thin wrappers over `pk_cli_core::output` for the two shapes every
//! command emits: one resource (a key/value block) or a list (`items` under
//! a pipe table, any scalar context fields printed above it).

use pk_cli_core::output;
use serde_json::Value;

/// One resource: the DTO in JSON mode, a `key: value` block otherwise.
pub fn emit_one(json: bool, schema: &str, value: Value) {
    output::emit(json, schema, value, |v| output::kv(v, 0));
}

/// A list: `{"items": [...], ...}` in JSON mode; in text mode the non-list
/// fields as `key: value` lines, then a table of `columns` (every column
/// when empty).
pub fn emit_list(json: bool, schema: &str, payload: Value, columns: &[&str]) {
    output::emit(json, schema, payload, |v| {
        if let Some(obj) = v.as_object() {
            for (k, val) in obj {
                if k != "items" && !val.is_array() && !val.is_object() {
                    println!("{k}: {}", output::scalar(val));
                }
            }
        }
        let rows = output::rows_of(v, "items");
        if rows.is_empty() {
            println!("(none)");
        } else if columns.is_empty() {
            output::table(&rows);
        } else {
            output::table(&output::table_view(&rows, columns));
        }
    });
}
