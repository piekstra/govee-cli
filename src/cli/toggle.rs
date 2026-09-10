use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::json;

use pk_cli_core::output::emit_one;

use super::output::emit_list_with;
use super::Ctx;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand, Debug)]
pub enum ToggleCommand {
    /// Gradient mode on or off.
    Gradient {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// DreamView mode on or off.
    Dreamview {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// The toggles a device offers (toggle-list/v1).
    #[command(visible_alias = "ls")]
    List {
        /// Device name or ID
        device: String,
    },
}

pub fn parse_on_off(state: &str) -> Result<bool, AppError> {
    match state.to_lowercase().as_str() {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(AppError::InvalidInput(format!(
            "invalid state `{state}`: use `on` or `off`"
        ))),
    }
}

/// Argument checks that need no credential (exit 2 before the keychain).
pub fn validate(cmd: &ToggleCommand) -> Result<(), CliError> {
    match cmd {
        ToggleCommand::Gradient { state, .. } | ToggleCommand::Dreamview { state, .. } => {
            parse_on_off(state)?;
        }
        ToggleCommand::List { .. } => {}
    }
    Ok(())
}

pub async fn handle(ctx: &Ctx, cmd: &ToggleCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    match cmd {
        ToggleCommand::Gradient { device, state } => {
            let on = parse_on_off(state)?;
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_gradient(on).await?;
            emit_one(
                ctx.json,
                "toggle",
                json!({ "device": dev.name(), "gradient": on_off(on) }),
            );
        }
        ToggleCommand::Dreamview { device, state } => {
            let on = parse_on_off(state)?;
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_dreamview(on).await?;
            emit_one(
                ctx.json,
                "toggle",
                json!({ "device": dev.name(), "dreamview": on_off(on) }),
            );
        }
        ToggleCommand::List { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let items: Vec<serde_json::Value> = dev
                .info
                .capabilities
                .iter()
                .filter(|c| c.capability_type == "devices.capabilities.toggle")
                .map(|c| json!({ "toggle": c.instance }))
                .collect();
            emit_list_with(
                ctx.json,
                "toggle",
                &[("device", json!(dev.name()))],
                items,
                &["toggle"],
            );
        }
    }
    Ok(())
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}
