use keyring::Entry;

use crate::error::AppError;

const SERVICE: &str = "govee-cli";
const KEY_NAME: &str = "api_key";

fn entry() -> Result<Entry, AppError> {
    Entry::new(SERVICE, KEY_NAME).map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn store_api_key(key: &str) -> Result<(), AppError> {
    entry()?
        .set_password(key)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get_api_key() -> Result<Option<String>, AppError> {
    match entry()?.get_password() {
        Ok(val) => Ok(Some(val)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn clear_api_key() -> Result<(), AppError> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}
