//! `govee auth …` — the Platform API key (`login`/`logout`/`status`/
//! `set-credential`, the SPEC v1 surface) and the Govee Home account session
//! (`login-account`/`logout-account`) the room commands need.

use clap::Subcommand;
use pk_cli_auth::{AuthMethod, AuthStatus, LoginArgs, LogoutArgs, SetCredentialArgs};
use pk_cli_core::{output, CliError};
use pk_cli_secrets::{read_stdin, Secret};
use serde_json::json;

use super::output::emit_one;
use super::{prompt_line, Ctx};
use crate::api::app::{GoveeApp, LoginOutcome};
use crate::api::client::GoveeApi;
use crate::auth::account::{self, AccountSession};
use crate::auth::{api_key, legacy, API_KEY_ITEM};

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Store the Govee Platform API key in the OS keychain (verified live
    /// unless --no-verify). Get one in the Govee Home app: Profile >
    /// Settings > Apply for API Key.
    Login(LoginArgs),
    /// Report credential/session state (auth-status/v1).
    Status,
    /// Clear the account session; --forget also removes the API key and
    /// the config file.
    Logout(LogoutArgs),
    /// Raw keychain write of the API key for rotation / headless setup.
    SetCredential(SetCredentialArgs),
    /// Sign in to the Govee Home account itself (email + password + emailed
    /// code), for the room commands the Platform API can't serve.
    LoginAccount {
        /// The verification code Govee emailed, when resuming a login
        /// that asked for one
        #[arg(long)]
        code: Option<String>,
        /// Read the password from stdin (`op read … | govee auth
        /// login-account --stdin --email you@example.com`)
        #[arg(long)]
        stdin: bool,
        /// Account email (with --stdin; otherwise prompted). Also
        /// $GOVEE_EMAIL, then `config set username`.
        #[arg(long)]
        email: Option<String>,
    },
    /// Clear the stored account session (the API key stays).
    LogoutAccount,
}

pub async fn handle(ctx: &Ctx, cmd: &AuthCommand) -> Result<(), CliError> {
    match cmd {
        AuthCommand::Login(args) => login(ctx, args).await,
        AuthCommand::Status => status(ctx),
        AuthCommand::Logout(args) => logout(ctx, args),
        AuthCommand::SetCredential(args) => set_credential(ctx, args),
        AuthCommand::LoginAccount { code, stdin, email } => {
            login_account(ctx, code.as_deref(), *stdin, email.as_deref()).await
        }
        AuthCommand::LogoutAccount => logout_account(ctx),
    }
}

/// Move 0.1's keychain entries (service `govee-cli`) and record them in the
/// config so the gated reads find them. Reports what moved on stderr.
fn migrate_legacy(ctx: &Ctx) -> Result<legacy::Migrated, CliError> {
    let moved = legacy::migrate(
        &ctx.creds,
        legacy::Wanted {
            api_key: !ctx.cfg.api_key_in_keychain,
            account: ctx.cfg.username.is_none(),
        },
    )?;
    if moved.any() {
        let email = moved.account.as_ref().map(|a| a.email.clone());
        let key_moved = moved.api_key.is_some();
        ctx.update_config(|c| {
            if key_moved {
                c.api_key_in_keychain = true;
            }
            if let Some(e) = email {
                c.username = Some(e);
            }
        })?;
        if key_moved {
            ctx.note(&format!(
                "moved the API key stored by govee-cli 0.1 (keychain service `{}`) to `{}`",
                legacy::LEGACY_SERVICE,
                ctx.creds.service()
            ));
        }
        if let Some(a) = &moved.account {
            ctx.note(&format!(
                "moved the Govee account session for {} to `{}`",
                a.email,
                ctx.creds.service()
            ));
        }
    }
    Ok(moved)
}

async fn verify_key(ctx: &Ctx, key: &Secret) -> Result<usize, CliError> {
    let api = GoveeApi::new(key.expose().to_string(), ctx.verbose)?;
    let data = api.get_devices().await?;
    let n = data.as_array().map(|a| a.len()).unwrap_or(0);
    ctx.note(&format!("ok: {n} device(s) visible to this key"));
    Ok(n)
}

