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
        OutputMode::Table => {
            // Table output will be implemented in v0.2.0
            // For now, fall back to JSON
            print_json(value);
        }
    }
}
