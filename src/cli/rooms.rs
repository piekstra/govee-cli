use clap::Subcommand;
use serde_json::{json, Value};

use crate::api::app::GoveeApp;
use crate::cli::auth::load_account;
use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;

#[derive(Subcommand)]
pub enum RoomsCommand {
    /// Move a device into a room. Asks first unless --force.
    Move {
        /// Device name, or `SKU_MAC` id
        device: String,
        /// Target room name (or id)
        #[arg(long)]
        room: String,
        /// Skip the confirmation prompt (required when non-interactive)
        #[arg(long)]
        force: bool,
    },
    /// Create a room. Asks first unless --force.
    Create {
        name: String,
        #[arg(long)]
        force: bool,
    },
    /// Rename a room. Asks first unless --force.
    Rename {
        room: String,
        name: String,
        #[arg(long)]
        force: bool,
    },
    /// Delete an empty room. Asks first unless --force.
    Delete {
        room: String,
        #[arg(long)]
        force: bool,
    },
    /// The rooms in the Govee Home app with their device counts
    /// (needs `auth login-account`)
    List,
    /// Every device with its room, as `device-rooms/v1` — pipe into
    /// `ghome audit --expect -`. Ids are `<SKU>_<MAC>`, the id Google Home
    /// sees for Govee devices.
    Devices,
}

fn confirm(force: bool, prompt: &str) -> Result<(), AppError> {
    use std::io::IsTerminal;
    if force {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Err(AppError::InvalidInput(format!(
            "{prompt} — pass --force to run non-interactively"
        )));
    }
    let ok = dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput("cancelled".into()))
    }
}

