mod app;
mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    match app::execute(cli::Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("equalturn: {error}");
            ExitCode::FAILURE
        }
    }
}
