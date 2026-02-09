use clap::Parser;

use govee::cli::output::print_error;
use govee::cli::Cli;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    if let Err(e) = govee::run(args).await {
        print_error(&e);
        std::process::exit(e.exit_code());
    }
}
