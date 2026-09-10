use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::json;

use super::Ctx;
use crate::resolve;
use pk_cli_core::output::emit_one;

#[derive(Subcommand, Debug)]
pub enum PowerCommand {
    /// Turn a device on.
    On {
        /// Device name or ID
        device: String,
    },
    /// Turn a device off.
    Off {
        /// Device name or ID
        device: String,
    },
    /// Flip a device's power.
    Toggle {
        /// Device name or ID
        device: String,
    },
    /// Report whether a device is on.
    Status {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(ctx: &Ctx, cmd: &PowerCommand) -> Result<(), CliError> {
    let api = ctx.api()?;
    let payload = match cmd {
        PowerCommand::On { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            dev.power_on().await?;
            json!({ "device": dev.name(), "power": "on" })
        }
        PowerCommand::Off { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            dev.power_off().await?;
            json!({ "device": dev.name(), "power": "off" })
        }
        PowerCommand::Toggle { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let state = dev.get_state().await?;
            let is_on = find_power_state(&state);
            if is_on {
                dev.power_off().await?;
            } else {
                dev.power_on().await?;
            }
            json!({
                "device": dev.name(),
                "power": if is_on { "off" } else { "on" },
                "toggled_from": if is_on { "on" } else { "off" },
            })
        }
        PowerCommand::Status { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let state = dev.get_state().await?;
            let is_on = find_power_state(&state);
            json!({ "device": dev.name(), "power": if is_on { "on" } else { "off" } })
        }
    };
    emit_one(ctx.json, "power", payload);
    Ok(())
}

/// The `powerSwitch` value out of a `/device/state` payload; absent counts
/// as off.
pub fn find_power_state(state: &serde_json::Value) -> bool {
    if let Some(capabilities) = state.get("capabilities").and_then(|v| v.as_array()) {
        for cap in capabilities {
            let cap_type = cap.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let instance = cap.get("instance").and_then(|v| v.as_str()).unwrap_or("");
            if cap_type == "devices.capabilities.on_off" && instance == "powerSwitch" {
                return cap
                    .get("state")
                    .and_then(|s| s.get("value"))
                    .and_then(|v| v.as_i64())
                    .map(|v| v == 1)
                    .unwrap_or(false);
            }
        }
    }
    false
}
