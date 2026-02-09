use clap::Subcommand;
use serde_json::json;

use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum PowerCommand {
    /// Turn device on
    On {
        /// Device name or ID
        device: String,
    },
    /// Turn device off
    Off {
        /// Device name or ID
        device: String,
    },
    /// Toggle device power
    Toggle {
        /// Device name or ID
        device: String,
    },
    /// Get device power status
    Status {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(cmd: &PowerCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        PowerCommand::On { device } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            dev.power_on().await?;
            print_json(&json!({
                "device": dev.name(),
                "power": "on",
            }));
        }
        PowerCommand::Off { device } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            dev.power_off().await?;
            print_json(&json!({
                "device": dev.name(),
                "power": "off",
            }));
        }
        PowerCommand::Toggle { device } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            // Query current state, then toggle
            let state = dev.get_state().await?;
            let is_on = find_power_state(&state);
            if is_on {
                dev.power_off().await?;
                print_json(&json!({
                    "device": dev.name(),
                    "power": "off",
                    "toggled_from": "on",
                }));
            } else {
                dev.power_on().await?;
                print_json(&json!({
                    "device": dev.name(),
                    "power": "on",
                    "toggled_from": "off",
                }));
            }
        }
        PowerCommand::Status { device } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            let state = dev.get_state().await?;
            let is_on = find_power_state(&state);
            print_json(&json!({
                "device": dev.name(),
                "power": if is_on { "on" } else { "off" },
            }));
        }
    }
    Ok(())
}

fn find_power_state(state: &serde_json::Value) -> bool {
    // State payload has "capabilities" array with current values
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
