use std::env;

use crate::auth::keychain;
use crate::error::AppError;

/// Resolve the API key from:
/// 1. GOVEE_API_KEY environment variable (highest priority)
/// 2. OS keychain (stored via `govee login`)
pub fn get_api_key() -> Result<String, AppError> {
    if let Ok(key) = env::var("GOVEE_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }
    keychain::get_api_key()?.ok_or(AppError::NotAuthenticated)
}
