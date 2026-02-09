use clap::Subcommand;
use serde_json::json;

use crate::cli::output::{print_json, print_output};
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

/// Normalize a string for comparison by replacing all Unicode whitespace
/// (including non-breaking spaces \u{00a0}) with regular spaces and lowercasing.
pub fn normalize_for_match(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .to_lowercase()
}

#[derive(Subcommand)]
pub enum SceneCommand {
    /// List available dynamic scenes
    List {
        /// Device name or ID
        device: String,
    },
    /// List user-created DIY scenes
    ListDiy {
        /// Device name or ID
        device: String,
    },
    /// Activate a scene by name
    Activate {
        /// Device name or ID
        device: String,
        /// Scene name (case-insensitive, partial match supported)
        name: String,
    },
}

pub async fn handle(cmd: &SceneCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        SceneCommand::List { device } => handle_list(device, config).await,
        SceneCommand::ListDiy { device } => handle_list_diy(device, config).await,
        SceneCommand::Activate { device, name } => handle_activate(device, name, config).await,
    }
}

async fn handle_list(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let scenes = dev.get_scenes().await?;

    let scene_names = extract_scene_names(&scenes);
    print_output(
        &json!({
            "device": dev.name(),
            "scenes": scene_names,
        }),
        config.output_mode,
    );
    Ok(())
}

async fn handle_list_diy(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let scenes = dev.get_diy_scenes().await?;

    let scene_names = extract_scene_names(&scenes);
    print_output(
        &json!({
            "device": dev.name(),
            "diy_scenes": scene_names,
        }),
        config.output_mode,
    );
    Ok(())
}

async fn handle_activate(device: &str, name: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let scenes = dev.get_scenes().await?;

    let name_normalized = normalize_for_match(name);

    // Search through scene capabilities for a matching scene name
    let all_scenes = extract_scene_names(&scenes);

    // Try exact match first (normalized), then partial
    let found = all_scenes
        .iter()
        .find(|s| {
            s.get("name")
                .and_then(|n| n.as_str())
                .map(|n| normalize_for_match(n) == name_normalized)
                .unwrap_or(false)
        })
        .or_else(|| {
            all_scenes.iter().find(|s| {
                s.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| normalize_for_match(n).contains(&name_normalized))
                    .unwrap_or(false)
            })
        });

    if let Some(scene) = found {
        let scene_name = scene.get("name").and_then(|n| n.as_str()).unwrap_or(name);
        if let Some(value) = scene.get("value").and_then(|v| v.as_object()) {
            let param_id = value.get("paramId").cloned().unwrap_or(json!(0));
            let id = value.get("id").cloned().unwrap_or(json!(0));
            dev.activate_scene(param_id, id).await?;
            print_json(&json!({
                "device": dev.name(),
                "scene": scene_name,
                "activated": true,
            }));
            return Ok(());
        }
    }

    Err(AppError::InvalidInput(format!(
        "Scene '{}' not found for device '{}'",
        name,
        dev.name()
    )))
}

pub fn extract_scene_names(data: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut names = Vec::new();
    if let Some(capabilities) = data.get("capabilities").and_then(|v| v.as_array()) {
        for cap in capabilities {
            if let Some(params) = cap.get("parameters") {
                if let Some(options) = params.get("options").and_then(|v| v.as_array()) {
                    for option in options {
                        if let Some(name) = option.get("name").and_then(|n| n.as_str()) {
                            names.push(json!({
                                "name": name,
                                "value": option.get("value"),
                            }));
                        }
                    }
                }
            }
        }
    }
    names
}
