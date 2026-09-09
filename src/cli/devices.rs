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

/// The app's view of every device, when the account is logged in. `Ok(None)`
/// means no account is configured (the Platform view stands alone);
/// `Err` means an account is configured and the app call failed, which the
/// caller reports rather than silently degrading.
async fn app_view(
    config: &RuntimeConfig,
) -> Result<Option<Vec<crate::api::app::AppDevice>>, AppError> {
    let session = match crate::cli::auth::load_account() {
        Ok(s) if !s.token.is_empty() => s,
        // No account stored (or an empty one): the Platform view stands alone.
        Ok(_) | Err(AppError::NotAuthenticated) => return Ok(None),
        // The keychain itself refused: that is a failure, not "no account".
        Err(e) => return Err(e),
    };
    let app = crate::api::app::GoveeApp::new(session.client_id.clone(), config.verbose)?;
    let list = app.device_list(&session.token).await?;
    Ok(Some(crate::api::app::app_devices(&list)))
}

/// Fetch the app view, downgrading a failure to a stderr warning so a stale
/// account session never hides the Platform listing.
async fn app_view_or_warn(config: &RuntimeConfig) -> Option<Vec<crate::api::app::AppDevice>> {
    match app_view(config).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("warning: app view unavailable ({e}); rooms and Bluetooth-only devices omitted — run `govee auth login-account`");
            None
        }
    }
}

async fn handle_list(config: &RuntimeConfig) -> Result<(), AppError> {
    let devices = resolve::fetch_all_devices(config.verbose).await?;
    let platform: Vec<crate::api::app::PlatformDevice> = devices
        .iter()
        .map(|(info, dtype)| crate::api::app::PlatformDevice {
            device: info.id().to_string(),
            sku: info.model().to_string(),
            name: info.name().to_string(),
            kind: dtype.display_name().to_string(),
            category: dtype.category().to_string(),
        })
        .collect();
    let app = app_view_or_warn(config).await;
    let rows = crate::api::app::merge_app_view(&platform, app.as_deref());
    print_output(&json!(rows), config.output_mode);
    Ok(())
}

async fn handle_get(device: &str, config: &RuntimeConfig) -> Result<(), AppError> {
    let app = app_view_or_warn(config).await;
    match resolve::resolve_device(device, config.verbose).await {
        Ok(dev) => {
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
            let platform = crate::api::app::PlatformDevice {
                device: dev.device_id().to_string(),
                sku: dev.sku().to_string(),
                name: dev.name().to_string(),
                kind: dev.device_type.display_name().to_string(),
                category: dev.device_type.category().to_string(),
            };
            // The same typed row `devices list` prints, plus capabilities.
            let row = crate::api::app::merge_app_view(&[platform], app.as_deref())
                .into_iter()
                .next()
                .expect("one platform device yields one row");
            let mut v = serde_json::to_value(row)?;
            v["capabilities"] = json!(capabilities);
            print_output(&v, config.output_mode);
            Ok(())
        }
        // Not on the Platform API: it may be one of the app's Bluetooth-only
        // devices, which `devices list` shows and this command must honour.
        Err(AppError::DeviceNotFound(_)) => {
            let rows = crate::api::app::merge_app_view(&[], app.as_deref());
            let want = device.trim().to_lowercase();
            let hit = rows
                .iter()
                .find(|r| {
                    r.name.to_lowercase() == want
                        || format!("{}_{}", r.sku, r.device).to_lowercase() == want
                })
                .or_else(|| {
                    let partial: Vec<_> = rows
                        .iter()
                        .filter(|r| r.name.to_lowercase().contains(&want))
                        .collect();
                    if partial.len() == 1 {
                        Some(partial[0])
                    } else {
                        None
                    }
                })
                .ok_or_else(|| AppError::DeviceNotFound(device.to_string()))?;
            let mut v = serde_json::to_value(hit)?;
            v["capabilities"] = json!([]);
            print_output(&v, config.output_mode);
            Ok(())
        }
        Err(e) => Err(e),
    }
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
