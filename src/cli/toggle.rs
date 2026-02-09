use clap::Subcommand;
use serde_json::json;

use crate::cli::output::{print_json, print_output};
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum ToggleCommand {
    /// Toggle gradient mode on or off
    Gradient {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// Toggle DreamView mode on or off
    Dreamview {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// List available toggles for a device
    List {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(cmd: &ToggleCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        ToggleCommand::Gradient { device, state } => handle_gradient(device, state, config).await,
        ToggleCommand::Dreamview { device, state } => handle_dreamview(device, state, config).await,
        ToggleCommand::List { device } => handle_list(device, config).await,
    }
}

fn parse_on_off(state: &str) -> Result<bool, AppError> {
    match state.to_lowercase().as_str() {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(AppError::InvalidInput(format!(
            "Invalid state '{}'. Use 'on' or 'off'",
            state
        ))),
    }
}

async fn handle_gradient(
    device: &str,
    state: &str,
    config: &RuntimeConfig,
) -> Result<(), AppError> {
    let on = parse_on_off(state)?;
    let dev = resolve::resolve_device(device, config.verbose).await?;
    dev.set_gradient(on).await?;
    print_json(&json!({
        "device": dev.name(),
        "gradient": if on { "on" } else { "off" },
    }));
    Ok(())
}

async fn handle_dreamview(
    device: &str,
    state: &str,
    config: &RuntimeConfig,
) -> Result<(), AppError> {
    let on = parse_on_off(state)?;
    let dev = resolve::resolve_device(device, config.verbose).await?;
    dev.set_dreamview(on).await?;
    print_json(&json!({
        "device": dev.name(),
        "dreamview": if on { "on" } else { "off" },
    }));
    Ok(())
}

async fn handle_list(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let toggles: Vec<serde_json::Value> = dev
        .info
        .capabilities
        .iter()
        .filter(|c| c.capability_type == "devices.capabilities.toggle")
        .map(|c| {
            json!({
                "toggle": c.instance,
            })
        })
        .collect();

    print_output(
        &json!({
            "device": dev.name(),
            "toggles": toggles,
        }),
        config.output_mode,
    );
    Ok(())
}
