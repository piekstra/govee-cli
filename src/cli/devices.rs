use clap::Subcommand;
use serde_json::json;

use crate::cli::output::print_output;
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand)]
pub enum DevicesCommand {
    /// List all devices
    List,
    /// Get device details
    Get {
        /// Device name or ID
        device: String,
    },
    /// Search devices by partial name
    Search {
        /// Search query
        query: String,
    },
    /// Show device capabilities in detail
    Caps {
        /// Device name or ID
        device: String,
    },
}

pub async fn handle(cmd: &DevicesCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        DevicesCommand::List => handle_list(config).await,
        DevicesCommand::Get { device } => handle_get(device, config).await,
        DevicesCommand::Search { query } => handle_search(query, config).await,
        DevicesCommand::Caps { device } => handle_caps(device, config).await,
    }
}

/// The app's view of every device, when the account is logged in: room and
/// connectivity for cloud devices, and the Bluetooth-only devices the
/// Platform API never lists. `None` without an account login.
async fn app_view(config: &RuntimeConfig) -> Option<Vec<crate::api::app::AppDevice>> {
    let session = crate::cli::auth::load_account().ok()?;
    if session.token.is_empty() {
        return None;
    }
    let app = crate::api::app::GoveeApp::new(session.client_id.clone(), config.verbose).ok()?;
    let list = app.device_list(&session.token).await.ok()?;
    Some(crate::api::app::app_devices(&list))
}

async fn handle_list(config: &RuntimeConfig) -> Result<(), AppError> {
    let devices = resolve::fetch_all_devices(config.verbose).await?;
    let app = app_view(config).await;
    let mut list: Vec<serde_json::Value> = devices
        .iter()
        .map(|(info, dtype)| {
            let mut row = json!({
                "name": info.name(),
                "device": info.id(),
                "sku": info.model(),
                "type": dtype.display_name(),
                "category": dtype.category(),
                // Listed by the Platform API, so the cloud can reach it.
                "connectivity": "wifi",
            });
            if let Some(app) = &app {
                if let Some(a) = app.iter().find(|a| a.device == info.id()) {
                    row["room"] = json!(a.room);
                }
            }
            row
        })
        .collect();
    // Devices only the app knows: Bluetooth-only, controllable from a phone
    // next to them and nowhere else.
    if let Some(app) = &app {
        for a in app.iter().filter(|a| a.connectivity == "bluetooth") {
            if !devices.iter().any(|(info, _)| info.id() == a.device) {
                list.push(json!({
                    "name": a.name,
                    "device": a.device,
                    "sku": a.sku,
                    "type": "bluetooth-only",
                    "category": "app-only",
                    "connectivity": "bluetooth",
                    "room": a.room,
                }));
            }
        }
    }

    print_output(&json!(list), config.output_mode);
    Ok(())
}

async fn handle_get(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let capabilities: Vec<serde_json::Value> = dev
        .info
        .capabilities
        .iter()
        .map(|c| {
            json!({
                "type": c.capability_type,
                "instance": c.instance,
            })
        })
        .collect();

    let app = app_view(config).await;
    let room = app
        .as_ref()
        .and_then(|a| a.iter().find(|x| x.device == dev.device_id()).and_then(|x| x.room.clone()));
    print_output(
        &json!({
            "name": dev.name(),
            "device": dev.device_id(),
            "sku": dev.sku(),
            "type": dev.device_type.display_name(),
            "category": dev.device_type.category(),
            "connectivity": "wifi",
            "room": room,
            "capabilities": capabilities,
        }),
        config.output_mode,
    );
    Ok(())
}

async fn handle_search(query: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let devices = resolve::fetch_all_devices(config.verbose).await?;
    let query_lower = query.to_lowercase();
    let matches: Vec<serde_json::Value> = devices
        .iter()
        .filter(|(info, _)| info.name().to_lowercase().contains(&query_lower))
        .map(|(info, dtype)| {
            json!({
                "name": info.name(),
                "device": info.id(),
                "sku": info.model(),
                "type": dtype.display_name(),
            })
        })
        .collect();

    print_output(&json!(matches), config.output_mode);
    Ok(())
}

async fn handle_caps(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let dev = resolve::resolve_device(device, config.verbose).await?;
    let capabilities: Vec<serde_json::Value> = dev
        .info
        .capabilities
        .iter()
        .map(|c| {
            json!({
                "type": c.capability_type,
                "instance": c.instance,
                "parameters": c.parameters,
            })
        })
        .collect();

    print_output(
        &json!({
            "name": dev.name(),
            "sku": dev.sku(),
            "capabilities": capabilities,
        }),
        config.output_mode,
    );
    Ok(())
}