fn find_room<'a>(groups: &'a [(i64, String)], q: &str) -> Result<&'a (i64, String), AppError> {
    if let Ok(id) = q.parse::<i64>() {
        if let Some(g) = groups.iter().find(|(gid, _)| *gid == id) {
            return Ok(g);
        }
    }
    let want = q.trim().to_lowercase();
    let exact: Vec<&(i64, String)> = groups
        .iter()
        .filter(|(_, n)| n.trim().to_lowercase() == want)
        .collect();
    if exact.len() == 1 {
        return Ok(exact[0]);
    }
    let partial: Vec<&(i64, String)> = groups
        .iter()
        .filter(|(_, n)| n.to_lowercase().contains(&want))
        .collect();
    match partial.len() {
        1 => Ok(partial[0]),
        0 => Err(AppError::DeviceNotFound(format!("no room matching `{q}`"))),
        _ => Err(AppError::InvalidInput(format!(
            "`{q}` matches more than one room: {}",
            partial
                .iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn find_device<'a>(
    devices: &'a [crate::api::app::AppDevice],
    q: &str,
) -> Result<&'a crate::api::app::AppDevice, AppError> {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    };
    if let Some(d) = devices
        .iter()
        .find(|d| norm(&format!("{}_{}", d.sku, d.device)) == norm(q))
    {
        return Ok(d);
    }
    let want = q.trim().to_lowercase();
    let exact: Vec<_> = devices
        .iter()
        .filter(|d| d.name.trim().to_lowercase() == want)
        .collect();
    if exact.len() == 1 {
        return Ok(exact[0]);
    }
    let partial: Vec<_> = devices
        .iter()
        .filter(|d| d.name.to_lowercase().contains(&want))
        .collect();
    match partial.len() {
        1 => Ok(partial[0]),
        0 => Err(AppError::DeviceNotFound(format!(
            "no device matching `{q}`"
        ))),
        _ => Err(AppError::InvalidInput(format!(
            "`{q}` matches more than one device: {}",
            partial
                .iter()
                .map(|d| format!("{} ({}_{})", d.name, d.sku, d.device))
                .collect::<Vec<_>>()
                .join("; ")
        ))),
    }
}

pub async fn handle(cmd: &RoomsCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    let session = load_account()?;
    if session.token.is_empty() {
        return Err(AppError::NotAuthenticated);
    }
    let app = GoveeApp::new(session.client_id.clone(), config.verbose)?;
    let list = app.device_list(&session.token).await?;
    if let Some(done) = handle_write(cmd, &app, &session.token, &list).await? {
        print_json(&done);
        return Ok(());
    }
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
    let room_name = |gid: i64| {
        groups
            .iter()
            .find(|(id, _)| *id == gid)
            .map(|(_, n)| n.clone())
    };
    let devices: Vec<&Value> = list
        .get("devices")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    match cmd {
        RoomsCommand::Move { .. }
        | RoomsCommand::Create { .. }
        | RoomsCommand::Rename { .. }
        | RoomsCommand::Delete { .. } => unreachable!("handled by handle_write"),
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
            let items: Vec<Value> = crate::api::app::app_devices(&list)
                .into_iter()
                .filter_map(|d| {
                    let room = d.room?;
                    Some(json!({
                        "id": format!("{}_{}", d.sku, d.device),
                        "name": d.name,
                        "room": room,
                        "source": "govee",
                        "cloud": d.connectivity == crate::api::app::Connectivity::Wifi,
                        "connectivity": d.connectivity,
                    }))
                })
                .collect();
            print_json(&json!({"schema": "device-rooms/v1", "items": items}));
            Ok(())
        }
    }
}

/// The write commands; `Ok(None)` for reads. Every write is verified by
/// re-reading the device list before it is reported.
async fn handle_write(
    cmd: &RoomsCommand,
    app: &GoveeApp,
    token: &str,
    list: &Value,
) -> Result<Option<Value>, AppError> {
    use crate::api::app::app_devices;
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
    let devices = app_devices(list);
    let members_of = |room: &str| -> Vec<(String, String)> {
        devices
            .iter()
            .filter(|d| d.room.as_deref() == Some(room))
            .map(|d| (d.device.clone(), d.sku.clone()))
            .collect()
    };

    match cmd {
        RoomsCommand::Move {
            device,
            room,
            force,
        } => {
            let d = find_device(&devices, device)?.clone();
            let (gid, gname) = find_room(&groups, room)?.clone();
            if d.room.as_deref() == Some(&gname) {
                return Ok(Some(
                    json!({"device": d.name, "room": gname, "changed": false}),
                ));
            }
            confirm(
                *force,
                &format!(
                    "Move \"{}\" from {} to {}?",
                    d.name,
                    d.room.as_deref().unwrap_or("no room"),
                    gname
                ),
            )?;
            let mut members = members_of(&gname);
            members.push((d.device.clone(), d.sku.clone()));
            app.edit_room(token, gid, &gname, &members).await?;
            let after = app_devices(&app.device_list(token).await?);
            let now = after
                .iter()
                .find(|x| x.device == d.device)
                .and_then(|x| x.room.clone());
            if now.as_deref() != Some(&gname) {
                return Err(AppError::Api {
                    message: format!(
                        "Govee accepted the move but lists the device in {} on read-back",
                        now.unwrap_or_else(|| "no room".into())
                    ),
                    error_code: None,
                });
            }
            Ok(Some(
                json!({"device": d.name, "id": format!("{}_{}", d.sku, d.device), "from_room": d.room, "room": gname, "changed": true}),
            ))
        }
        RoomsCommand::Create { name, force } => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 22 {
                return Err(AppError::InvalidInput(
                    "room names are 1–22 characters".into(),
                ));
            }
            if groups.iter().any(|(_, n)| n.eq_ignore_ascii_case(name)) {
                return Err(AppError::InvalidInput(format!(
                    "a room named `{name}` already exists"
                )));
            }
            confirm(*force, &format!("Create room \"{name}\"?"))?;
            let gid = app.create_room(token, name).await?;
            Ok(Some(json!({"room_id": gid, "name": name, "created": true})))
        }
        RoomsCommand::Rename { room, name, force } => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 22 {
                return Err(AppError::InvalidInput(
                    "room names are 1–22 characters".into(),
                ));
            }
            let (gid, old) = find_room(&groups, room)?.clone();
            confirm(*force, &format!("Rename \"{old}\" to \"{name}\"?"))?;
            app.edit_room(token, gid, name, &members_of(&old)).await?;
            let after = app.device_list(token).await?;
            let renamed = after
                .get("groups")
                .and_then(Value::as_array)
                .is_some_and(|a| {
                    a.iter().any(|g| {
                        g.get("groupId").and_then(Value::as_i64) == Some(gid)
                            && g.get("groupName").and_then(Value::as_str) == Some(name)
                    })
                });
            if !renamed {
                return Err(AppError::Api {
                    message: "Govee accepted the rename but still shows the old name on read-back"
                        .into(),
                    error_code: None,
                });
            }
            Ok(Some(
                json!({"room_id": gid, "previous_name": old, "name": name, "changed": true}),
            ))
        }
        RoomsCommand::Delete { room, force } => {
            let (gid, gname) = find_room(&groups, room)?.clone();
            let n = members_of(&gname).len();
            if n > 0 {
                return Err(AppError::InvalidInput(format!(
                    "`{gname}` still holds {n} device(s); move them out first"
                )));
            }
            confirm(*force, &format!("Delete empty room \"{gname}\"?"))?;
            let keep: Vec<i64> = groups
                .iter()
                .map(|(g, _)| *g)
                .filter(|g| *g != gid)
                .collect();
            app.manage_rooms(token, &keep, &[gid]).await?;
            let after = app.device_list(token).await?;
            let gone = !after
                .get("groups")
                .and_then(Value::as_array)
                .is_some_and(|a| {
                    a.iter()
                        .any(|g| g.get("groupId").and_then(Value::as_i64) == Some(gid))
                });
            if !gone {
                return Err(AppError::Api {
                    message: "Govee accepted the delete but still lists the room on read-back"
                        .into(),
                    error_code: None,
                });
            }
            Ok(Some(
                json!({"room_id": gid, "name": gname, "deleted": true}),
            ))
        }
        _ => Ok(None),
    }
}
