//! The Govee Home app session: one keychain item holding the bearer token
//! and the identity it was minted for. The `client_id` must stay stable
//! across login attempts (the emailed verification code is bound to it), so
//! it is persisted before the first network call.

use pk_cli_core::CliError;
use pk_cli_secrets::{CredentialStore, Secret};
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSession {
    pub token: String,
    pub account_id: String,
    pub client_id: String,
    pub email: String,
}

impl AccountSession {
    pub fn signed_in(&self) -> bool {
        !self.token.is_empty()
    }
}

/// Parse the stored blob; anything unparsable counts as "no session".
pub fn parse(raw: &Secret) -> Option<AccountSession> {
    serde_json::from_str(raw.expose()).ok()
}

/// The stored session, read only when the config names an account (the
/// gate that keeps a fresh machine prompt-free). `None` when nothing usable
/// is stored.
pub fn load(cfg: &Config, creds: &CredentialStore) -> Result<Option<AccountSession>, CliError> {
    if cfg.username.is_none() {
        return Ok(None);
    }
    Ok(creds.get(super::ACCOUNT_ITEM)?.as_ref().and_then(parse))
}

/// The session a room command needs, or exit 3 pointing at `auth login-account`.
pub fn require(cfg: &Config, creds: &CredentialStore) -> Result<AccountSession, CliError> {
    match load(cfg, creds)? {
        Some(s) if s.signed_in() => Ok(s),
        _ => Err(CliError::Auth(
            "no Govee account session; run `govee auth login-account`".into(),
        )),
    }
}

pub fn store(creds: &CredentialStore, session: &AccountSession) -> Result<(), CliError> {
    let blob = serde_json::to_string(session)
        .map_err(|e| CliError::Other(format!("serializing account session: {e}")))?;
    creds.set(super::ACCOUNT_ITEM, &Secret::new(blob))
}

/// Remove the session item. Returns whether one existed.
pub fn clear(creds: &CredentialStore) -> Result<bool, CliError> {
    creds.delete(super::ACCOUNT_ITEM)
}
