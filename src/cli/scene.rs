use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use pk_cli_core::output::emit_one;

use super::output::emit_list_with;
use super::Ctx;
use crate::error::AppError;
use crate::models::device::Device;
use crate::resolve;

/// Normalize a string for comparison by replacing all Unicode whitespace
/// (including non-breaking spaces \u{00a0}) with regular spaces and lowercasing.
pub fn normalize_for_match(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .to_lowercase()
}

#[derive(Subcommand, Debug)]
pub enum SceneCommand {
    /// List a device's dynamic scenes (scene-list/v1).
    #[command(visible_alias = "ls")]
    List {
        /// Device name or ID
        device: String,
    },
    /// List user-created DIY scenes (diy-scene-list/v1).
    ListDiy {
        /// Device name or ID
        device: String,
    },
    /// List saved snapshot scenes (snapshot-list/v1).
    ListSnapshots {
        /// Device name or ID
        device: String,
    },
    /// Activate a scene by name.
    Activate {
        /// Device name or ID
        device: String,
        /// Scene name (case-insensitive, partial match supported)
        name: String,
    },
    /// Activate a snapshot by name.
    ActivateSnapshot {
        /// Device name or ID
        device: String,
        /// Snapshot name (case-insensitive, partial match supported)
        name: String,
    },
}

pub async fn handle(ctx: &Ctx, cmd: &SceneCommand) -> Result<(), CliError> {
    let api = ctx.api()?;
    match cmd {
        SceneCommand::List { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let scenes = dev.get_scenes().await?;
            let items = extract_scene_names_for_instance(&scenes, "lightScene");
            emit_named_list(ctx, "scene", dev.name(), items);
        }
        SceneCommand::ListDiy { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let scenes = dev.get_diy_scenes().await?;
            let items = extract_scene_names(&scenes);
            emit_named_list(ctx, "diy-scene", dev.name(), items);
        }
        SceneCommand::ListSnapshots { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            require_snapshots(&dev)?;
            let scenes = dev.get_scenes().await?;
            let items = extract_scene_names_for_instance(&scenes, "snapshot");
            emit_named_list(ctx, "snapshot", dev.name(), items);
        }
        SceneCommand::Activate { device, name } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let scenes = dev.get_scenes().await?;
            let all = extract_scene_names_for_instance(&scenes, "lightScene");
            let (scene_name, param_id, id) = pick_scene(&all, name, "scene", dev.name())?;
            dev.activate_scene(param_id, id).await?;
            emit_one(
                ctx.json,
                "scene-activation",
                json!({ "device": dev.name(), "scene": scene_name, "activated": true }),
            );
        }
        SceneCommand::ActivateSnapshot { device, name } => {
            let dev = resolve::resolve_device(&api, device).await?;
            require_snapshots(&dev)?;
            let scenes = dev.get_scenes().await?;
            let all = extract_scene_names_for_instance(&scenes, "snapshot");
            let (scene_name, param_id, id) = pick_scene(&all, name, "snapshot", dev.name())?;
            dev.activate_snapshot(param_id, id).await?;
            emit_one(
                ctx.json,
                "snapshot-activation",
                json!({ "device": dev.name(), "snapshot": scene_name, "activated": true }),
            );
        }
    }
    Ok(())
}

fn emit_named_list(ctx: &Ctx, record: &str, device: &str, items: Vec<Value>) {
    emit_list_with(
        ctx.json,
        record,
        &[("device", json!(device))],
        items,
        &["name"],
    );
}

fn require_snapshots(dev: &Device) -> Result<(), AppError> {
    if dev.info.has_snapshots() {
        Ok(())
    } else {
        Err(AppError::UnsupportedOperation(format!(
            "{} does not support snapshot scenes",
            dev.name()
        )))
    }
}

/// The scene matching `name` and the `(paramId, id)` pair its activation
/// needs; a miss is exit 4.
fn pick_scene(
    scenes: &[Value],
    name: &str,
    what: &str,
    device: &str,
) -> Result<(String, Value, Value), AppError> {
    let found = find_scene_by_name(scenes, &normalize_for_match(name)).ok_or_else(|| {
        AppError::DeviceNotFound(format!("{what} `{name}` not found for device `{device}`"))
    })?;
    let scene_name = found
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(name)
        .to_string();
    let value = found
        .get("value")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::Api {
            message: format!("{what} `{scene_name}` has no activation value"),
            error_code: None,
        })?;
    let param_id = value.get("paramId").cloned().unwrap_or(json!(0));
    let id = value.get("id").cloned().unwrap_or(json!(0));
    Ok((scene_name, param_id, id))
}

fn find_scene_by_name<'a>(scenes: &'a [Value], name_normalized: &str) -> Option<&'a Value> {
    let name_of = |s: &'a Value| {
        s.get("name")
            .and_then(|n| n.as_str())
            .map(normalize_for_match)
    };
    scenes
        .iter()
        .find(|s| name_of(s).as_deref() == Some(name_normalized))
        .or_else(|| {
            scenes
                .iter()
                .find(|s| name_of(s).is_some_and(|n| n.contains(name_normalized)))
        })
}

/// Extract scene options from all capabilities (any instance).
pub fn extract_scene_names(data: &Value) -> Vec<Value> {
    options_of(data, None)
}

/// Extract scene options only from capabilities matching a specific instance.
fn extract_scene_names_for_instance(data: &Value, instance: &str) -> Vec<Value> {
    options_of(data, Some(instance))
}

fn options_of(data: &Value, instance: Option<&str>) -> Vec<Value> {
    let mut names = Vec::new();
    let Some(capabilities) = data.get("capabilities").and_then(|v| v.as_array()) else {
        return names;
    };
    for cap in capabilities {
        if let Some(want) = instance {
            let have = cap.get("instance").and_then(|v| v.as_str()).unwrap_or("");
            if have != want {
                continue;
            }
        }
        let Some(options) = cap
            .get("parameters")
            .and_then(|p| p.get("options"))
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for option in options {
            if let Some(name) = option.get("name").and_then(|n| n.as_str()) {
                names.push(json!({ "name": name, "value": option.get("value") }));
            }
        }
    }
    names
}
