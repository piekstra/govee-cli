//! Toggles: the on/off features a device exposes as
//! `devices.capabilities.toggle` (gradient, DreamView, and on lamps with
//! several parts the ripple / side / bottom lights). `set` takes any of
//! them by name; `gradient` and `dreamview` stay as shorthands.

use clap::Subcommand;
use pk_cli_core::CliError;
use serde_json::json;

use pk_cli_core::output::emit_one;

use super::output::emit_list_with;
use super::Ctx;
use crate::error::AppError;
use crate::resolve;

const TOGGLE: &str = "devices.capabilities.toggle";

#[derive(Subcommand, Debug)]
pub enum ToggleCommand {
    /// Switch one of the device's toggles (toggle/v1): by instance name
    /// (`rippleLightToggle`) or any unique part of it (`ripple`, `side`,
    /// `bottom`, `dreamview`). `toggle list` shows what a device has.
    Set {
        /// Device name or ID
        device: String,
        /// Toggle: instance name or a unique part of it
        toggle: String,
        /// "on" or "off"
        state: String,
    },
    /// Gradient mode on or off (shorthand for `set <device> gradient`).
    Gradient {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// DreamView mode on or off (shorthand for `set <device> dreamview`).
    Dreamview {
        /// Device name or ID
        device: String,
        /// "on" or "off"
        state: String,
    },
    /// The toggles a device offers (toggle-list/v1).
    #[command(visible_alias = "ls")]
    List {
        /// Device name or ID
        device: String,
    },
}

pub fn parse_on_off(state: &str) -> Result<bool, AppError> {
    match state.to_lowercase().as_str() {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(AppError::InvalidInput(format!(
            "invalid state `{state}`: use `on` or `off`"
        ))),
    }
}

/// Which of `instances` the user means: the exact instance, else a
/// case-insensitive match on the instance with its `Toggle` suffix
/// dropped (`ripple` → `rippleLightToggle`, `dreamview` →
/// `dreamViewToggle`), else the one instance containing the query.
/// Ambiguity and no match are usage errors naming the choices.
pub fn pick_toggle<'a>(instances: &[&'a str], query: &str) -> Result<&'a str, AppError> {
    if let Some(i) = instances.iter().find(|i| **i == query) {
        return Ok(i);
    }
    let q: String = query
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let stem = |i: &str| -> String {
        let lower = i.to_lowercase();
        let base = lower.strip_suffix("toggle").unwrap_or(&lower);
        base.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
    };
    if !q.is_empty() {
        let exact: Vec<&&str> = instances
            .iter()
            .filter(|i| stem(i) == q || stem(i).strip_suffix("light") == Some(q.as_str()))
            .collect();
        if exact.len() == 1 {
            return Ok(exact[0]);
        }
        let partial: Vec<&&str> = instances
            .iter()
            .filter(|i| i.to_lowercase().contains(&q))
            .collect();
        if partial.len() == 1 {
            return Ok(partial[0]);
        }
        if partial.len() > 1 {
            return Err(AppError::InvalidInput(format!(
                "`{query}` matches more than one toggle: {}",
                partial
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    Err(AppError::InvalidInput(format!(
        "no toggle matching `{query}`; this device has: {}",
        if instances.is_empty() {
            "none".to_string()
        } else {
            instances.join(", ")
        }
    )))
}

/// Argument checks that need no credential (exit 2 before the keychain).
pub fn validate(cmd: &ToggleCommand) -> Result<(), CliError> {
    match cmd {
        ToggleCommand::Set { state, toggle, .. } => {
            parse_on_off(state)?;
            if toggle.trim().is_empty() {
                return Err(AppError::InvalidInput("a toggle name is required".into()).into());
            }
        }
        ToggleCommand::Gradient { state, .. } | ToggleCommand::Dreamview { state, .. } => {
            parse_on_off(state)?;
        }
        ToggleCommand::List { .. } => {}
    }
    Ok(())
}

pub async fn handle(ctx: &Ctx, cmd: &ToggleCommand) -> Result<(), CliError> {
    validate(cmd)?;
    let api = ctx.api()?;
    let (device, query, state) = match cmd {
        ToggleCommand::Set {
            device,
            toggle,
            state,
        } => (device, toggle.as_str(), state),
        ToggleCommand::Gradient { device, state } => (device, "gradientToggle", state),
        ToggleCommand::Dreamview { device, state } => (device, "dreamViewToggle", state),
        ToggleCommand::List { device } => {
            let dev = resolve::resolve_device(&api, device).await?;
            let items: Vec<serde_json::Value> = dev
                .info
                .capabilities
                .iter()
                .filter(|c| c.capability_type == TOGGLE)
                .map(|c| json!({ "toggle": c.instance }))
                .collect();
            emit_list_with(
                ctx.json,
                "toggle",
                &[("device", json!(dev.name()))],
                items,
                &["toggle"],
            );
            return Ok(());
        }
    };
    let on = parse_on_off(state)?;
    let dev = resolve::resolve_device(&api, device).await?;
    let instances: Vec<&str> = dev
        .info
        .capabilities
        .iter()
        .filter(|c| c.capability_type == TOGGLE)
        .map(|c| c.instance.as_str())
        .collect();
    let instance = pick_toggle(&instances, query)?.to_string();
    dev.set_toggle(&instance, on).await?;
    emit_one(
        ctx.json,
        "toggle",
        json!({ "device": dev.name(), "toggle": instance, "state": on_off(on) }),
    );
    Ok(())
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAMP: &[&str] = &[
        "dreamViewToggle",
        "rippleLightToggle",
        "sideLightToggle",
        "bottomLightToggle",
    ];

    #[test]
    fn toggles_resolve_by_instance_stem_or_unique_part() {
        assert_eq!(
            pick_toggle(LAMP, "rippleLightToggle").unwrap(),
            "rippleLightToggle"
        );
        assert_eq!(pick_toggle(LAMP, "ripple").unwrap(), "rippleLightToggle");
        assert_eq!(
            pick_toggle(LAMP, "Ripple light").unwrap(),
            "rippleLightToggle"
        );
        assert_eq!(pick_toggle(LAMP, "side").unwrap(), "sideLightToggle");
        assert_eq!(pick_toggle(LAMP, "bottom").unwrap(), "bottomLightToggle");
        assert_eq!(pick_toggle(LAMP, "dreamview").unwrap(), "dreamViewToggle");
        assert_eq!(pick_toggle(LAMP, "dream").unwrap(), "dreamViewToggle");
        let e = pick_toggle(LAMP, "light").unwrap_err().to_string();
        assert!(e.contains("more than one"), "{e}");
        let e = pick_toggle(LAMP, "gradient").unwrap_err().to_string();
        assert!(e.contains("this device has: dreamViewToggle"), "{e}");
        assert!(pick_toggle(&[], "ripple")
            .unwrap_err()
            .to_string()
            .contains("none"));
    }

    #[test]
    fn states_parse() {
        assert!(parse_on_off("ON").unwrap());
        assert!(!parse_on_off("0").unwrap());
        assert!(parse_on_off("maybe").is_err());
    }
}
