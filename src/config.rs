//! Non-secret settings (`~/.config/govee/config.json`, or `--config` /
//! `$GOVEE_CONFIG`). Credentials never live here — they are keychain-only
//! (`piekstra.govee`). What does live here is the knowledge *that* a
//! credential is stored, so a command can stop with exit 3 before touching
//! the keychain when nothing was ever configured (SPEC §1.5): a keychain read
//! from a fresh binary is a macOS permission prompt, and a driver must never
//! hang on one.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Govee Home account email, set by `auth login-account`. Its presence
    /// gates the account-session keychain read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Set by `auth login` / `auth set-credential` once the Platform API key
    /// is in the keychain; cleared by `auth logout --forget`. Gates the
    /// API-key keychain read. Not user-settable through `config set`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub api_key_in_keychain: bool,
}

/// Keys `config set|unset` accept.
pub const KEYS: &[&str] = &["username"];

impl Config {
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "username" => self.username = Some(value.to_string()),
            other => return Err(unknown(other)),
        }
        Ok(())
    }

    pub fn unset(&mut self, key: &str) -> Result<(), String> {
        match key {
            "username" => self.username = None,
            other => return Err(unknown(other)),
        }
        Ok(())
    }
}

fn unknown(key: &str) -> String {
    format!("unknown config key `{key}` (known: {})", KEYS.join(", "))
}
