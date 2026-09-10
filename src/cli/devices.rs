use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use super::output::{emit_list, emit_one};
use super::Ctx;
use crate::api::app::{self, AppDevice, GoveeApp, PlatformDevice};
use crate::error::AppError;
use crate::resolve;

#[derive(Subcommand, Debug)]
pub enum DevicesCommand {
    /// List every device (device-list/v1); with an account signed in, rooms
    /// and Bluetooth-only devices too.
    #[command(visible_alias = "ls")]
    List,
    /// One device with its capabilities (device/v1).
    Get {
        /// Device name or ID
        device: String,
    },
    /// Devices whose name contains the query (device-list/v1).
    Search {
        /// Search query
        query: String,
    },
    /// Capabilities in full, parameters included (device-capabilities/v1).
    Caps {
        /// Device name or ID
        device: String,
    },
}

pub const COLUMNS: &[&str] = &[
    "name",
    "device",
    "sku",
    "type",
    "category",
    "connectivity",
    "room",
];

pub async fn handle(ctx: &Ctx, cmd: &DevicesCommand) -> Result<(), CliError> {
    match cmd {
        DevicesCommand::List => list(ctx).await,
        DevicesCommand::Get { device } => get(ctx, device).await,
        DevicesCommand::Search { query } => search(ctx, query).await,
        DevicesCommand::Caps { device } => caps(ctx, device).await,
    }
}

/// The app's view of every device, when an account is signed in. `Ok(None)`
/// means no account is configured (the Platform view stands alone); `Err`
/// means an account is configured and the app call failed.
async fn app_view(ctx: &Ctx) -> Result<Option<Vec<AppDevice>>, CliError> {
    let Some(session) = ctx.account()? else {
        return Ok(None);
    };
    if !session.signed_in() {
        return Ok(None);
    }
    let app = GoveeApp::new(session.client_id.clone(), ctx.verbose)?;
    let list = app.device_list(&session.token).await?;
    Ok(Some(app::app_devices(&list)))
}

/// The app view, downgrading a failure to a stderr warning so a stale
/// account session never hides the Platform listing.
async fn app_view_or_warn(ctx: &Ctx) -> Option<Vec<AppDevice>> {
    match app_view(ctx).await {
        Ok(v) => v,
        Err(e) => {
            ctx.note(&format!(
                "warning: app view unavailable ({e}); rooms and Bluetooth-only devices omitted — run `govee auth login-account`"
            ));
            None
        }
    }
}

fn platform_row(
    info: &crate::models::device_info::DeviceInfo,
    dtype: &crate::models::device_type::DeviceType,
) -> PlatformDevice {
    PlatformDevice {
        device: info.id().to_string(),
        sku: info.model().to_string(),
        name: info.name().to_string(),
        kind: dtype.display_name().to_string(),
        category: dtype.category().to_string(),
    }
}

async fn list(ctx: &Ctx) -> Result<(), CliError> {
    let api = ctx.api()?;
    let devices = resolve::fetch_all_devices(&api).await?;
    let platform: Vec<PlatformDevice> = devices
        .iter()
        .map(|(info, dtype)| platform_row(info, dtype))
        .collect();
    let app = app_view_or_warn(ctx).await;
    let rows = app::merge_app_view(&platform, app.as_deref());
    let items: Vec<Value> = rows
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
        .collect();
    emit_list(ctx.json, "device-list", json!({ "items": items }), COLUMNS);
    Ok(())
}

async fn get(ctx: &Ctx, device: &str) -> Result<(), CliError> {
    let api = ctx.api()?;
    let app = app_view_or_warn(ctx).await;
    match resolve::resolve_device(&api, device).await {
        Ok(dev) => {
            let capabilities: Vec<Value> = dev
                .info
                .capabilities
                .iter()
                .map(|c| json!({ "type": c.capability_type, "instance": c.instance }))
                .collect();
            let platform = platform_row(&dev.info, &dev.device_type);
            // The same typed row `devices list` prints, plus capabilities.
            let row = app::merge_app_view(&[platform], app.as_deref())
                .into_iter()
                .next()
                .expect("one platform device yields one row");
            let mut v = serde_json::to_value(row).map_err(AppError::from)?;
            v["capabilities"] = json!(capabilities);
            emit_one(ctx.json, "device", v);
            Ok(())
        }
        // Not on the Platform API: it may be one of the app's Bluetooth-only
        // devices, which `devices list` shows and this command must honour.
        Err(AppError::DeviceNotFound(reason)) => {
            let rows = app::merge_app_view(&[], app.as_deref());
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
                .ok_or(AppError::DeviceNotFound(reason))?;
            let mut v = serde_json::to_value(hit).map_err(AppError::from)?;
            v["capabilities"] = json!([]);
            emit_one(ctx.json, "device", v);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

async fn search(ctx: &Ctx, query: &str) -> Result<(), CliError> {
    let api = ctx.api()?;
    let devices = resolve::fetch_all_devices(&api).await?;
    let query_lower = query.to_lowercase();
    let items: Vec<Value> = devices
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
    emit_list(
        ctx.json,
        "device-list",
        json!({ "query": query, "items": items }),
        &["name", "device", "sku", "type"],
    );
    Ok(())
}

async fn caps(ctx: &Ctx, device: &str) -> Result<(), CliError> {
    let api = ctx.api()?;
    let dev = resolve::resolve_device(&api, device).await?;
    let capabilities: Vec<Value> = dev
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
    emit_one(
        ctx.json,
        "device-capabilities",
        json!({
            "name": dev.name(),
            "sku": dev.sku(),
            "capabilities": capabilities,
        }),
    );
    Ok(())
}
