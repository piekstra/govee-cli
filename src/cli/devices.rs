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

async fn handle_list(config: &RuntimeConfig) -> Result<(), AppError> {
    let devices = resolve::fetch_all_devices(config.verbose).await?;
    let list: Vec<serde_json::Value> = devices
        .iter()
        .map(|(info, dtype)| {
            json!({
                "name": info.name(),
                "device": info.id(),
                "sku": info.model(),
                "type": dtype.display_name(),
                "category": dtype.category(),
            })
        })
        .collect();

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

    print_output(
        &json!({
            "name": dev.name(),
            "device": dev.device_id(),
            "sku": dev.sku(),
            "type": dev.device_type.display_name(),
            "category": dev.device_type.category(),
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
