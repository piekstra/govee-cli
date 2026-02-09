use clap::Subcommand;
use serde_json::json;

use crate::cli::output::{print_json, print_output};
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum SegmentCommand {
    /// Set per-segment colors (value as JSON matching Govee API format)
    Color {
        /// Device name or ID
        device: String,
        /// JSON value for segment colors (e.g., '{"segment":[[0,5,16711680]]}')
        value: String,
    },
    /// Set per-segment brightness (value as JSON matching Govee API format)
    Brightness {
        /// Device name or ID
        device: String,
        /// JSON value for segment brightness (e.g., '{"segment":[[0,5,80]]}')
        value: String,
    },
    /// Show segment capability info for a device
    Info {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(cmd: &SegmentCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        SegmentCommand::Color { device, value } => handle_color(device, value, config).await,
        SegmentCommand::Brightness { device, value } => {
            handle_brightness(device, value, config).await
        }
        SegmentCommand::Info { device } => handle_info(device, config).await,
    }
}

async fn handle_color(device: &str, value: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let parsed: serde_json::Value = serde_json::from_str(value).map_err(|e| {
        AppError::InvalidInput(format!(
            "Invalid JSON: {}. See 'govee segment info' for format",
            e
        ))
    })?;
    let dev = resolve::resolve_device(device, config.verbose).await?;
    dev.set_segment_color(parsed).await?;
    print_json(&json!({
        "device": dev.name(),
        "segment_color": "set",
    }));
    Ok(())
}

async fn handle_brightness(
    device: &str,
    value: &str,
    config: &RuntimeConfig,
) -> Result<(), AppError> {
    let parsed: serde_json::Value = serde_json::from_str(value).map_err(|e| {
        AppError::InvalidInput(format!(
            "Invalid JSON: {}. See 'govee segment info' for format",
            e
        ))
    })?;
    let dev = resolve::resolve_device(device, config.verbose).await?;
    dev.set_segment_brightness(parsed).await?;
    print_json(&json!({
        "device": dev.name(),
        "segment_brightness": "set",
    }));
    Ok(())
}

async fn handle_info(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let segment_caps: Vec<serde_json::Value> = dev
        .info
        .capabilities
        .iter()
        .filter(|c| c.capability_type == "devices.capabilities.segment_color_setting")
        .map(|c| {
            json!({
                "instance": c.instance,
                "parameters": c.parameters,
            })
        })
        .collect();

    if segment_caps.is_empty() {
        return Err(AppError::UnsupportedOperation(format!(
            "{} does not support segment control",
            dev.name()
        )));
    }

    print_output(
        &json!({
            "device": dev.name(),
            "segment_capabilities": segment_caps,
        }),
        config.output_mode,
    );
    Ok(())
}
