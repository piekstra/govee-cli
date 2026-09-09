//! Rooms as the Govee Home app has them ("groups"). Reads come from the
//! device list; writes use the app's own room endpoints (see the API notes
//! in the README). Every write is verified by re-reading the device list.

use clap::Subcommand;
use serde_json::{json, Value};

use crate::api::app::{app_devices, AppDevice, Connectivity, GoveeApp};
use crate::cli::auth::load_account;
use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::resolve::pick;

#[derive(Subcommand)]
pub enum RoomsCommand {
    /// The rooms in the Govee Home app with their device counts
    /// (needs `auth login-account`)
    List,
    /// Every device with its room, as `device-rooms/v1` — pipe into
    /// `ghome audit --expect -`. Ids are `<SKU>_<MAC>`, the id Google Home
    /// sees for Govee devices.
    Devices,
    /// Move a device into a room. Asks first unless --force.
    Move {
        /// Device name, `SKU_MAC`, or MAC
        device: String,
        /// Target room name or id
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: i64,
    pub name: String,
}

/// The app's `groups[]` table.
pub fn rooms_of(list: &Value) -> Vec<Room> {
    list.get("groups")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some(Room {
                        id: g.get("groupId")?.as_i64()?,
                        name: g.get("groupName")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The app limits room names to 22 characters.
pub fn validate_room_name(name: &str) -> Result<&str, AppError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 22 {
        return Err(AppError::InvalidInput(
            "room names are 1–22 characters".into(),
        ));
    }
    Ok(n)
}

/// `(device, sku)` pairs the app must be sent as a room's complete
/// membership, keyed by group id.
pub fn members_of(devices: &[AppDevice], group_id: i64) -> Vec<(String, String)> {
    devices
        .iter()
        .filter(|d| d.room_id == Some(group_id))
        .map(|d| (d.device.clone(), d.sku.clone()))
        .collect()
}

/// Membership of `group_id` once `device` joins it (no duplicate if it is
/// already there).
pub fn membership_with(
    devices: &[AppDevice],
    group_id: i64,
    device: &AppDevice,
) -> Vec<(String, String)> {
    let mut m = members_of(devices, group_id);
    if !m.iter().any(|(d, _)| *d == device.device) {
        m.push((device.device.clone(), device.sku.clone()));
    }
    m
}

/// Whether `device` sits in `group_id` in a (re-read) device list.
pub fn placed_in(devices: &[AppDevice], device: &str, group_id: i64) -> bool {
    devices
        .iter()
        .any(|d| d.device == device && d.room_id == Some(group_id))
}

pub fn find_room<'a>(rooms: &'a [Room], q: &str) -> Result<&'a Room, AppError> {
    pick(rooms, q, |r| vec![r.id.to_string()], |r| &r.name, "room")
}

pub fn find_device<'a>(devices: &'a [AppDevice], q: &str) -> Result<&'a AppDevice, AppError> {
    pick(
        devices,
        q,
        |d| vec![format!("{}_{}", d.sku, d.device), d.device.clone()],
        |d| &d.name,
        "device",
    )
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

fn accepted_but(msg: &str) -> AppError {
    AppError::Api {
        message: format!("Govee accepted the write but {msg} on read-back"),
        error_code: None,
    }
}

