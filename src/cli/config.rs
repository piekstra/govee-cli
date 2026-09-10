//! `govee config path|show|set|unset` — offline; never touches the keychain.

use clap::Subcommand;
use pk_cli_config::ConfigStore;
use pk_cli_core::{output, CliError};

use crate::config::Config;

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Print the resolved config file path.
    Path,
    /// Show the effective configuration.
    Show,
    /// Set a config key (`username`).
    Set { key: String, value: String },
    /// Remove a config key.
    Unset { key: String },
}

pub fn run(json: bool, cmd: &ConfigCommand, store: &ConfigStore) -> Result<(), CliError> {
    match cmd {
        ConfigCommand::Path => {
            println!("{}", store.path()?.display());
            Ok(())
        }
        ConfigCommand::Show => {
            let cfg: Config = store.load()?;
            let v = serde_json::to_value(&cfg).unwrap_or_default();
            if json {
                output::json(&v);
            } else {
                output::render(&v);
            }
            Ok(())
        }
        ConfigCommand::Set { key, value } => {
            let mut cfg: Config = store.load()?;
            cfg.set(key, value).map_err(CliError::Usage)?;
            store.save(&cfg)
        }
        ConfigCommand::Unset { key } => {
            let mut cfg: Config = store.load()?;
            cfg.unset(key).map_err(CliError::Usage)?;
            store.save(&cfg)
        }
    }
}
