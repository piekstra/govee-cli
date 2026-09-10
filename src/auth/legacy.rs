//! One-time move of the credentials govee-cli 0.1 stored under the
//! unprefixed keychain service `govee-cli` (items `api_key` and `account`)
//! to the family service `piekstra.govee`. Read old → write new → delete
//! old, so the next run finds nothing to move.
//!
//! Runs only from `auth login` / `auth login-account` (explicit, interactive
//! auth commands): reads are the prompting operation on macOS, and the
//! config-gated read path never probes a service it has no record of. Only
//! the items the config has no record of are moved — a credential the user
//! already stored by hand in the new layout is never overwritten, and its
//! legacy twin is left untouched rather than deleted unread.

use pk_cli_core::CliError;
use pk_cli_secrets::{CredentialStore, Secret};

use super::account::{self, AccountSession};
use super::{ACCOUNT_ITEM, API_KEY_ITEM};

pub const LEGACY_SERVICE: &str = "govee-cli";

/// Which legacy items to move: those the config does not yet account for.
#[derive(Debug, Clone, Copy)]
pub struct Wanted {
    pub api_key: bool,
    pub account: bool,
}

/// What `migrate` moved. The key comes back so the caller can verify it
/// without a second keychain read.
#[derive(Default)]
pub struct Migrated {
    pub api_key: Option<Secret>,
    pub account: Option<AccountSession>,
}

impl Migrated {
    pub fn any(&self) -> bool {
        self.api_key.is_some() || self.account.is_some()
    }
}

pub fn migrate(creds: &CredentialStore, wanted: Wanted) -> Result<Migrated, CliError> {
    migrate_from(&CredentialStore::new(LEGACY_SERVICE), creds, wanted)
}

/// The mechanism behind [`migrate`], over an arbitrary source store.
pub fn migrate_from(
    legacy: &CredentialStore,
    creds: &CredentialStore,
    wanted: Wanted,
) -> Result<Migrated, CliError> {
    let mut moved = Migrated::default();
    if wanted.api_key {
        if let Some(key) = legacy.get(API_KEY_ITEM)? {
            creds.set(API_KEY_ITEM, &key)?;
            legacy.delete(API_KEY_ITEM)?;
            moved.api_key = Some(key);
        }
    }
    if wanted.account {
        if let Some(blob) = legacy.get(ACCOUNT_ITEM)? {
            if let Some(session) = account::parse(&blob) {
                creds.set(ACCOUNT_ITEM, &blob)?;
                moved.account = Some(session);
            }
            // Unparsable blobs are dropped rather than carried forward.
            legacy.delete(ACCOUNT_ITEM)?;
        }
    }
    Ok(moved)
}
