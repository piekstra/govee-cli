//! Per-segment colour and brightness (`devices.capabilities.segment_color_setting`).
//! Segments are given as a list (`0,1,2`), ranges (`0-3,7`) or `all`; the
//! device's capability says how many there are and how many one call may
//! address, and longer lists are sent in as many calls as needed.
//!
//! `validate` turns the arguments into the one checked `Plan` that the
//! handler then sends, so what was validated is what goes on the wire.

use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use super::light::parse_hex_color;
use super::Ctx;
use crate::error::AppError;
use crate::models::device::{validate_brightness, Device};
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
        /// Brightness 1-100
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

/// The two segment instances the Platform API defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Color,
    Brightness,
}

impl SegmentKind {
    pub fn instance(self) -> &'static str {
        match self {
            SegmentKind::Color => "segmentedColorRgb",
            SegmentKind::Brightness => "segmentedBrightness",
        }
    }

    /// The `"set"` marker key the 0.2 DTO carried for this instance.
    fn marker(self) -> &'static str {
        match self {
            SegmentKind::Color => "segment_color",
            SegmentKind::Brightness => "segment_brightness",
        }
    }
}

/// Which segments a spec names: an explicit list, or every one the device has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segments {
    All,
    List(Vec<u8>),
}

/// What a write sets on its segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Rgb(u32),
    Brightness(u8),
}

impl Setting {
    fn kind(self) -> SegmentKind {
        match self {
            Setting::Rgb(_) => SegmentKind::Color,
            Setting::Brightness(_) => SegmentKind::Brightness,
        }
    }

    fn value_for(self, batch: &[u8]) -> Value {
        match self {
            Setting::Rgb(rgb) => json!({ "segment": batch, "rgb": rgb }),
            Setting::Brightness(b) => json!({ "segment": batch, "brightness": b }),
        }
    }
}

/// The one checked description of a write, shared by the gate and the handler.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// `--segments` and a setting: batched against the device's limits.
    Batched { spec: Segments, setting: Setting },
    /// The deprecated `--value`: the Platform API's value sent as given.
    Raw { kind: SegmentKind, value: Value },
}

