use clap::Subcommand;
use serde_json::json;

use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum LightCommand {
    /// Set brightness (1-100)
    Brightness {
        /// Device name or ID
        device: String,
        /// Brightness level (1-100)
        level: u8,
    },
    /// Set RGB color
    Color {
        /// Device name or ID
        device: String,
        /// Red (0-255), or hex color if --hex is used
        #[arg(long)]
        red: Option<u8>,
        /// Green (0-255)
        #[arg(long)]
        green: Option<u8>,
        /// Blue (0-255)
        #[arg(long)]
        blue: Option<u8>,
        /// Hex color code (e.g., "#FF0000" or "FF0000")
        #[arg(long)]
        hex: Option<String>,
    },
    /// Set color temperature (2000-9000 Kelvin)
    #[command(visible_alias = "color-temp")]
    Temp {
        /// Device name or ID
        device: String,
        /// Color temperature in Kelvin (2000-9000)
        kelvin: u16,
    },
    /// Get current light state
    State {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(cmd: &LightCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        LightCommand::Brightness { device, level } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            dev.set_brightness(*level).await?;
            print_json(&json!({
                "device": dev.name(),
                "brightness": level,
            }));
        }
        LightCommand::Color {
            device,
            red,
            green,
            blue,
            hex,
        } => {
            let (r, g, b) = if let Some(hex_str) = hex {
                parse_hex_color(hex_str)?
            } else {
                match (red, green, blue) {
                    (Some(r), Some(g), Some(b)) => (*r, *g, *b),
                    _ => {
                        return Err(AppError::InvalidInput(
                            "Provide either --hex or all of --red --green --blue".to_string(),
                        ))
                    }
                }
            };
            let dev = resolve::resolve_device(device, config.verbose).await?;
            dev.set_color_rgb(r, g, b).await?;
            print_json(&json!({
                "device": dev.name(),
                "color": { "r": r, "g": g, "b": b },
            }));
        }
        LightCommand::Temp { device, kelvin } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            dev.set_color_temp(*kelvin).await?;
            print_json(&json!({
                "device": dev.name(),
                "color_temp_k": kelvin,
            }));
        }
        LightCommand::State { device } => {
            let dev = resolve::resolve_device(device, config.verbose).await?;
            let state = dev.get_state().await?;
            print_json(&json!({
                "device": dev.name(),
                "state": state,
            }));
        }
    }
    Ok(())
}

pub fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8), AppError> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(AppError::InvalidInput(format!(
            "Invalid hex color '{}'. Expected 6 hex digits (e.g., FF0000)",
            hex
        )));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| AppError::InvalidInput(format!("Invalid hex color '{}'", hex)))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| AppError::InvalidInput(format!("Invalid hex color '{}'", hex)))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| AppError::InvalidInput(format!("Invalid hex color '{}'", hex)))?;
    Ok((r, g, b))
}
