mod cli;
mod commands;
mod error;
mod output;
mod upgrade;

use clap::Parser;

use crate::{
    cli::{Cli, Command},
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let Cli { connect, command } = Cli::parse();
    let command = match command {
        Command::Upgrade(args) => {
            upgrade::run("redloop-cli", args).await?;
            return Ok(());
        }
        command => command,
    };
    commands::run_command(connect, command).await
}
