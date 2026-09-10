//! One-time move of the credentials govee-cli 0.1 stored under the
//! unprefixed keychain service `govee-cli` (items `api_key` and `account`)
//! to the family service `piekstra.govee`, via
//! `CredentialStore::migrate_from` (SPEC §1.7): per item, read old → write
//! new → delete old; an item already stored under the new service wins and
//! the legacy copy is still retired, so repeated runs converge.
//!
//! Runs only from `auth login` / `auth login-account` (explicit auth
//! commands): reads are the prompting operation on macOS, and the
//! config-gated read path never probes a service it has no record of.

use pk_cli_core::CliError;
use pk_cli_secrets::CredentialStore;

use super::{ACCOUNT_ITEM, API_KEY_ITEM};

pub const LEGACY_SERVICE: &str = "govee-cli";

/// Which items were copied into the new service.
#[derive(Debug, Default, Clone, Copy)]
pub struct Moved {
    pub api_key: bool,
    pub account: bool,
}

impl Moved {
    pub fn any(&self) -> bool {
        self.api_key || self.account
    }
}

/// One `migrate_from` call per item, so the caller learns which one moved:
/// the config records the two credentials separately.
pub fn migrate(creds: &CredentialStore) -> Result<Moved, CliError> {
    let legacy = CredentialStore::new(LEGACY_SERVICE);
    Ok(Moved {
        api_key: creds.migrate_from(&legacy, &[(API_KEY_ITEM, API_KEY_ITEM)])? > 0,
        account: creds.migrate_from(&legacy, &[(ACCOUNT_ITEM, ACCOUNT_ITEM)])? > 0,
    })
}
