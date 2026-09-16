//! Per-segment colour and brightness (`devices.capabilities.segment_color_setting`).
//! Segments are given as a list (`0,1,2`), ranges (`0-3,7`) or `all`; the
//! device's capability says how many there are and how many one call may
//! address, and longer lists are sent in as many calls as needed.

use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use super::light::parse_hex_color;
use super::Ctx;
use crate::error::AppError;
use crate::models::device::Device;
use crate::resolve;
use pk_cli_core::output::emit_one;

const SEGMENTS: &str = "devices.capabilities.segment_color_setting";

#[derive(Subcommand, Debug)]
pub enum SegmentCommand {
    /// Set the colour of some segments (segment-control/v1).
    Color {
        /// Device name or ID
        device: String,
        /// Segments: a list and/or ranges (`0,1,2`, `0-3,7`) or `all`
        #[arg(long, value_name = "SEGMENTS", required_unless_present = "value")]
        segments: Option<String>,
        /// Hex colour (`#FF0080` or `FF0080`)
        #[arg(long, conflicts_with_all = ["red", "green", "blue"])]
        hex: Option<String>,
        /// Red (0-255)
        #[arg(long)]
        red: Option<u8>,
        /// Green (0-255)
        #[arg(long)]
        green: Option<u8>,
        /// Blue (0-255)
        #[arg(long)]
        blue: Option<u8>,
        /// The Platform API's raw value (`{"segment":[0,1],"rgb":16711680}`);
        /// the 0.2 positional form, kept one major version.
        #[arg(long, hide = true, conflicts_with_all = ["segments", "hex", "red", "green", "blue"])]
        value: Option<String>,
    },
    /// Set the brightness of some segments (segment-control/v1).
    Brightness {
        /// Device name or ID
        device: String,
        /// Segments: a list and/or ranges (`0,1,2`, `0-3,7`) or `all`
        #[arg(long, value_name = "SEGMENTS", required_unless_present = "value")]
        segments: Option<String>,
        /// Brightness 0-100
        #[arg(long, value_name = "PCT", required_unless_present = "value")]
        brightness: Option<u8>,
        /// The Platform API's raw value (`{"segment":[0,1],"brightness":80}`);
        /// the 0.2 positional form, kept one major version.
        #[arg(long, hide = true, conflicts_with_all = ["segments", "brightness"])]
        value: Option<String>,
    },
    /// The device's segment capabilities (segment-info/v1).
    Info {
        /// Device name or ID
        device: String,
    },
}

/// Which segments a spec names: an explicit list, or every one the device has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segments {
    All,
    List(Vec<u8>),
}

/// `0,1,2`, `0-3,7`, `all` (case-insensitive; spaces allowed). Duplicates
/// collapse, order is ascending.
pub fn parse_segments(spec: &str) -> Result<Segments, AppError> {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("all") {
        return Ok(Segments::All);
    }
    let bad = |what: &str| {
        AppError::InvalidInput(format!(
            "invalid segments `{spec}` ({what}); use a list or ranges like `0,1,2` or `0-3,7`, or `all`"
        ))
    };
    let mut out: Vec<u8> = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(bad("empty entry"));
        }
        if let Some((a, b)) = part.split_once('-') {
            let a: u8 = a.trim().parse().map_err(|_| bad(part))?;
            let b: u8 = b.trim().parse().map_err(|_| bad(part))?;
            if a > b {
                return Err(bad("range runs backwards"));
            }
            out.extend(a..=b);
        } else {
            out.push(part.parse().map_err(|_| bad(part))?);
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(bad("no segments"));
    }
    Ok(Segments::List(out))
}

/// The segment limits a device declares for an instance: the highest
/// segment index and how many one call may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_index: u8,
    pub per_call: usize,
}

/// From the capability's `parameters.fields[name=segment]`.
pub fn limits(parameters: &Value) -> Option<Limits> {
    let field = parameters
        .get("fields")?
        .as_array()?
        .iter()
        .find(|f| f.get("fieldName").and_then(Value::as_str) == Some("segment"))?;
    let max_index = field.pointer("/elementRange/max")?.as_u64()? as u8;
    let per_call = field
        .pointer("/size/max")
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(max_index) + 1) as usize;
    Some(Limits {
        max_index,
        per_call: per_call.max(1),
    })
}

