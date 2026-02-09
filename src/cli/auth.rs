use clap::Subcommand;
use serde_json::json;

use crate::api::client::GoveeApi;
use crate::auth::{api_key, keychain};
use crate::cli::output::print_json;
use crate::config::RuntimeConfig;
use crate::error::AppError;

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Store a Govee API key for authentication
    Login,
    /// Clear stored API key
    Logout,
    /// Show authentication status
    Status,
}

pub async fn handle(cmd: &AuthCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        AuthCommand::Login => handle_login(config).await,
        AuthCommand::Logout => handle_logout(),
        AuthCommand::Status => handle_status(config).await,
    }
}

async fn handle_login(config: &RuntimeConfig) -> Result<(), AppError> {
    // Check if already provided via env var
    let key = match std::env::var("GOVEE_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => dialoguer::Password::new()
            .with_prompt("Govee API Key")
            .interact()
            .map_err(|e| AppError::InvalidInput(e.to_string()))?,
    };

    // Validate by fetching devices
    let api = GoveeApi::new(key.clone(), config.verbose)?;
    let data = api.get_devices().await?;
    let device_count = data.as_array().map(|a| a.len()).unwrap_or(0);

    keychain::store_api_key(&key)?;

    print_json(&json!({
        "status": "authenticated",
        "devices_found": device_count,
    }));
    Ok(())
}

fn handle_logout() -> Result<(), AppError> {
    keychain::clear_api_key()?;
    print_json(&json!({ "status": "logged_out" }));
    Ok(())
}

async fn handle_status(config: &RuntimeConfig) -> Result<(), AppError> {
    match api_key::get_api_key() {
        Ok(key) => {
            let api = GoveeApi::new(key, config.verbose)?;
            match api.get_devices().await {
                Ok(data) => {
                    let device_count = data.as_array().map(|a| a.len()).unwrap_or(0);
                    print_json(&json!({
                        "authenticated": true,
                        "devices_found": device_count,
                    }));
                }
                Err(_) => {
                    print_json(&json!({
                        "authenticated": true,
                        "api_reachable": false,
                    }));
                }
            }
        }
        Err(_) => {
            print_json(&json!({ "authenticated": false }));
        }
    }
    Ok(())
}
