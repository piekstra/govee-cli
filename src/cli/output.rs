//! One composition over `pk_cli_core::output` for the lists that carry
//! scalar context alongside their rows (the device a scene list belongs to,
//! the count of room-less devices). Plain lists and single resources use
//! `output::emit_list` / `output::emit_one` directly.

use pk_cli_core::output;
use serde_json::{Map, Value};

/// `{"schema": "<record>-list/v1", <context…>, "items": [...]}` in JSON
/// mode; the context as `key: value` lines and then a `columns` table of
/// the items in text mode.
pub fn emit_list_with(
    json: bool,
    record: &str,
    context: &[(&str, Value)],
    items: Vec<Value>,
    columns: &[&str],
) {
    let mut payload = Map::new();
    for (k, v) in context {
        payload.insert((*k).to_string(), v.clone());
    }
    payload.insert("items".into(), Value::Array(items));
    output::emit(
        json,
        &format!("{record}-list"),
        Value::Object(payload),
        |v| {
            for (k, _) in context {
                if let Some(val) = v.get(*k) {
                    println!("{k}: {}", output::scalar(val));
                }
            }
            output::table(&output::table_view(&output::rows_of(v, "items"), columns));
        },
    );
}
