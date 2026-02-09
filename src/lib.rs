pub mod api;
pub mod auth;
pub mod cli;
pub mod config;
pub mod error;
pub mod models;
pub mod resolve;

use cli::{Cli, Commands};
use config::{OutputMode, RuntimeConfig};
use error::AppError;

pub async fn run(args: Cli) -> Result<(), AppError> {
    let config = RuntimeConfig {
        output_mode: if args.table {
            OutputMode::Table
        } else {
            OutputMode::Json
        },
        verbose: args.verbose,
    };

    dispatch(&args.command, &config).await
}

async fn dispatch(command: &Commands, config: &RuntimeConfig) -> Result<(), AppError> {
    match command {
        Commands::Auth(cmd) => cli::auth::handle(cmd, config).await,
        Commands::Devices(cmd) => cli::devices::handle(cmd, config).await,
        Commands::Power(cmd) => cli::power::handle(cmd, config).await,
        Commands::Light(cmd) => cli::light::handle(cmd, config).await,
        Commands::Scene(cmd) => cli::scene::handle(cmd, config).await,
        Commands::Toggle(cmd) => cli::toggle::handle(cmd, config).await,
        Commands::Segment(cmd) => cli::segment::handle(cmd, config).await,
        Commands::Music(cmd) => cli::music::handle(cmd, config).await,
    }
}