/// The concrete segment list for a spec, checked against the device's
/// limits, split into call-sized batches.
pub fn batches(spec: &Segments, limits: Limits) -> Result<Vec<Vec<u8>>, AppError> {
    let list: Vec<u8> = match spec {
        Segments::All => (0..=limits.max_index).collect(),
        Segments::List(l) => l.clone(),
    };
    if let Some(too_big) = list.iter().find(|s| **s > limits.max_index) {
        return Err(AppError::InvalidInput(format!(
            "segment {too_big} is out of range; this device has segments 0-{}",
            limits.max_index
        )));
    }
    Ok(list.chunks(limits.per_call).map(<[u8]>::to_vec).collect())
}

/// `0xRRGGBB` from `--hex` or the three channels.
pub fn rgb_from_args(
    hex: Option<&str>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
) -> Result<u32, AppError> {
    let (r, g, b) = match (hex, red, green, blue) {
        (Some(h), None, None, None) => parse_hex_color(h)?,
        (None, Some(r), Some(g), Some(b)) => (r, g, b),
        (None, None, None, None) => {
            return Err(AppError::InvalidInput(
                "a colour is required: --hex RRGGBB or --red/--green/--blue".into(),
            ))
        }
        _ => {
            return Err(AppError::InvalidInput(
                "give all three of --red/--green/--blue, or --hex".into(),
            ))
        }
    };
    Ok((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
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
        SegmentCommand::Color {
            segments,
            hex,
            red,
            green,
            blue,
            value,
            ..
        } => {
            if let Some(v) = value {
                parse_value(v)?;
            } else {
                parse_segments(segments.as_deref().unwrap_or_default())?;
                rgb_from_args(hex.as_deref(), *red, *green, *blue)?;
            }
        }
        SegmentCommand::Brightness {
            segments,
            brightness,
            value,
            ..
        } => {
            if let Some(v) = value {
                parse_value(v)?;
            } else {
                parse_segments(segments.as_deref().unwrap_or_default())?;
                if brightness.is_some_and(|b| b > 100) {
                    return Err(AppError::InvalidInput("brightness is 0-100".into()).into());
                }
            }
        }
        SegmentCommand::Info { .. } => {}
    }
    Ok(())
}

fn device_limits(dev: &Device, instance: &str) -> Result<Limits, AppError> {
    dev.info
        .capabilities
        .iter()
        .find(|c| c.capability_type == SEGMENTS && c.instance == instance)
        .and_then(|c| limits(&c.parameters))
        .ok_or_else(|| {
            AppError::UnsupportedOperation(format!(
                "{} does not support {instance}; see `govee segment info`",
                dev.name()
            ))
        })
}

pub async fn handle(ctx: &Ctx, cmd: &SegmentCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    match cmd {
        SegmentCommand::Color {
            device,
            segments,
            hex,
            red,
            green,
            blue,
            value,
        } => {
            let dev = resolve::resolve_device(&api, device).await?;
            if let Some(v) = value {
                dev.set_segment_color(parse_value(v)?).await?;
                emit_one(
                    ctx.json,
                    "segment-control",
                    json!({ "device": dev.name(), "segment_color": "set" }),
                );
                return Ok(());
            }
            let spec = parse_segments(segments.as_deref().unwrap_or_default())?;
            let rgb = rgb_from_args(hex.as_deref(), *red, *green, *blue)?;
            let batches = batches(&spec, device_limits(&dev, "segmentedColorRgb")?)?;
            for batch in &batches {
                dev.set_segment_color(json!({ "segment": batch, "rgb": rgb }))
                    .await?;
            }
            emit_one(
                ctx.json,
                "segment-control",
                json!({
                    "device": dev.name(),
                    "segments": batches.concat(),
                    "hex": format!("#{rgb:06X}"),
                    "calls": batches.len(),
                }),
            );
        }
        SegmentCommand::Brightness {
            device,
            segments,
            brightness,
            value,
        } => {
            let dev = resolve::resolve_device(&api, device).await?;
            if let Some(v) = value {
                dev.set_segment_brightness(parse_value(v)?).await?;
                emit_one(
                    ctx.json,
                    "segment-control",
                    json!({ "device": dev.name(), "segment_brightness": "set" }),
                );
                return Ok(());
            }
            let spec = parse_segments(segments.as_deref().unwrap_or_default())?;
            let level = brightness.unwrap_or(100);
            let batches = batches(&spec, device_limits(&dev, "segmentedBrightness")?)?;
            for batch in &batches {
                dev.set_segment_brightness(json!({ "segment": batch, "brightness": level }))
                    .await?;
            }
            emit_one(
                ctx.json,
                "segment-control",
                json!({
                    "device": dev.name(),
                    "segments": batches.concat(),
                    "brightness": level,
                    "calls": batches.len(),
                }),
            );
        }
        SegmentCommand::Info { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let segment_caps: Vec<Value> = dev
                .info
                .capabilities
                .iter()
                .filter(|c| c.capability_type == SEGMENTS)
                .map(|c| {
                    let l = limits(&c.parameters);
                    json!({
                        "instance": c.instance,
                        "segments": l.map(|l| u32::from(l.max_index) + 1),
                        "per_call": l.map(|l| l.per_call),
                        "parameters": c.parameters,
                    })
                })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_specs_parse() {
        assert_eq!(parse_segments("all").unwrap(), Segments::All);
        assert_eq!(parse_segments(" ALL ").unwrap(), Segments::All);
        assert_eq!(
            parse_segments("0,1,2").unwrap(),
            Segments::List(vec![0, 1, 2])
        );
        assert_eq!(
            parse_segments("0-3,7").unwrap(),
            Segments::List(vec![0, 1, 2, 3, 7])
        );
        assert_eq!(
            parse_segments("3, 1, 3").unwrap(),
            Segments::List(vec![1, 3])
        );
        for bad in ["", "a", "3-1", "0,,1", "1-"] {
            assert!(parse_segments(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn limits_and_batches_follow_the_capability() {
        // The H60B0's segmentedColorRgb parameters, as the Platform API sends them.
        let params = json!({"dataType": "STRUCT", "fields": [
            {"fieldName": "segment", "size": {"min": 1, "max": 8}, "dataType": "Array",
             "elementRange": {"min": 0, "max": 7}, "elementType": "INTEGER", "required": true},
            {"fieldName": "rgb", "dataType": "INTEGER", "range": {"min": 0, "max": 16777215}}
        ]});
        let l = limits(&params).unwrap();
        assert_eq!(
            l,
            Limits {
                max_index: 7,
                per_call: 8
            }
        );
        assert_eq!(
            batches(&Segments::All, l).unwrap(),
            vec![(0..=7).collect::<Vec<u8>>()]
        );
        let small = Limits {
            max_index: 14,
            per_call: 4,
        };
        assert_eq!(
            batches(&Segments::All, small).unwrap(),
            vec![
                vec![0, 1, 2, 3],
                vec![4, 5, 6, 7],
                vec![8, 9, 10, 11],
                vec![12, 13, 14]
            ]
        );
        let e = batches(&Segments::List(vec![0, 9]), l)
            .unwrap_err()
            .to_string();
        assert!(e.contains("segment 9 is out of range"), "{e}");
        assert!(limits(&json!({"dataType": "ENUM"})).is_none());
    }

    #[test]
    fn colours_pack_from_hex_or_channels() {
        assert_eq!(
            rgb_from_args(Some("#FF1493"), None, None, None).unwrap(),
            0xFF1493
        );
        assert_eq!(
            rgb_from_args(None, Some(255), Some(0), Some(128)).unwrap(),
            0xFF0080
        );
        assert!(rgb_from_args(None, Some(1), None, None).is_err());
        assert!(rgb_from_args(None, None, None, None).is_err());
    }
}
