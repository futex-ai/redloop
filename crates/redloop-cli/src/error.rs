use thiserror::Error;

/// Result type used by the CLI entrypoint.
pub(crate) type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("[redloop_cli/main] redloop error: {0}")]
    Redloop(#[from] redloop::Error),
    #[error("[redloop_cli/main] upgrade error: {0}")]
    Upgrade(#[from] crate::upgrade::Error),
}
