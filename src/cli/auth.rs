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
    /// Sign in to the Govee account itself (email + password + emailed
    /// code), for the room commands the public API can't serve
    LoginAccount {
        /// The verification code Govee emailed, when resuming a login
        /// that asked for one
        #[arg(long)]
        code: Option<String>,
        /// Read the password from stdin (`op read … | govee auth
        /// login-account --stdin --email you@example.com`)
        #[arg(long)]
        stdin: bool,
        /// Account email (with --stdin; otherwise prompted)
        #[arg(long)]
        email: Option<String>,
    },
    /// Clear the stored account session (the API key stays)
    LogoutAccount,
}

pub async fn handle(cmd: &AuthCommand, config: &RuntimeConfig) -> Result<(), AppError> {
    match cmd {
        AuthCommand::Login => handle_login(config).await,
        AuthCommand::Logout => handle_logout(),
        AuthCommand::Status => handle_status(config).await,
        AuthCommand::LoginAccount { code, stdin, email } => {
            handle_login_account(code.as_deref(), *stdin, email.as_deref(), config).await
        }
        AuthCommand::LogoutAccount => {
            keychain::clear_account()?;
            print_json(&json!({ "status": "account_logged_out" }));
            Ok(())
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AccountSession {
    pub token: String,
    pub account_id: String,
    pub client_id: String,
    pub email: String,
}

pub fn load_account() -> Result<AccountSession, AppError> {
    let blob = keychain::get_account()?.ok_or(AppError::NotAuthenticated)?;
    serde_json::from_str(&blob).map_err(|_| AppError::NotAuthenticated)
}

async fn handle_login_account(
    code: Option<&str>,
    stdin: bool,
    email_flag: Option<&str>,
    config: &RuntimeConfig,
) -> Result<(), AppError> {
    use crate::api::app::{GoveeApp, LoginOutcome};
    use std::io::{IsTerminal, Read};
    let interactive = std::io::stdin().is_terminal() && !stdin;
    // Keep the client id stable across attempts so the emailed code matches.
    let (client_id, remembered_email) = match load_account() {
        Ok(a) => (a.client_id, Some(a.email)),
        Err(_) => (GoveeApp::new_client_id(), None),
    };
    let email = match email_flag.map(str::to_string).or_else(|| std::env::var("GOVEE_EMAIL").ok().filter(|e| !e.is_empty())) {
        Some(e) => e,
        None if interactive => {
            let mut p = dialoguer::Input::<String>::new().with_prompt("Govee account email");
            if let Some(e) = remembered_email {
                p = p.default(e);
            }
            p.interact_text()
                .map_err(|e| AppError::InvalidInput(e.to_string()))?
        }
        None => remembered_email.ok_or_else(|| {
            AppError::InvalidInput("not a terminal: pass --email and the password on --stdin".into())
        })?,
    };
    let password = if stdin {
        let mut pw = String::new();
        std::io::stdin()
            .read_to_string(&mut pw)
            .map_err(|e| AppError::InvalidInput(format!("reading password from stdin: {e}")))?;
        let pw = pw.trim_end_matches(['\r', '\n']).to_string();
        if pw.is_empty() {
            return Err(AppError::InvalidInput("no password on stdin".into()));
        }
        pw
    } else {
        match std::env::var("GOVEE_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ if interactive => dialoguer::Password::new()
                .with_prompt("Govee account password")
                .interact()
                .map_err(|e| AppError::InvalidInput(e.to_string()))?,
            _ => {
                return Err(AppError::InvalidInput(
                    "not a terminal: pass the password on --stdin".into(),
                ))
            }
        }
    };
    let app = GoveeApp::new(client_id.clone(), config.verbose)?;
    // Remember the identity before any network call so a code-resume finds it.
    keychain::store_account(&serde_json::to_string(&AccountSession {
        token: String::new(),
        account_id: String::new(),
        client_id: client_id.clone(),
        email: email.clone(),
    })?)?;

    let mut code = code.map(str::to_string);
    loop {
        match app.login(&email, &password, code.as_deref()).await? {
            LoginOutcome::Token { token, account_id } => {
                keychain::store_account(&serde_json::to_string(&AccountSession {
                    token,
                    account_id: account_id.clone(),
                    client_id,
                    email: email.clone(),
                })?)?;
                print_json(&json!({"status": "account_authenticated", "email": email, "account_id": account_id}));
                return Ok(());
            }
            LoginOutcome::NeedsCode => {
                if code.is_some() {
                    return Err(AppError::Api {
                        message: "Govee still wants a verification code; request a fresh one by running without --code".into(),
                        error_code: Some(454),
                    });
                }
                app.request_code(&email).await?;
                eprintln!("Govee emailed a verification code to {email}.");
                if !interactive {
                    print_json(&json!({"status": "code_sent", "email": email, "next": "re-run `govee auth login-account --stdin --code <CODE>`"}));
                    return Ok(());
                }
                let c = dialoguer::Input::<String>::new()
                    .with_prompt("Verification code")
                    .interact_text()
                    .map_err(|e| AppError::InvalidInput(e.to_string()))?;
                code = Some(c.trim().to_string());
            }
        }
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
