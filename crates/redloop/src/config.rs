use crate::error::{Error, InvalidConfigKind, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::time::Duration;

/// Shared Redloop timestamp type.
pub type Timestamp = DateTime<Utc>;

/// Redis connection configuration.
#[derive(Debug, Clone)]
pub struct ConnectConfig {
    pub deployment: RedisDeployment,
    pub key_prefix: String,
    /// Per-command/pipeline deadline, also retained by Redis connections on reconnect.
    pub command_timeout: Duration,
}

/// Supported Redis deployment modes.
#[derive(Debug, Clone)]
pub enum RedisDeployment {
    Standalone {
        url: String,
    },
    Sentinel {
        service_name: String,
        nodes: Vec<String>,
    },
    Cluster {
        nodes: Vec<String>,
    },
}

/// Retry policy applied per namespace by workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RetryPolicy {
    Never,
    Count { max_retries: u32, backoff: Backoff },
    Infinite { backoff: Backoff },
}

/// Backoff strategy for handler failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Backoff {
    None,
    Fixed {
        delay_ms: u64,
    },
    Exponential {
        initial_delay_ms: u64,
        multiplier: u32,
        max_delay_ms: u64,
    },
}

/// Worker runtime configuration.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub concurrency: usize,
    pub retry_policy: RetryPolicy,
    pub lease_duration: Duration,
    pub heartbeat_interval: Duration,
    pub reap_interval: Duration,
    pub poll_interval_min: Duration,
    pub poll_interval_max: Duration,
}

impl ConnectConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        self.deployment.validate()
    }
}

impl RedisDeployment {
    fn validate(&self) -> Result<()> {
        match self {
            RedisDeployment::Standalone { .. } => Ok(()),
            RedisDeployment::Sentinel {
                service_name,
                nodes,
            } => {
                if service_name.is_empty() {
                    return Err(Error::InvalidConfig {
                        kind: InvalidConfigKind::EmptySentinelServiceName,
                    });
                }
                if nodes.is_empty() {
                    return Err(Error::InvalidConfig {
                        kind: InvalidConfigKind::EmptyRedisNodes,
                    });
                }
                Ok(())
            }
            RedisDeployment::Cluster { nodes } => {
                if nodes.is_empty() {
                    return Err(Error::InvalidConfig {
                        kind: InvalidConfigKind::EmptyRedisNodes,
                    });
                }
                Ok(())
            }
        }
    }
}

impl WorkerConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.concurrency == 0 {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::ConcurrencyZero,
            });
        }
        if self.lease_duration.is_zero() {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::LeaseDurationZero,
            });
        }
        if self.heartbeat_interval.is_zero() {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::HeartbeatIntervalZero,
            });
        }
        if self.heartbeat_interval >= self.lease_duration {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::HeartbeatNotShorterThanLease,
            });
        }
        if self.reap_interval.is_zero() {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::ReapIntervalZero,
            });
        }
        if self.poll_interval_min.is_zero() {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::PollIntervalMinZero,
            });
        }
        if self.poll_interval_min > self.poll_interval_max {
            return Err(Error::InvalidConfig {
                kind: InvalidConfigKind::PollIntervalRange,
            });
        }
        Ok(())
    }

    pub(crate) fn retry_policy_key(&self) -> String {
        match &self.retry_policy {
            RetryPolicy::Never => "never".to_string(),
            RetryPolicy::Count {
                max_retries,
                backoff,
            } => format!("count:{max_retries}:{}", backoff.key()),
            RetryPolicy::Infinite { backoff } => format!("infinite:{}", backoff.key()),
        }
    }
}

impl Backoff {
    fn key(&self) -> String {
        match self {
            Backoff::None => "none".to_string(),
            Backoff::Fixed { delay_ms } => format!("fixed:{delay_ms}"),
            Backoff::Exponential {
                initial_delay_ms,
                multiplier,
                max_delay_ms,
            } => format!("exponential:{initial_delay_ms}:{multiplier}:{max_delay_ms}"),
        }
    }
}

#[cfg(test)]
#[path = "_tests_/config_tests.rs"]
mod config_tests;
