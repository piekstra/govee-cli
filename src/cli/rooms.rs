//! Rooms as the Govee Home app has them ("groups"). Reads come from the
//! device list; writes use the app's own room endpoints (docs/api.md). Every
//! write confirms first (exit 6 non-interactively without `--force`, before
//! any credential is read) and is verified by re-reading the device list.

use clap::Subcommand;
use pk_cli_core::confirm::{confirm, require_confirmable};
use pk_cli_core::output::{self, emit_one};
use pk_cli_core::resolve::pick;
use pk_cli_core::CliError;
use serde_json::{json, Value};

use super::output::emit_list_with;
use super::Ctx;
use crate::api::app::{app_devices, AppDevice, Connectivity, GoveeApp};
use crate::error::AppError;

#[derive(Subcommand, Debug)]
pub enum RoomsCommand {
    /// The rooms in the Govee Home app with their device counts (room-list/v1).
    #[command(visible_alias = "ls")]
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
        /// Skip the confirmation prompt (required when non-interactive)
        #[arg(long)]
        force: bool,
    },
    /// Rename a room. Asks first unless --force.
    Rename {
        room: String,
        name: String,
        /// Skip the confirmation prompt (required when non-interactive)
        #[arg(long)]
        force: bool,
    },
    /// Delete an empty room. Asks first unless --force.
    Delete {
        room: String,
        /// Skip the confirmation prompt (required when non-interactive)
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

pub fn find_room<'a>(rooms: &'a [Room], q: &str) -> Result<&'a Room, CliError> {
    pick(rooms, q, |r| vec![r.id.to_string()], |r| &r.name, "room")
}

pub fn find_device<'a>(devices: &'a [AppDevice], q: &str) -> Result<&'a AppDevice, CliError> {
    pick(
        devices,
        q,
        |d| vec![format!("{}_{}", d.sku, d.device), d.device.clone()],
        |d| &d.name,
        "device",
    )
}

/// One `device-rooms/v1` row: `id` is `<SKU>_<MAC>` (what Google Home sees),
/// `name` is omitted — never null — when the app reports none, and `cloud`
/// is false for the Bluetooth-only devices an assistant can never see.
/// Devices in no room have no row.
pub fn device_room_row(d: &AppDevice) -> Option<Value> {
    let room = d.room.clone()?;
    let mut row = json!({ "id": format!("{}_{}", d.sku, d.device) });
    if !d.name.trim().is_empty() {
        row["name"] = json!(d.name);
    }
    row["room"] = json!(room);
    row["source"] = json!("govee");
    row["cloud"] = json!(d.connectivity == Connectivity::Wifi);
    row["connectivity"] = json!(d.connectivity);
    Some(row)
}

fn accepted_but(msg: &str) -> CliError {
    CliError::Upstream(format!("Govee accepted the write but {msg} on read-back"))
}

/// Argument and confirmation gates that need no credential: bad input is
/// exit 2, a non-interactive write without `--force` is exit 6 — both
/// before the keychain or the network (`pk_cli_core::confirm`'s ordering
/// rule).
pub fn validate(cmd: &RoomsCommand, interactive: bool) -> Result<(), CliError> {
    match cmd {
        RoomsCommand::List | RoomsCommand::Devices => Ok(()),
        RoomsCommand::Move { force, .. } => {
            require_confirmable(*force, interactive, "moving a device between rooms")
        }
        RoomsCommand::Create { name, force } => {
            validate_room_name(name)?;
            require_confirmable(*force, interactive, "creating a room")
        }
        RoomsCommand::Rename { name, force, .. } => {
            validate_room_name(name)?;
            require_confirmable(*force, interactive, "renaming a room")
        }
        RoomsCommand::Delete { force, .. } => {
            require_confirmable(*force, interactive, "deleting a room")
        }
    }
}

struct Session {
    app: GoveeApp,
    token: String,
}

impl Session {
    async fn list(&self) -> Result<Value, CliError> {
        Ok(self.app.device_list(&self.token).await?)
    }
}