pub async fn handle(cmd: &RoomsCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    let session = load_account()?;
    if session.token.is_empty() {
        return Err(AppError::NotAuthenticated);
    }
    let token = session.token.as_str();
    let app = GoveeApp::new(session.client_id.clone(), config.verbose)?;
    let list = app.device_list(token).await?;
    let rooms = rooms_of(&list);
    let devices = app_devices(&list);

    match cmd {
        RoomsCommand::List => {
            let items: Vec<Value> = rooms
                .iter()
                .map(|r| {
                    json!({
                        "room_id": r.id,
                        "name": r.name,
                        "devices": devices.iter().filter(|d| d.room_id == Some(r.id)).count(),
                    })
                })
                .collect();
            let without = devices.iter().filter(|d| d.room_id.is_none()).count();
            print_json(&json!({"rooms": items, "devices_without_room": without}));
            Ok(())
        }
        RoomsCommand::Devices => {
            let items: Vec<Value> = devices
                .iter()
                .filter_map(|d| {
                    let room = d.room.clone()?;
                    Some(json!({
                        "id": format!("{}_{}", d.sku, d.device),
                        "name": d.name,
                        "room": room,
                        "source": "govee",
                        "cloud": d.connectivity == Connectivity::Wifi,
                        "connectivity": d.connectivity,
                    }))
                })
                .collect();
            print_json(&json!({"schema": "device-rooms/v1", "items": items}));
            Ok(())
        }
        RoomsCommand::Move {
            device,
            room,
            force,
        } => {
            let d = find_device(&devices, device)?.clone();
            let r = find_room(&rooms, room)?.clone();
            if d.room_id == Some(r.id) {
                print_json(&json!({"device": d.name, "room": r.name, "changed": false}));
                return Ok(());
            }
            confirm(
                *force,
                &format!(
                    "Move \"{}\" from {} to {}?",
                    d.name,
                    d.room.as_deref().unwrap_or("no room"),
                    r.name
                ),
            )?;
            // The prompt may have blocked for a while and `edit_room` sends
            // the room's complete membership, so compute it from a fresh read
            // rather than the pre-prompt snapshot.
            let fresh = app_devices(&app.device_list(token).await?);
            app.edit_room(token, r.id, &r.name, &membership_with(&fresh, r.id, &d))
                .await?;
            let after = app_devices(&app.device_list(token).await?);
            if !placed_in(&after, &d.device, r.id) {
                let now = after
                    .iter()
                    .find(|x| x.device == d.device)
                    .and_then(|x| x.room.clone())
                    .unwrap_or_else(|| "no room".into());
                return Err(accepted_but(&format!("lists the device in {now}")));
            }
            print_json(&json!({
                "device": d.name,
                "id": format!("{}_{}", d.sku, d.device),
                "from_room": d.room,
                "room": r.name,
                "changed": true,
            }));
            Ok(())
        }
        RoomsCommand::Create { name, force } => {
            let name = validate_room_name(name)?;
            if rooms.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
                return Err(AppError::InvalidInput(format!(
                    "a room named `{name}` already exists"
                )));
            }
            confirm(*force, &format!("Create room \"{name}\"?"))?;
            let gid = app.create_room(token, name).await?;
            let after = rooms_of(&app.device_list(token).await?);
            if !after.iter().any(|r| r.id == gid) {
                return Err(accepted_but("does not list the new room"));
            }
            print_json(&json!({"room_id": gid, "name": name, "created": true}));
            Ok(())
        }
        RoomsCommand::Rename { room, name, force } => {
            let name = validate_room_name(name)?;
            let r = find_room(&rooms, room)?.clone();
            confirm(*force, &format!("Rename \"{}\" to \"{name}\"?", r.name))?;
            // Same full-membership write as a move: read fresh after the prompt.
            let fresh = app_devices(&app.device_list(token).await?);
            app.edit_room(token, r.id, name, &members_of(&fresh, r.id))
                .await?;
            let after = rooms_of(&app.device_list(token).await?);
            if !after.iter().any(|x| x.id == r.id && x.name == name) {
                return Err(accepted_but("still shows the old name"));
            }
            print_json(
                &json!({"room_id": r.id, "previous_name": r.name, "name": name, "changed": true}),
            );
            Ok(())
        }
        RoomsCommand::Delete { room, force } => {
            let r = find_room(&rooms, room)?.clone();
            let n = members_of(&devices, r.id).len();
            if n > 0 {
                return Err(AppError::InvalidInput(format!(
                    "`{}` still holds {n} device(s); move them out first",
                    r.name
                )));
            }
            confirm(*force, &format!("Delete empty room \"{}\"?", r.name))?;
            let keep: Vec<i64> = rooms
                .iter()
                .map(|x| x.id)
                .filter(|id| *id != r.id)
                .collect();
            app.manage_rooms(token, &keep, &[r.id]).await?;
            let after = rooms_of(&app.device_list(token).await?);
            if after.iter().any(|x| x.id == r.id) {
                return Err(accepted_but("still lists the room"));
            }
            print_json(&json!({"room_id": r.id, "name": r.name, "deleted": true}));
            Ok(())
        }
    }
}
