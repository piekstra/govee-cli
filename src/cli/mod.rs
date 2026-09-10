//! The command tree (SPEC v1 surface + the Govee domain nouns) and the
//! per-invocation context every handler receives.

pub mod api;
pub mod auth;
pub mod config;
pub mod devices;
pub mod light;
pub mod music;
pub mod output;
pub mod power;
pub mod rooms;
pub mod scene;
pub mod segment;
pub mod toggle;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use pk_cli_config::ConfigStore;
use pk_cli_core::{CliError, CommonArgs};
use pk_cli_secrets::CredentialStore;
use pk_cli_selfupdate::SelfUpdateArgs;

use crate::api::client::GoveeApi;
use crate::auth::{account, api_key};
use crate::config::Config;

#[derive(Parser, Debug)]
#[command(
    name = crate::BIN,
    version,
    about = "Govee smart-home devices from the terminal (conforms to piekstra-cli/1)",
    long_about = None
)]
pub struct Cli {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Override the config file location.
    #[arg(long, global = true, value_name = "PATH", env = "GOVEE_CONFIG")]
    pub config: Option<std::path::PathBuf>,

    /// Accepted for 0.1 compatibility and ignored: text is the default now,
    /// `--json` selects JSON.
    #[arg(short = 't', long, global = true, hide = true)]
    pub table: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Credential management and session status.
    #[command(subcommand)]
    Auth(auth::AuthCommand),
    /// Non-secret settings.
    #[command(subcommand)]
    Config(config::ConfigCommand),
    /// Devices on the account: list, get, search, capabilities.
    #[command(subcommand)]
    Devices(devices::DevicesCommand),
    /// Power: on, off, toggle, status.
    #[command(subcommand)]
    Power(power::PowerCommand),
    /// Light controls: brightness, color, temperature, state.
    #[command(subcommand)]
    Light(light::LightCommand),
    /// Dynamic scenes, DIY scenes and snapshots.
    #[command(subcommand)]
    Scene(scene::SceneCommand),
    /// Toggle features (gradient, DreamView).
    #[command(subcommand)]
    Toggle(toggle::ToggleCommand),
    /// Per-segment color and brightness.
    #[command(subcommand)]
    Segment(segment::SegmentCommand),
    /// Music mode.
    #[command(subcommand)]
    Music(music::MusicCommand),
    /// Rooms as the Govee Home app has them (needs `auth login-account`).
    #[command(subcommand)]
    Rooms(rooms::RoomsCommand),
    /// Raw Platform API passthrough: `api GET /user/devices`.
    Api(api::ApiCommand),
    /// Update to the latest release from GitHub.
    SelfUpdate(SelfUpdateArgs),
    /// Print a shell completion script.
    Completions { shell: Shell },
    /// Machine-readable capability discovery (cli-info/v1).
    Info,
}

/// What every credentialed handler gets: output mode, the config, and the
/// stores. Nothing here has touched the keychain or the network yet.
pub struct Ctx {
    pub json: bool,
    pub verbose: bool,
    pub quiet: bool,
    /// Prompting is acceptable: stdin is a TTY and no `--json`.
    pub interactive: bool,
    pub store: ConfigStore,
    pub creds: CredentialStore,
    pub cfg: Config,
}

impl Ctx {
    /// A Platform API client, or exit 3 before any network call.
    pub fn api(&self) -> Result<GoveeApi, CliError> {
        let (key, _) = api_key::resolve(&self.cfg, &self.creds)?;
        Ok(GoveeApi::new(key.expose().to_string(), self.verbose)?)
    }

    /// The app account session if one is configured and stored; `None`
    /// when no account was ever signed in (no keychain read in that case).
    pub fn account(&self) -> Result<Option<account::AccountSession>, CliError> {
        account::load(&self.cfg, &self.creds)
    }

    /// The app account session, or exit 3 pointing at `auth login-account`.
    pub fn require_account(&self) -> Result<account::AccountSession, CliError> {
        account::require(&self.cfg, &self.creds)
    }

    /// Re-read, edit and save the config file.
    pub fn update_config(&self, f: impl FnOnce(&mut Config)) -> Result<(), CliError> {
        let mut cfg: Config = self.store.load()?;
        f(&mut cfg);
        self.store.save(&cfg)
    }

    pub fn note(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{msg}");
        }
    }
}

/// The mutation gate (SPEC §1.3). Call **before** any credential or network
/// work: with `--force` it passes; non-interactive without it is exit 6, so
/// a driver never hangs on a prompt.
pub fn require_confirmable(force: bool, interactive: bool, what: &str) -> Result<(), CliError> {
    if force || interactive {
        Ok(())
    } else {
        Err(CliError::ConfirmationRequired(format!(
            "{what} — pass --force to run non-interactively"
        )))
    }
}

/// Interactive yes/no on stderr; only reached when `require_confirmable`
/// passed without `--force`.
pub fn confirm(force: bool, prompt: &str) -> Result<(), CliError> {
    if force {
        return Ok(());
    }
    eprint!("{prompt} [y/N] ");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| CliError::Other(format!("reading confirmation: {e}")))?;
    if matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(CliError::ConfirmationRequired("cancelled".into()))
    }
}

/// Read one line from stdin after a stderr prompt (emails, codes — never
/// secrets; those go through `Secret::prompt`).
pub fn prompt_line(label: &str, default: Option<&str>) -> Result<String, CliError> {
    match default {
        Some(d) => eprint!("{label} [{d}]: "),
        None => eprint!("{label}: "),
    }
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| CliError::Other(format!("reading input: {e}")))?;
    let line = line.trim().to_string();
    if line.is_empty() {
        if let Some(d) = default {
            return Ok(d.to_string());
        }
    }
    Ok(line)
}
