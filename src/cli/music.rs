use clap::Subcommand;
use serde_json::json;

use crate::cli::output::{print_json, print_output};
use crate::cli::scene::normalize_for_match;
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum MusicCommand {
    /// List available music modes for a device
    List {
        /// Device name or ID
        device: String,
    },
    /// Activate a music mode by name
    Set {
        /// Device name or ID
        device: String,
        /// Music mode name (case-insensitive, partial match supported)
        mode: String,
        /// Sensitivity (0-100, default: 50)
        #[arg(short, long, default_value = "50")]
        sensitivity: u8,
    },
}

pub async fn handle(cmd: &MusicCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        MusicCommand::List { device } => handle_list(device, config).await,
        MusicCommand::Set {
            device,
            mode,
            sensitivity,
        } => handle_set(device, mode, *sensitivity, config).await,
    }
}

fn extract_music_modes(
    capabilities: &[crate::models::capability::Capability],
) -> Vec<serde_json::Value> {
    let mut modes = Vec::new();
    for cap in capabilities {
        if cap.capability_type == "devices.capabilities.music_setting"
            && cap.instance == "musicMode"
        {
            // Music mode uses STRUCT parameters with fields, not top-level options.
            // The mode enum is in the field named "musicMode".
            if let Some(fields) = cap.parameters.get("fields").and_then(|v| v.as_array()) {
                for field in fields {
                    let field_name = field.get("fieldName").and_then(|v| v.as_str());
                    if field_name == Some("musicMode") {
                        if let Some(options) = field.get("options").and_then(|v| v.as_array()) {
                            for option in options {
                                if let Some(name) = option.get("name").and_then(|n| n.as_str()) {
                                    modes.push(json!({
                                        "name": name,
                                        "value": option.get("value"),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    modes
}

async fn handle_list(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    if !dev.info.has_music_mode() {
        return Err(AppError::UnsupportedOperation(format!(
            "{} does not support music mode",
            dev.name()
        )));
    }

    let modes = extract_music_modes(&dev.info.capabilities);
    print_output(
        &json!({
            "device": dev.name(),
            "music_modes": modes,
        }),
        config.output_mode,
    );
    Ok(())
}

async fn handle_set(
    device: &str,
    mode: &str,
    sensitivity: u8,
    config: &RuntimeConfig,
) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let modes = extract_music_modes(&dev.info.capabilities);
    let mode_normalized = normalize_for_match(mode);

    // Exact match (normalized), then partial
    let found = modes
        .iter()
        .find(|m| {
            m.get("name")
                .and_then(|n| n.as_str())
                .map(|n| normalize_for_match(n) == mode_normalized)
                .unwrap_or(false)
        })
        .or_else(|| {
            modes.iter().find(|m| {
                m.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| normalize_for_match(n).contains(&mode_normalized))
                    .unwrap_or(false)
            })
        });

    if let Some(music_mode) = found {
        let mode_name = music_mode
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(mode);
        if let Some(mode_value) = music_mode.get("value") {
            // The API expects a struct: {musicMode: <id>, sensitivity: <0-100>, autoColor: 1}
            let value = json!({
                "musicMode": mode_value,
                "sensitivity": sensitivity,
                "autoColor": 1,
            });
            dev.set_music_mode(value).await?;
            print_json(&json!({
                "device": dev.name(),
                "music_mode": mode_name,
                "activated": true,
            }));
            return Ok(());
        }
    }

    Err(AppError::InvalidInput(format!(
        "Music mode '{}' not found for device '{}'",
        mode,
        dev.name()
    )))
}
