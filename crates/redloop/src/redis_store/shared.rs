//! Shared parsing and Redis helper functions.

use crate::config::{Backoff, RetryPolicy, Timestamp};
use crate::error::{Error, InvalidConfigKind, Result};
use crate::store::{FailureAction, FailureRequest, ReplaceMode};
use crate::types::{EnqueueStatus, JobState};

use super::connection::RedisDriver;

pub(crate) async fn hmget<T: redis::FromRedisValue>(
    driver: &RedisDriver,
    timeout: std::time::Duration,
    operation: &'static str,
    key: &str,
    fields: &[String],
) -> Result<Vec<T>> {
    if fields.is_empty() {
        return Ok(Vec::new());
    }

    let mut command = redis::cmd("HMGET");
    command.arg(key).arg(fields);
    driver.query_cmd(operation, timeout, &command).await
}

pub(crate) fn parse_script_error(job_id: &str, response: &[String]) -> Error {
    match response.get(1).map(String::as_str) {
        Some("not_found") => Error::NotFound {
            job_id: job_id.to_string(),
        },
        Some("invalid_state") => {
            let state = response
                .get(2)
                .and_then(|value| JobState::from_str(value))
                .unwrap_or(JobState::Failed);
            Error::InvalidState {
                job_id: job_id.to_string(),
                state,
            }
        }
        Some("lease_mismatch") => Error::LeaseMismatch {
            job_id: job_id.to_string(),
        },
        Some("invalid_config") => Error::InvalidConfig {
            kind: InvalidConfigKind::RetryPolicyMismatch,
        },
        _ => Error::InvalidData {
            field: "script.response",
        },
    }
}

pub(crate) fn parse_lease_response(job_id: &str, response: Vec<String>) -> Result<()> {
    if response.first().map(String::as_str) == Some("ok") {
        Ok(())
    } else {
        Err(parse_script_error(job_id, &response))
    }
}

pub(crate) fn parse_state(value: &str) -> Result<JobState> {
    JobState::from_str(value).ok_or(Error::InvalidData { field: "job_state" })
}

pub(crate) fn parse_enqueue_status(value: &str) -> Result<EnqueueStatus> {
    EnqueueStatus::from_str(value).ok_or(Error::InvalidData {
        field: "enqueue.status",
    })
}

pub(crate) fn parse_i64(field: &'static str, value: &str) -> Result<i64> {
    value
        .parse::<i64>()
        .map_err(|_| Error::InvalidData { field })
}

pub(crate) fn micros(timestamp: Timestamp) -> i64 {
    timestamp.timestamp_micros()
}

pub(crate) fn duration_micros(duration: std::time::Duration) -> i64 {
    duration.as_micros().min(i64::MAX as u128) as i64
}

pub(crate) fn from_micros(value: i64) -> Result<Timestamp> {
    chrono::DateTime::<chrono::Utc>::from_timestamp_micros(value)
        .ok_or(Error::InvalidTimestamp { micros: value })
}

pub(crate) fn replace_mode(mode: ReplaceMode) -> &'static str {
    match mode {
        ReplaceMode::Always => "replace",
        ReplaceMode::IfEarlier => "earlier",
        ReplaceMode::IfLater => "later",
    }
}

pub(crate) fn failure_args(request: &FailureRequest) -> Vec<String> {
    let mut args = vec![
        request.job_id.clone(),
        request.worker_id.clone(),
        request.lease_token.clone(),
        micros(request.now).to_string(),
        match request.action {
            FailureAction::Retryable => "retryable".to_string(),
            FailureAction::Terminal => "terminal".to_string(),
        },
    ];

    match &request.retry_policy {
        RetryPolicy::Never => {
            args.extend(["never", "0", "none", "0", "0", "0"].map(str::to_string));
        }
        RetryPolicy::Count {
            max_retries,
            backoff,
        } => {
            args.push("count".to_string());
            args.push(max_retries.to_string());
            args.extend(backoff_args(backoff));
        }
        RetryPolicy::Infinite { backoff } => {
            args.push("infinite".to_string());
            args.push("0".to_string());
            args.extend(backoff_args(backoff));
        }
    }

    args
}

fn backoff_args(backoff: &Backoff) -> [String; 4] {
    match backoff {
        Backoff::None => ["none".into(), "0".into(), "0".into(), "0".into()],
        Backoff::Fixed { delay_ms } => {
            ["fixed".into(), delay_ms.to_string(), "0".into(), "0".into()]
        }
        Backoff::Exponential {
            initial_delay_ms,
            multiplier,
            max_delay_ms,
        } => [
            "exponential".into(),
            initial_delay_ms.to_string(),
            multiplier.to_string(),
            max_delay_ms.to_string(),
        ],
    }
}
