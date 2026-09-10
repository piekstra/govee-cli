//! Credential state. Two secrets, both keychain-only under `piekstra.govee`:
//!
//! - `api_key` — the Platform API key (`auth login`). `$GOVEE_API_KEY` wins
//!   over the keychain when set, for `op run`-style invocations.
//! - `account` — the Govee Home app session (`auth login-account`), a JSON
//!   blob `{token, account_id, client_id, email}`.
//!
//! Reads are gated by the config file (see `crate::config`) so a machine with
//! nothing configured exits 3 without a keychain prompt. 0.1 kept the same two
//! items under the unprefixed service `govee-cli`; `legacy` moves them.

pub mod account;
pub mod api_key;
pub mod legacy;

/// Keychain item holding the Platform API key.
pub const API_KEY_ITEM: &str = "api_key";
/// Keychain item holding the app account session blob.
pub const ACCOUNT_ITEM: &str = "account";