async fn login(ctx: &Ctx, args: &LoginArgs) -> Result<(), CliError> {
    let explicit = args.source.stdin || args.source.from_env.is_some();
    if args.non_interactive && !explicit {
        return Err(CliError::Usage(
            "--non-interactive never prompts: provide the key via --stdin or --from-env <VAR>"
                .into(),
        ));
    }
    // Upgrade path: at a terminal with nothing configured yet, a key stored
    // by 0.1 is moved instead of asked for again.
    if !ctx.cfg.api_key_in_keychain && !explicit && ctx.interactive {
        let moved = migrate_legacy(ctx)?;
        if let Some(key) = &moved.api_key {
            if !args.no_verify {
                verify_key(ctx, key).await?;
            }
            return Ok(());
        }
    }
    if ctx.cfg.api_key_in_keychain && ctx.creds.get(API_KEY_ITEM)?.is_some() && !args.overwrite {
        return Err(CliError::Usage(
            "an API key is already stored; pass --overwrite to replace it".into(),
        ));
    }
    let prompt = if args.non_interactive {
        None
    } else {
        Some("Govee API key")
    };
    let pasted = args.source.read(prompt)?;
    let key = Secret::new(pasted.expose().trim().to_string());
    if key.is_empty() {
        return Err(CliError::Usage("no API key given".into()));
    }
    if !args.no_verify {
        verify_key(ctx, &key).await?;
    }
    ctx.creds.set(API_KEY_ITEM, &key)?;
    ctx.update_config(|c| c.api_key_in_keychain = true)?;
    ctx.note(&format!(
        "API key stored in the OS keychain ({})",
        ctx.creds.service()
    ));
    Ok(())
}

fn status(ctx: &Ctx) -> Result<(), CliError> {
    let (source, in_keychain) = api_key::status(&ctx.cfg, &ctx.creds)?;
    let mut status = AuthStatus::new(true, source.is_some(), AuthMethod::Password);
    status.username = ctx.cfg.username.clone();
    status.credential_in_keychain = Some(in_keychain);

    // The app account session is a second, optional credential; reported as
    // an extra field (additive within auth-status/v1).
    let session = ctx.account()?;
    let signed_in = session.as_ref().is_some_and(|s| s.signed_in());
    let account_session = json!({
        "configured": ctx.cfg.username.is_some(),
        "signed_in": signed_in,
    });

    if ctx.json {
        let mut v = status.to_json();
        if let Some(s) = source {
            v["key_source"] = json!(s.as_str());
        }
        v["account_session"] = account_session;
        output::json(&v);
    } else {
        status.render();
        if let Some(s) = source {
            println!("Key source:    {}", s.as_str());
        }
        println!(
            "App account:   {}",
            match (&ctx.cfg.username, signed_in) {
                (Some(u), true) => format!("signed in as {u}"),
                (Some(u), false) => format!("{u} (not signed in; run `govee auth login-account`)"),
                (None, _) => "not signed in (run `govee auth login-account` for rooms)".to_string(),
            }
        );
    }
    Ok(())
}

fn logout(ctx: &Ctx, args: &LogoutArgs) -> Result<(), CliError> {
    if ctx.cfg.username.is_some() && account::clear(&ctx.creds)? {
        ctx.note("account session cleared");
    }
    if args.forget {
        if ctx.cfg.api_key_in_keychain {
            ctx.creds.delete(API_KEY_ITEM)?;
        }
        ctx.store.clear()?;
        ctx.note("API key removed from the keychain; config cleared");
    } else {
        ctx.note("API key kept (stateless); pass --forget to remove it");
    }
    Ok(())
}

fn set_credential(ctx: &Ctx, args: &SetCredentialArgs) -> Result<(), CliError> {
    if ctx.cfg.api_key_in_keychain && ctx.creds.get(API_KEY_ITEM)?.is_some() && !args.overwrite {
        return Err(CliError::Usage(
            "an API key is already stored; pass --overwrite to replace it".into(),
        ));
    }
    let pasted = args.source.read(None)?;
    let key = Secret::new(pasted.expose().trim().to_string());
    if key.is_empty() {
        return Err(CliError::Usage("no API key given".into()));
    }
    ctx.creds.set(API_KEY_ITEM, &key)?;
    ctx.update_config(|c| c.api_key_in_keychain = true)?;
    ctx.note("API key stored");
    Ok(())
}

fn logout_account(ctx: &Ctx) -> Result<(), CliError> {
    if ctx.cfg.username.is_none() {
        ctx.note("no account session configured; nothing to clear");
        return Ok(());
    }
    account::clear(&ctx.creds)?;
    ctx.note("account session cleared (the API key stays)");
    Ok(())
}

