//! Shared worker runtime test helpers.

use std::time::Duration;

use crate::config::{Backoff, RetryPolicy, WorkerConfig};
use crate::error::{Error, InvalidConfigKind};

pub(super) fn worker_config() -> WorkerConfig {
    worker_config_with_concurrency(1)
}

pub(super) fn worker_config_with_concurrency(concurrency: usize) -> WorkerConfig {
    WorkerConfig {
        worker_id: "worker-test".to_owned(),
        concurrency,
        retry_policy: RetryPolicy::Infinite {
            backoff: Backoff::Fixed { delay_ms: 100 },
        },
        lease_duration: Duration::from_secs(30),
        heartbeat_interval: Duration::from_secs(5),
        reap_interval: Duration::from_secs(10),
        poll_interval_min: Duration::from_millis(50),
        poll_interval_max: Duration::from_secs(1),
    }
}

pub(super) fn redis_timeout_error() -> redis::RedisError {
    std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out").into()
}

pub(super) fn unused_store_error() -> Error {
    Error::InvalidConfig {
        kind: InvalidConfigKind::EmptyRedisNodes,
    }
}
