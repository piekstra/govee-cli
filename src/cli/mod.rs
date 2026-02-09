pub mod auth;
pub mod devices;
pub mod light;
pub mod output;
pub mod power;
pub mod scene;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "govee", about = "CLI for Govee smart home devices", version)]
pub struct Cli {
    /// Output as human-readable table instead of JSON
    #[arg(short, long, global = true)]
    pub table: bool,

    /// Verbose output (show HTTP requests/responses)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authentication commands
    #[command(subcommand)]
    Auth(auth::AuthCommand),

    /// Manage devices
    #[command(subcommand)]
    Devices(devices::DevicesCommand),

    /// Control device power
    #[command(subcommand)]
    Power(power::PowerCommand),

    /// Light controls (brightness, color, temperature)
    #[command(subcommand)]
    Light(light::LightCommand),

    /// Dynamic scene controls
    #[command(subcommand)]
    Scene(scene::SceneCommand),
}