/// What a report describes: a setting the CLI built, or a raw value.
#[derive(Debug, Clone, Copy)]
pub enum Reported<'a> {
    Setting(Setting),
    Raw(SegmentKind, &'a Value),
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

/// The checked plan for a write; `None` for `info`.
pub fn plan(cmd: &SegmentCommand) -> Result<Option<Plan>, AppError> {
    Ok(match cmd {
        SegmentCommand::Color { value: Some(v), .. } => Some(Plan::Raw {
            kind: SegmentKind::Color,
            value: parse_value(v)?,
        }),
        SegmentCommand::Color {
            segments,
            hex,
            red,
            green,
            blue,
            ..
        } => Some(Plan::Batched {
            spec: parse_segments(segments.as_deref().unwrap_or_default())?,
            setting: Setting::Rgb(rgb_from_args(hex.as_deref(), *red, *green, *blue)?),
        }),
        SegmentCommand::Brightness { value: Some(v), .. } => Some(Plan::Raw {
            kind: SegmentKind::Brightness,
            value: parse_value(v)?,
        }),
        SegmentCommand::Brightness {
            segments,
            brightness,
            ..
        } => {
            // The same 1..=100 the whole-lamp brightness takes.
            let level = brightness.unwrap_or(100);
            validate_brightness(level)?;
            Some(Plan::Batched {
                spec: parse_segments(segments.as_deref().unwrap_or_default())?,
                setting: Setting::Brightness(level),
            })
        }
        SegmentCommand::Info { .. } => None,
    })
}

/// Argument checks that need no credential (exit 2 before the keychain):
/// the plan the handler sends.
pub fn validate(cmd: &SegmentCommand) -> Result<Option<Plan>, CliError> {
    Ok(plan(cmd)?)
}

fn device_limits(dev: &Device, kind: SegmentKind) -> Result<Limits, AppError> {
    dev.info
        .capabilities
        .iter()
        .find(|c| c.capability_type == SEGMENTS && c.instance == kind.instance())
        .and_then(|c| limits(&c.parameters))
        .ok_or_else(|| {
            AppError::UnsupportedOperation(format!(
                "{} does not support {}; see `govee segment info`",
                dev.name(),
                kind.instance()
            ))
        })
}

/// A failure part-way through the batches, with what already went through:
/// the caller can finish with a narrower `--segments` instead of guessing.
/// The error keeps its class (auth stays exit 3, not-found exit 4); the
/// note rides in the message where there is one and on stderr otherwise.
fn partial(e: AppError, done: &[Vec<u8>], failed: &[u8]) -> AppError {
    let applied: Vec<u8> = done.concat();
    let note = if applied.is_empty() {
        format!("no segments were changed (batch {failed:?} failed)")
    } else {
        format!("segments {applied:?} were already set; batch {failed:?} failed")
    };
    match e {
        AppError::Api {
            message,
            error_code,
        } => AppError::Api {
            message: format!("{message}; {note}"),
            error_code,
        },
        AppError::RateLimited(m) => AppError::RateLimited(format!("{m}; {note}")),
        AppError::UnsupportedOperation(m) => AppError::UnsupportedOperation(format!("{m}; {note}")),
        AppError::InvalidInput(m) => AppError::InvalidInput(format!("{m}; {note}")),
        AppError::DeviceNotFound(m) => AppError::DeviceNotFound(format!("{m}; {note}")),
        other => {
            eprintln!("note: {note}");
            other
        }
    }
}

async fn send(dev: &Device, kind: SegmentKind, value: Value) -> Result<(), AppError> {
    match kind {
        SegmentKind::Color => dev.set_segment_color(value).await,
        SegmentKind::Brightness => dev.set_segment_brightness(value).await,
    }
}

/// The `segment-control/v1` row: the device, the instance, what was set,
/// on which segments, in how many calls. Both paths fill the same keys,
/// including the 0.2 `segment_color` / `segment_brightness: "set"` marker.
pub fn row(device: &str, segments: Vec<Value>, calls: usize, reported: Reported<'_>) -> Value {
    let kind = match reported {
        Reported::Setting(s) => s.kind(),
        Reported::Raw(k, _) => k,
    };
    let mut row = serde_json::Map::new();
    row.insert("device".into(), json!(device));
    row.insert("instance".into(), json!(kind.instance()));
    row.insert("segments".into(), Value::Array(segments));
    row.insert("calls".into(), json!(calls));
    match reported {
        Reported::Setting(Setting::Rgb(rgb)) => {
            row.insert("hex".into(), json!(format!("#{rgb:06X}")));
        }
        Reported::Setting(Setting::Brightness(b)) => {
            row.insert("brightness".into(), json!(b));
        }
        Reported::Raw(_, v) => {
            if let Some(rgb) = v.get("rgb").and_then(Value::as_u64) {
                row.insert("hex".into(), json!(format!("#{rgb:06X}")));
            }
            if let Some(b) = v.get("brightness") {
                row.insert("brightness".into(), b.clone());
            }
        }
    }
    row.insert(kind.marker().into(), json!("set"));
    Value::Object(row)
}

pub async fn handle(ctx: &Ctx, cmd: &SegmentCommand) -> Result<(), CliError> {
    let plan = validate(cmd)?;
    let api = ctx.api()?;
    let device = match cmd {
        SegmentCommand::Color { device, .. }
        | SegmentCommand::Brightness { device, .. }
        | SegmentCommand::Info { device } => device,
    };
    let dev = resolve::resolve_device(&api, device).await?;
    match plan {
        Some(Plan::Raw { kind, value }) => {
            send(&dev, kind, value.clone()).await?;
            let segments = value
                .get("segment")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            emit_one(
                ctx.json,
                "segment-control",
                row(dev.name(), segments, 1, Reported::Raw(kind, &value)),
            );
        }
        Some(Plan::Batched { spec, setting }) => {
            let kind = setting.kind();
            let batches = batches(&spec, device_limits(&dev, kind)?)?;
            for (i, batch) in batches.iter().enumerate() {
                send(&dev, kind, setting.value_for(batch))
                    .await
                    .map_err(|e| partial(e, &batches[..i], batch))?;
            }
            let segments = batches.concat().into_iter().map(|s| json!(s)).collect();
            emit_one(
                ctx.json,
                "segment-control",
                row(
                    dev.name(),
                    segments,
                    batches.len(),
                    Reported::Setting(setting),
                ),
            );
        }
        None => {
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

    #[test]
    fn the_plan_is_what_gets_sent() {
        let color = SegmentCommand::Color {
            device: "Lamp".into(),
            segments: Some("0-1".into()),
            hex: Some("FF1493".into()),
            red: None,
            green: None,
            blue: None,
            value: None,
        };
        let p = plan(&color).unwrap().unwrap();
        assert_eq!(
            p,
            Plan::Batched {
                spec: Segments::List(vec![0, 1]),
                setting: Setting::Rgb(0xFF1493)
            }
        );
        assert_eq!(
            Setting::Rgb(0xFF1493).value_for(&[0, 1]),
            json!({"segment": [0, 1], "rgb": 0xFF1493})
        );
        assert_eq!(
            Setting::Brightness(40).value_for(&[7]),
            json!({"segment": [7], "brightness": 40})
        );
        let raw = SegmentCommand::Brightness {
            device: "Lamp".into(),
            segments: None,
            brightness: None,
            value: Some(r#"{"segment":[2],"brightness":30}"#.into()),
        };
        assert!(matches!(
            plan(&raw).unwrap().unwrap(),
            Plan::Raw {
                kind: SegmentKind::Brightness,
                ..
            }
        ));
        for level in [0u8, 101] {
            let bad = SegmentCommand::Brightness {
                device: "Lamp".into(),
                segments: Some("0".into()),
                brightness: Some(level),
                value: None,
            };
            assert!(plan(&bad).is_err(), "brightness {level} is outside 1-100");
        }
        assert!(plan(&SegmentCommand::Info { device: "x".into() })
            .unwrap()
            .is_none());
    }

    #[test]
    fn both_paths_emit_the_same_dto_keys() {
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        let built = row(
            "Lamp",
            vec![json!(0), json!(1)],
            1,
            Reported::Setting(Setting::Rgb(0xFF1493)),
        );
        let raw_value = json!({"segment": [0, 1], "rgb": 0xFF1493});
        let raw = row(
            "Lamp",
            vec![json!(0), json!(1)],
            1,
            Reported::Raw(SegmentKind::Color, &raw_value),
        );
        assert_eq!(keys(&built), keys(&raw));
        assert_eq!(
            built, raw,
            "the same write reports the same row on both paths"
        );
        assert_eq!(built["segment_color"], "set");
        assert_eq!(built["hex"], "#FF1493");
        assert_eq!(built["instance"], "segmentedColorRgb");
        let b = row(
            "Lamp",
            vec![json!(7)],
            1,
            Reported::Setting(Setting::Brightness(40)),
        );
        assert_eq!(b["segment_brightness"], "set");
        assert_eq!(b["brightness"], 40);
        assert!(b.get("segment_color").is_none());
    }

    #[test]
    fn a_failure_mid_batch_says_what_was_already_set() {
        let e = partial(
            AppError::InvalidInput("Govee said no".into()),
            &[vec![0, 1], vec![2, 3]],
            &[4, 5],
        );
        let m = e.to_string();
        assert!(m.contains("segments [0, 1, 2, 3] were already set"), "{m}");
        assert!(m.contains("batch [4, 5] failed"), "{m}");
        let first = partial(AppError::InvalidInput("x".into()), &[], &[0]).to_string();
        assert!(first.contains("no segments were changed"), "{first}");
        // The class survives: an auth failure mid-run still says `auth login` (exit 3).
        assert!(matches!(
            partial(AppError::NotAuthenticated, &[vec![0]], &[1]),
            AppError::NotAuthenticated
        ));
        assert!(matches!(
            partial(AppError::RateLimited("slow down".into()), &[vec![0]], &[1]),
            AppError::RateLimited(m) if m.contains("slow down") && m.contains("[0] were already set")
        ));
    }
}
