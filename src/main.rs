use clap::Parser;
use pk_cli_core::output;

fn main() {
    let cli = govee::cli::Cli::parse();
    if let Err(e) = govee::run(&cli) {
        std::process::exit(output::fail(&e, cli.common.json));
    }
}
