use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use pk_cli_core::output::emit_one;

use super::output::emit_list_with;
use super::scene::normalize_for_match;
use super::Ctx;
use crate::error::AppError;
use crate::models::device::validate_sensitivity;
use crate::resolve;

#[derive(Subcommand, Debug)]
pub enum MusicCommand {
    /// The music modes a device offers (music-mode-list/v1).
    #[command(visible_alias = "ls")]
    List {
        /// Device name or ID
        device: String,
    },
    /// Activate a music mode by name.
    Set {
        /// Device name or ID
        device: String,
        /// Music mode name (case-insensitive, partial match supported)
        mode: String,
        /// Sensitivity (0-100)
        #[arg(short, long, default_value = "50")]
        sensitivity: u8,
    },
}

/// Argument checks that need no credential (exit 2 before the keychain).
pub fn validate(cmd: &MusicCommand) -> Result<(), CliError> {
    if let MusicCommand::Set { sensitivity, .. } = cmd {
        validate_sensitivity(*sensitivity)?;
    }
    Ok(())
}

pub async fn handle(ctx: &Ctx, cmd: &MusicCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    match cmd {
        MusicCommand::List { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            if !dev.info.has_music_mode() {
                return Err(AppError::UnsupportedOperation(format!(
                    "{} does not support music mode",
                    dev.name()
                ))
                .into());
            }
            let items = extract_music_modes(&dev.info.capabilities);
            emit_list_with(
                ctx.json,
                "music-mode",
                &[("device", json!(dev.name()))],
                items,
                &["name"],
            );
        }
        MusicCommand::Set {
            device,
            mode,
            sensitivity,
        } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let modes = extract_music_modes(&dev.info.capabilities);
            let mode_normalized = normalize_for_match(mode);
            let name_of = |m: &Value| {
                m.get("name")
                    .and_then(|n| n.as_str())
                    .map(normalize_for_match)
            };
            let found = modes
                .iter()
                .find(|m| name_of(m).as_deref() == Some(mode_normalized.as_str()))
                .or_else(|| {
                    modes
                        .iter()
                        .find(|m| name_of(m).is_some_and(|n| n.contains(&mode_normalized)))
                })
                .ok_or_else(|| {
                    AppError::DeviceNotFound(format!(
                        "music mode `{mode}` not found for device `{}`",
                        dev.name()
                    ))
                })?;
            let mode_name = found.get("name").and_then(|n| n.as_str()).unwrap_or(mode);
            let mode_value = found.get("value").cloned().unwrap_or(Value::Null);
            // The API expects a struct: {musicMode: <id>, sensitivity: <0-100>, autoColor: 1}
            let value = json!({
                "musicMode": mode_value,
                "sensitivity": sensitivity,
                "autoColor": 1,
            });
            dev.set_music_mode(value).await?;
            emit_one(
                ctx.json,
                "music-mode",
                json!({ "device": dev.name(), "music_mode": mode_name, "sensitivity": sensitivity, "activated": true }),
            );
        }
    }
    Ok(())
}

/// Music mode uses STRUCT parameters with fields, not top-level options;
/// the mode enum is in the field named `musicMode`.
pub fn extract_music_modes(capabilities: &[crate::models::capability::Capability]) -> Vec<Value> {
    let mut modes = Vec::new();
    for cap in capabilities {
        if cap.capability_type != "devices.capabilities.music_setting"
            || cap.instance != "musicMode"
        {
            continue;
        }
        let Some(fields) = cap.parameters.get("fields").and_then(|v| v.as_array()) else {
            continue;
        };
        for field in fields {
            if field.get("fieldName").and_then(|v| v.as_str()) != Some("musicMode") {
                continue;
            }
            let Some(options) = field.get("options").and_then(|v| v.as_array()) else {
                continue;
            };
            for option in options {
                if let Some(name) = option.get("name").and_then(|n| n.as_str()) {
                    modes.push(json!({ "name": name, "value": option.get("value") }));
                }
            }
        }
    }
    modes
}
