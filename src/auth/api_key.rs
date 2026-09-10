//! The Platform API key: environment first, then the keychain (gated).

use pk_cli_core::CliError;
use pk_cli_secrets::{CredentialStore, Secret};

use crate::config::Config;

pub const ENV_VAR: &str = "GOVEE_API_KEY";

/// Where a usable key came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Keychain,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Env => "env",
            Source::Keychain => "keychain",
        }
    }
}

/// `$GOVEE_API_KEY`, when set and non-empty.
pub fn from_env() -> Option<Secret> {
    std::env::var(ENV_VAR)
        .ok()
        .filter(|k| !k.trim().is_empty())
        .map(|k| Secret::new(k.trim().to_string()))
}

/// The key to use: `$GOVEE_API_KEY`, else the keychain — read only when the
/// config says a key was stored, so a fresh machine never prompts.
pub fn resolve(cfg: &Config, creds: &CredentialStore) -> Result<(Secret, Source), CliError> {
    if let Some(k) = from_env() {
        return Ok((k, Source::Env));
    }
    if !cfg.api_key_in_keychain {
        return Err(CliError::Auth(format!(
            "no Govee API key configured; run `govee auth login` (or set ${ENV_VAR})"
        )));
    }
    match creds.get(super::API_KEY_ITEM)? {
        Some(k) => Ok((k, Source::Keychain)),
        None => Err(CliError::Auth(format!(
            "the config says an API key is stored but the keychain ({}) has none; run `govee auth login --overwrite`",
            creds.service()
        ))),
    }
}

/// For `auth status`: `(source of a usable key, key present in the keychain)`.
/// The keychain is consulted only when the config says a key was stored.
pub fn status(cfg: &Config, creds: &CredentialStore) -> Result<(Option<Source>, bool), CliError> {
    let in_keychain = cfg.api_key_in_keychain && creds.get(super::API_KEY_ITEM)?.is_some();
    let source = if from_env().is_some() {
        Some(Source::Env)
    } else if in_keychain {
        Some(Source::Keychain)
    } else {
        None
    };
    Ok((source, in_keychain))
}
