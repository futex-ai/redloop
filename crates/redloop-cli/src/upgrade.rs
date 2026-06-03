//! Explicit Redloop CLI upgrade support.

use std::{path::PathBuf, sync::Arc};

use clap::{Args, Subcommand};
use thiserror::Error;

const DEFAULT_REDLOOP_RELEASE_SERVER_URL: &str =
    "https://redloop-cli-release-993259844560.europe-west2.run.app";
const REDLOOP_RELEASE_SERVER_URL_ENV: &str = "REDLOOP_RELEASE_SERVER_URL";

/// Arguments for explicit Redloop CLI upgrades.
#[derive(Args, Debug)]
pub(crate) struct UpgradeArgs {
    #[arg(long, global = true)]
    release_server: Option<String>,
    #[arg(long, global = true)]
    target: Option<String>,
    #[arg(long, global = true, default_value_t = false)]
    force: bool,
    #[command(subcommand)]
    command: Option<UpgradeCommand>,
}

#[derive(Subcommand, Debug)]
enum UpgradeCommand {
    Status,
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("[redloop_cli/upgrade] updater error: {0}")]
    Updater(#[from] cli_updater::Error),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub(crate) async fn run(binary: &'static str, args: UpgradeArgs) -> Result<()> {
    let status_only = matches!(args.command, Some(UpgradeCommand::Status));
    let target = resolve_target(args.target)?;
    let check = build_update_check(binary, target);
    let updater = build_updater(resolve_release_server(args.release_server))?;

    if status_only {
        let status = updater.status(check).await?;
        println!("{}", cli_updater::format_status_summary(&status));
        if status.update_available {
            println!(
                "run `{binary} upgrade` to install {}",
                status.latest.version
            );
        }
        return Ok(());
    }

    let current_exe = current_exe()?;
    match updater
        .upgrade(cli_updater::UpdateRequest {
            check,
            current_exe,
            force: args.force,
        })
        .await?
    {
        cli_updater::UpgradeOutcome::UpToDate(status) => {
            println!("{}", cli_updater::format_status_summary(&status));
        }
        cli_updater::UpgradeOutcome::Installed(status) => {
            println!("updated {binary} to {}", status.latest.version);
        }
    }
    Ok(())
}

fn current_exe() -> Result<PathBuf> {
    std::env::current_exe()
        .map_err(|source| Error::Updater(cli_updater::Error::CurrentExe { source }))
}

fn resolve_target(target: Option<String>) -> Result<String> {
    match target {
        Some(target) => Ok(target),
        None => Ok(cli_updater::current_target()?.to_owned()),
    }
}

fn build_update_check(binary: &'static str, target: String) -> cli_updater::UpdateCheck {
    cli_updater::UpdateCheck {
        binary: binary.to_owned(),
        current_version: env!("CARGO_PKG_VERSION").to_owned(),
        target,
    }
}

fn resolve_release_server(release_server: Option<String>) -> String {
    resolve_release_server_with_env(
        release_server,
        std::env::var(REDLOOP_RELEASE_SERVER_URL_ENV).ok(),
    )
}

fn resolve_release_server_with_env(
    release_server: Option<String>,
    env_value: Option<String>,
) -> String {
    release_server
        .or_else(|| env_value.filter(|value| !value.is_empty()))
        .unwrap_or_else(|| DEFAULT_REDLOOP_RELEASE_SERVER_URL.to_owned())
}

fn build_updater(server_url: String) -> Result<cli_updater::Updater> {
    Ok(cli_updater::Updater::new(
        Arc::new(cli_updater::HttpReleaseClient::new(server_url)?),
        Arc::new(cli_updater::TarGzBinaryInstaller::new()),
    ))
}

#[cfg(test)]
#[path = "_tests_/upgrade_tests.rs"]
mod upgrade_tests;