pub async fn handle(ctx: &Ctx, cmd: &RoomsCommand) -> Result<(), CliError> {
    validate(cmd, ctx.interactive)?;
    let account = ctx.require_account()?;
    let s = Session {
        app: GoveeApp::new(account.client_id.clone(), ctx.verbose)?,
        token: account.token.clone(),
    };
    let list = s.list().await?;
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
            emit_list_with(
                ctx.json,
                "room",
                &[("devices_without_room", json!(without))],
                items,
                &["room_id", "name", "devices"],
            );
            Ok(())
        }
        RoomsCommand::Devices => {
            let items: Vec<Value> = devices.iter().filter_map(device_room_row).collect();
            // The smart-home/v1 profile's shape (SPEC §1.8), so the name is
            // `device-rooms/v1` rather than a `<record>-list`.
            output::emit(ctx.json, "device-rooms", json!({ "items": items }), |v| {
                output::table(&output::table_view(
                    &output::rows_of(v, "items"),
                    &["id", "name", "room", "connectivity"],
                ));
            });
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
                emit_one(
                    ctx.json,
                    "room-move",
                    json!({ "device": d.name, "room": r.name, "changed": false }),
                );
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
            let fresh = app_devices(&s.list().await?);
            s.app
                .edit_room(&s.token, r.id, &r.name, &membership_with(&fresh, r.id, &d))
                .await?;
            let after = app_devices(&s.list().await?);
            if !placed_in(&after, &d.device, r.id) {
                let now = after
                    .iter()
                    .find(|x| x.device == d.device)
                    .and_then(|x| x.room.clone())
                    .unwrap_or_else(|| "no room".into());
                return Err(accepted_but(&format!("lists the device in {now}")));
            }
            emit_one(
                ctx.json,
                "room-move",
                json!({
                    "device": d.name,
                    "id": format!("{}_{}", d.sku, d.device),
                    "from_room": d.room,
                    "room": r.name,
                    "changed": true,
                }),
            );
            Ok(())
        }
        RoomsCommand::Create { name, force } => {
            let name = validate_room_name(name)?;
            if rooms.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
                return Err(CliError::Usage(format!(
                    "a room named `{name}` already exists"
                )));
            }
            confirm(*force, &format!("Create room \"{name}\"?"))?;
            let gid = s.app.create_room(&s.token, name).await?;
            let after = rooms_of(&s.list().await?);
            if !after.iter().any(|r| r.id == gid) {
                return Err(accepted_but("does not list the new room"));
            }
            emit_one(
                ctx.json,
                "room-create",
                json!({ "room_id": gid, "name": name, "created": true }),
            );
            Ok(())
        }
        RoomsCommand::Rename { room, name, force } => {
            let name = validate_room_name(name)?;
            let r = find_room(&rooms, room)?.clone();
            confirm(*force, &format!("Rename \"{}\" to \"{name}\"?", r.name))?;
            // Same full-membership write as a move: read fresh after the prompt.
            let fresh = app_devices(&s.list().await?);
            s.app
                .edit_room(&s.token, r.id, name, &members_of(&fresh, r.id))
                .await?;
            let after = rooms_of(&s.list().await?);
            if !after.iter().any(|x| x.id == r.id && x.name == name) {
                return Err(accepted_but("still shows the old name"));
            }
            emit_one(
                ctx.json,
                "room-rename",
                json!({ "room_id": r.id, "previous_name": r.name, "name": name, "changed": true }),
            );
            Ok(())
        }
        RoomsCommand::Delete { room, force } => {
            let r = find_room(&rooms, room)?.clone();
            let n = members_of(&devices, r.id).len();
            if n > 0 {
                return Err(CliError::Usage(format!(
                    "`{}` still holds {n} device(s); move them out first",
                    r.name
                )));
            }
            confirm(*force, &format!("Delete empty room \"{}\"?", r.name))?;
            // `manage_rooms` sends the complete remaining room list, so read
            // it fresh after the prompt like the other whole-collection writes.
            let fresh_list = s.list().await?;
            let fresh_rooms = rooms_of(&fresh_list);
            let fresh_devices = app_devices(&fresh_list);
            if !members_of(&fresh_devices, r.id).is_empty() {
                return Err(CliError::Usage(format!(
                    "`{}` gained devices while waiting; move them out first",
                    r.name
                )));
            }
            let keep: Vec<i64> = fresh_rooms
                .iter()
                .map(|x| x.id)
                .filter(|id| *id != r.id)
                .collect();
            s.app.manage_rooms(&s.token, &keep, &[r.id]).await?;
            let after = rooms_of(&s.list().await?);
            if after.iter().any(|x| x.id == r.id) {
                return Err(accepted_but("still lists the room"));
            }
            emit_one(
                ctx.json,
                "room-delete",
                json!({ "room_id": r.id, "name": r.name, "deleted": true }),
            );
            Ok(())
        }
    }
}