async fn login_account(
    ctx: &Ctx,
    code: Option<&str>,
    stdin: bool,
    email_flag: Option<&str>,
) -> Result<(), CliError> {
    use std::io::IsTerminal;
    let interactive = std::io::stdin().is_terminal() && !stdin;
    let env_password = std::env::var("GOVEE_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty());
    // The password's source must be knowable before any keychain read, so a
    // headless run without one stops here (exit 2), prompt-free.
    if !stdin && env_password.is_none() && !interactive {
        return Err(CliError::Usage(
            "not a terminal: pass the password on --stdin (or set $GOVEE_PASSWORD)".into(),
        ));
    }

    // The existing session, if any, for a stable client id and the
    // remembered email — from the new store, or moved from 0.1's.
    let mut existing = ctx.account()?;
    let mut migrated = false;
    if existing.is_none() && ctx.cfg.username.is_none() {
        let moved = migrate_legacy(ctx)?;
        if moved.account.is_some() {
            existing = moved.account;
            migrated = true;
        }
    }
    if let (true, None, Some(s)) = (migrated, code, existing.as_ref()) {
        if s.signed_in() {
            ctx.note("the moved session is ready to use; run again to sign in afresh");
            emit_one(
                ctx.json,
                "account-login",
                json!({ "status": "migrated", "email": s.email }),
            );
            return Ok(());
        }
    }
    // Keep the client id stable across attempts so the emailed code matches.
    let (client_id, remembered_email) = match existing {
        Some(a) => (a.client_id, Some(a.email)),
        None => (GoveeApp::new_client_id(), None),
    };
    let email = match email_flag
        .map(str::to_string)
        .or_else(|| std::env::var("GOVEE_EMAIL").ok().filter(|e| !e.is_empty()))
        .or_else(|| ctx.cfg.username.clone())
        .or(remembered_email)
    {
        Some(e) => e,
        None if interactive => {
            let e = prompt_line("Govee account email", None)?;
            if e.is_empty() {
                return Err(CliError::Usage("an email is required".into()));
            }
            e
        }
        None => {
            return Err(CliError::Usage(
                "no account email: pass --email (or set $GOVEE_EMAIL)".into(),
            ))
        }
    };
    let password = if stdin {
        let pw = read_stdin()?;
        if pw.is_empty() {
            return Err(CliError::Usage("no password on stdin".into()));
        }
        pw
    } else if let Some(p) = env_password {
        Secret::new(p)
    } else {
        Secret::prompt("Govee account password")?
    };

    let app = GoveeApp::new(client_id.clone(), ctx.verbose)?;
    // Remember the identity before any network call so a code-resume finds it.
    account::store(
        &ctx.creds,
        &AccountSession {
            token: String::new(),
            account_id: String::new(),
            client_id: client_id.clone(),
            email: email.clone(),
        },
    )?;
    ctx.update_config(|c| c.username = Some(email.clone()))?;

    let mut code = code.map(str::to_string);
    loop {
        match app
            .login(&email, password.expose(), code.as_deref())
            .await?
        {
            LoginOutcome::Token { token, account_id } => {
                account::store(
                    &ctx.creds,
                    &AccountSession {
                        token,
                        account_id: account_id.clone(),
                        client_id,
                        email: email.clone(),
                    },
                )?;
                ctx.note(&format!(
                    "account session stored in the OS keychain ({})",
                    ctx.creds.service()
                ));
                emit_one(
                    ctx.json,
                    "account-login",
                    json!({ "status": "authenticated", "email": email, "account_id": account_id }),
                );
                return Ok(());
            }
            LoginOutcome::NeedsCode => {
                if code.is_some() {
                    return Err(CliError::Upstream(
                        "Govee still wants a verification code; request a fresh one by running without --code".into(),
                    ));
                }
                app.request_code(&email).await?;
                ctx.note(&format!("Govee emailed a verification code to {email}."));
                if !interactive {
                    emit_one(
                        ctx.json,
                        "account-login",
                        json!({
                            "status": "code_sent",
                            "email": email,
                            "next": "re-run `govee auth login-account --stdin --code <CODE>`",
                        }),
                    );
                    return Ok(());
                }
                let c = prompt_line("Verification code", None)?;
                code = Some(c);
            }
        }
    }
}
