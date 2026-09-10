use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::json;

use super::Ctx;
use crate::error::AppError;
use crate::models::device::{validate_brightness, validate_color_temp};
use crate::resolve;
use pk_cli_core::output::emit_one;

#[derive(Subcommand, Debug)]
pub enum LightCommand {
    /// Set brightness (1-100).
    Brightness {
        /// Device name or ID
        device: String,
        /// Brightness level (1-100)
        level: u8,
    },
    /// Set an RGB color, as --red/--green/--blue or --hex.
    Color {
        /// Device name or ID
        device: String,
        /// Red (0-255)
        #[arg(long)]
        red: Option<u8>,
        /// Green (0-255)
        #[arg(long)]
        green: Option<u8>,
        /// Blue (0-255)
        #[arg(long)]
        blue: Option<u8>,
        /// Hex color code (e.g. "#FF0000" or "FF0000")
        #[arg(long, conflicts_with_all = ["red", "green", "blue"])]
        hex: Option<String>,
    },
    /// Set color temperature (2000-9000 Kelvin).
    #[command(visible_alias = "color-temp")]
    Temp {
        /// Device name or ID
        device: String,
        /// Color temperature in Kelvin (2000-9000)
        kelvin: u16,
    },
    /// The device's current state, raw from the Platform API (light-state/v1).
    State {
        /// Device name or ID
        device: String,
    },
}

/// Argument checks that need no credential (exit 2 before the keychain).
pub fn validate(cmd: &LightCommand) -> Result<(), CliError> {
    match cmd {
        LightCommand::Brightness { level, .. } => validate_brightness(*level)?,
        LightCommand::Temp { kelvin, .. } => validate_color_temp(*kelvin)?,
        LightCommand::Color {
            red,
            green,
            blue,
            hex,
            ..
        } => {
            rgb_of(*red, *green, *blue, hex.as_deref())?;
        }
        LightCommand::State { .. } => {}
    }
    Ok(())
}

fn rgb_of(
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    hex: Option<&str>,
) -> Result<(u8, u8, u8), AppError> {
    if let Some(h) = hex {
        return parse_hex_color(h);
    }
    match (red, green, blue) {
        (Some(r), Some(g), Some(b)) => Ok((r, g, b)),
        _ => Err(AppError::InvalidInput(
            "provide either --hex or all of --red --green --blue".to_string(),
        )),
    }
}

pub async fn handle(ctx: &Ctx, cmd: &LightCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    match cmd {
        LightCommand::Brightness { device, level } => {
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_brightness(*level).await?;
            emit_one(
                ctx.json,
                "light-brightness",
                json!({ "device": dev.name(), "brightness": level }),
            );
        }
        LightCommand::Color {
            device,
            red,
            green,
            blue,
            hex,
        } => {
            let (r, g, b) = rgb_of(*red, *green, *blue, hex.as_deref())?;
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_color_rgb(r, g, b).await?;
            emit_one(
                ctx.json,
                "light-color",
                json!({
                    "device": dev.name(),
                    "color": { "r": r, "g": g, "b": b },
                    "hex": format!("#{r:02X}{g:02X}{b:02X}"),
                }),
            );
        }
        LightCommand::Temp { device, kelvin } => {
            let dev = resolve::resolve_device(&api, device).await?;
            dev.set_color_temp(*kelvin).await?;
            emit_one(
                ctx.json,
                "light-temperature",
                json!({ "device": dev.name(), "color_temp_k": kelvin }),
            );
        }
        LightCommand::State { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let state = dev.get_state().await?;
            emit_one(
                ctx.json,
                "light-state",
                json!({ "device": dev.name(), "state": state }),
            );
        }
    }
    Ok(())
}

pub fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8), AppError> {
    let hex = hex.trim_start_matches('#');
    // Six ASCII hex digits exactly: checking the characters (not the byte
    // length) keeps the fixed-offset slices below on char boundaries, so a
    // multi-byte character is a usage error rather than a panic.
    if hex.chars().count() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::InvalidInput(format!(
            "invalid hex color `{hex}`: expected 6 hex digits (e.g. FF0000)"
        )));
    }
    let channel = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| AppError::InvalidInput(format!("invalid hex color `{hex}`")))
    };
    Ok((channel(0)?, channel(2)?, channel(4)?))
}
