//! `govee` — Govee smart-home devices from the terminal. Conforms to
//! piekstra-cli/1: `--json` everywhere, the family exit codes, keychain-only
//! secrets under `piekstra.govee`.
//!
//! Two upstreams: the public Platform API (device control) and the Govee
//! Home app's private API (rooms; see `docs/api.md`).

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;
pub mod error;
pub mod models;
pub mod resolve;

use clap::CommandFactory;
use pk_cli_config::ConfigStore;
use pk_cli_core::info::{AuthInfo, CliInfo};
use pk_cli_core::{output, CliError};
use pk_cli_secrets::CredentialStore;
use pk_cli_selfupdate::Updater;

use cli::{Cli, Commands, Ctx};
use config::Config;

pub const BIN: &str = "govee";
pub const REPO: &str = "piekstra/govee-cli";

/// What `info` advertises.
pub const CAPABILITIES: &[&str] = &[
    "devices", "power", "light", "scene", "toggle", "segment", "music", "rooms", "api",
];

pub fn run(cli: &Cli) -> Result<(), CliError> {
    let store = ConfigStore::new(BIN).with_override(cli.config.clone());

    // Offline commands first: they must never touch the keychain or the
    // vendor network. `self-update` talks to GitHub with its own blocking
    // client, so it runs outside the async runtime.
    match &cli.command {
        Commands::Config(cmd) => return cli::config::run(cli.common.json, cmd, &store),
        Commands::SelfUpdate(args) => {
            return Updater {
                repo: REPO.into(),
                binary: BIN.into(),
                target: env!("BUILD_TARGET").into(),
                current: env!("CARGO_PKG_VERSION").into(),
            }
            .run(args, cli.common.json, cli.common.quiet)
        }
        Commands::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), BIN, &mut std::io::stdout());
            return Ok(());
        }
        Commands::Info => {
            let info = CliInfo::new(
                BIN,
                env!("CARGO_PKG_VERSION"),
                &format!("https://github.com/{REPO}"),
                AuthInfo {
                    required: true,
                    method: "password".into(),
                    login_hint: Some(format!("{BIN} auth login")),
                },
                CAPABILITIES,
            );
            output::json(&serde_json::to_value(&info).unwrap_or_default());
            return Ok(());
        }
        _ => {}
    }

    let cfg: Config = store.load()?;
    let ctx = Ctx {
        json: cli.common.json,
        verbose: cli.common.verbose,
        quiet: cli.common.quiet,
        interactive: cli.common.interactive(),
        store,
        creds: CredentialStore::for_binary(BIN),
        cfg,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Other(format!("starting the async runtime: {e}")))?;
    rt.block_on(dispatch(&ctx, &cli.command))
}

async fn dispatch(ctx: &Ctx, command: &Commands) -> Result<(), CliError> {
    match command {
        Commands::Auth(cmd) => cli::auth::handle(ctx, cmd).await,
        Commands::Devices(cmd) => cli::devices::handle(ctx, cmd).await,
        Commands::Power(cmd) => cli::power::handle(ctx, cmd).await,
        Commands::Light(cmd) => cli::light::handle(ctx, cmd).await,
        Commands::Scene(cmd) => cli::scene::handle(ctx, cmd).await,
        Commands::Toggle(cmd) => cli::toggle::handle(ctx, cmd).await,
        Commands::Segment(cmd) => cli::segment::handle(ctx, cmd).await,
        Commands::Music(cmd) => cli::music::handle(ctx, cmd).await,
        Commands::Rooms(cmd) => cli::rooms::handle(ctx, cmd).await,
        Commands::Api(args) => {
            // Method and body are validated before any credential is read.
            let (method, body) = cli::api::validate(args)?;
            cli::api::run(ctx, args, method, body).await
        }
        Commands::Config(_)
        | Commands::SelfUpdate(_)
        | Commands::Completions { .. }
        | Commands::Info => unreachable!("handled before the runtime starts"),
    }
}
