use crate::types::JobState;
use thiserror::Error;

/// Crate-wide result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Public Redloop error surface.
#[derive(Debug, Error)]
pub enum Error {
    #[error("[redloop/error] job_id exceeds 256 bytes: {length}")]
    JobIdTooLong { length: usize },
    #[error("[redloop/error] invalid state for job '{job_id}': {state}")]
    InvalidState { job_id: String, state: JobState },
    #[error("[redloop/error] job '{job_id}' not found")]
    NotFound { job_id: String },
    #[error("[redloop/error] lease mismatch for job '{job_id}'")]
    LeaseMismatch { job_id: String },
    #[error("[redloop/error] invalid config: {kind}")]
    InvalidConfig { kind: InvalidConfigKind },
    #[error("[redloop/error] invalid cursor '{cursor}'")]
    InvalidCursor { cursor: String },
    #[error("[redloop/error] invalid timestamp micros: {micros}")]
    InvalidTimestamp { micros: i64 },
    #[error("[redloop/error] invalid stored value for field '{field}'")]
    InvalidData { field: &'static str },
    #[error("[redloop/error] redis operation '{operation}' failed: {source}")]
    Redis {
        operation: &'static str,
        #[source]
        source: redis::RedisError,
    },
    #[error("[redloop/error] redis operation '{operation}' timed out after {timeout_ms}ms")]
    CommandTimedOut {
        operation: &'static str,
        timeout_ms: u64,
    },
    #[error("[redloop/error] worker task failed: {source}")]
    WorkerTaskJoin {
        #[source]
        source: tokio::task::JoinError,
    },
}

/// Structured invalid configuration kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidConfigKind {
    ConcurrencyZero,
    LeaseDurationZero,
    HeartbeatIntervalZero,
    HeartbeatNotShorterThanLease,
    ReapIntervalZero,
    PollIntervalMinZero,
    PollIntervalRange,
    EmptyRedisNodes,
    EmptySentinelServiceName,
    RetryPolicyMismatch,
}

impl std::fmt::Display for InvalidConfigKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            InvalidConfigKind::ConcurrencyZero => "worker concurrency must be greater than zero",
            InvalidConfigKind::LeaseDurationZero => "lease_duration must be greater than zero",
            InvalidConfigKind::HeartbeatIntervalZero => {
                "heartbeat_interval must be greater than zero"
            }
            InvalidConfigKind::HeartbeatNotShorterThanLease => {
                "heartbeat_interval must be shorter than lease_duration"
            }
            InvalidConfigKind::ReapIntervalZero => "reap_interval must be greater than zero",
            InvalidConfigKind::PollIntervalMinZero => "poll_interval_min must be greater than zero",
            InvalidConfigKind::PollIntervalRange => {
                "poll_interval_min must be less than or equal to poll_interval_max"
            }
            InvalidConfigKind::EmptyRedisNodes => "redis deployment requires at least one node",
            InvalidConfigKind::EmptySentinelServiceName => {
                "sentinel deployment requires a service_name"
            }
            InvalidConfigKind::RetryPolicyMismatch => {
                "worker retry_policy does not match the namespace retry_policy"
            }
        };

        f.write_str(value)
    }
}
