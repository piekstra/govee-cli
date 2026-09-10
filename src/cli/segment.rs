use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use super::Ctx;
use crate::error::AppError;
use crate::resolve;
use pk_cli_core::output::emit_one;

#[derive(Subcommand, Debug)]
pub enum SegmentCommand {
    /// Set per-segment colors (value as JSON in the Platform API's format).
    Color {
        /// Device name or ID
        device: String,
        /// JSON value for segment colors (e.g. '{"segment":[0,1,2],"rgb":16711680}')
        value: String,
    },
    /// Set per-segment brightness (value as JSON in the Platform API's format).
    Brightness {
        /// Device name or ID
        device: String,
        /// JSON value for segment brightness (e.g. '{"segment":[0,1,2],"brightness":80}')
        value: String,
    },
    /// The device's segment capabilities (segment-info/v1).
    Info {
        /// Device name or ID
        device: String,
    },
}

fn parse_value(value: &str) -> Result<Value, AppError> {
    serde_json::from_str(value).map_err(|e| {
        AppError::InvalidInput(format!(
            "invalid JSON: {e}; see `govee segment info` for the format"
        ))
    })
}

/// Argument checks that need no credential (exit 2 before the keychain).
pub fn validate(cmd: &SegmentCommand) -> Result<(), CliError> {
    match cmd {
        SegmentCommand::Color { value, .. } | SegmentCommand::Brightness { value, .. } => {
            parse_value(value)?;
        }
        SegmentCommand::Info { .. } => {}
    }
    Ok(())
}

pub async fn handle(ctx: &Ctx, cmd: &SegmentCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    match cmd {
        SegmentCommand::Color { device, value } => {
            let parsed = parse_value(value)?;
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_segment_color(parsed).await?;
            emit_one(
                ctx.json,
                "segment-control",
                json!({ "device": dev.name(), "segment_color": "set" }),
            );
        }
        SegmentCommand::Brightness { device, value } => {
            let parsed = parse_value(value)?;
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_segment_brightness(parsed).await?;
            emit_one(
                ctx.json,
                "segment-control",
                json!({ "device": dev.name(), "segment_brightness": "set" }),
            );
        }
        SegmentCommand::Info { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let segment_caps: Vec<Value> = dev
                .info
                .capabilities
                .iter()
                .filter(|c| c.capability_type == "devices.capabilities.segment_color_setting")
                .map(|c| json!({ "instance": c.instance, "parameters": c.parameters }))
                .collect();
            if segment_caps.is_empty() {
                return Err(AppError::UnsupportedOperation(format!(
                    "{} does not support segment control",
                    dev.name()
                ))
                .into());
            }
            emit_one(
                ctx.json,
                "segment-info",
                json!({ "device": dev.name(), "segment_capabilities": segment_caps }),
            );
        }
    }
    Ok(())
}
