use clap::Subcommand;
use serde_json::{json, Value};

use crate::api::app::GoveeApp;
use crate::cli::auth::load_account;
use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;

#[derive(Subcommand)]
pub enum RoomsCommand {
    /// The rooms in the Govee Home app with their device counts
    /// (needs `auth login-account`)
    List,
    /// Every device with its room, as `device-rooms/v1` — pipe into
    /// `ghome audit --expect -`. Ids are `<SKU>_<MAC>`, the id Google Home
    /// sees for Govee devices.
    Devices,
}

pub async fn handle(cmd: &RoomsCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    let session = load_account()?;
    if session.token.is_empty() {
        return Err(AppError::NotAuthenticated);
    }
    let app = GoveeApp::new(session.client_id.clone(), config.verbose)?;
    let list = app.device_list(&session.token).await?;
    let groups: Vec<(i64, String)> = list
        .get("groups")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some((
                        g.get("groupId")?.as_i64()?,
                        g.get("groupName")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let room_name = |gid: i64| groups.iter().find(|(id, _)| *id == gid).map(|(_, n)| n.clone());
    let devices: Vec<&Value> = list
        .get("devices")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    match cmd {
        RoomsCommand::List => {
            let rooms: Vec<Value> = groups
                .iter()
                .map(|(gid, name)| {
                    let n = devices
                        .iter()
                        .filter(|d| d.get("groupId").and_then(Value::as_i64) == Some(*gid))
                        .count();
                    json!({"room_id": gid, "name": name, "devices": n})
                })
                .collect();
            let ungrouped = devices
                .iter()
                .filter(|d| {
                    d.get("groupId")
                        .and_then(Value::as_i64)
                        .map(|g| room_name(g).is_none())
                        .unwrap_or(true)
                })
                .count();
            print_json(&json!({"rooms": rooms, "devices_without_room": ungrouped}));
            Ok(())
        }
        RoomsCommand::Devices => {
            let items: Vec<Value> = devices
                .iter()
                .filter_map(|d| {
                    let sku = d.get("sku")?.as_str()?;
                    let mac = d.get("device")?.as_str()?;
                    let name = d.get("deviceName").and_then(Value::as_str).unwrap_or("");
                    let room = d.get("groupId").and_then(Value::as_i64).and_then(room_name)?;
                    Some(json!({
                        "id": format!("{sku}_{mac}"),
                        "name": name,
                        "room": room,
                        "source": "govee",
                    }))
                })
                .collect();
            print_json(&json!({"schema": "device-rooms/v1", "items": items}));
            Ok(())
        }
    }
}
