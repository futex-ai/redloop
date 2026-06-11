//! Worker runtime recovery helpers.

use std::time::{Duration, Instant};

use crate::error::Result;

pub(crate) fn recoverable_runtime_result(
    operation: &'static str,
    result: Result<()>,
) -> Result<bool> {
    match result {
        Ok(()) => Ok(false),
        Err(error) if error.is_recoverable_worker_runtime() => {
            tracing::warn!(
                operation,
                error = %error,
                "worker runtime operation hit recoverable Redis error; backing off before retry"
            );
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn runtime_backoff_deadline(duration: Duration) -> Instant {
    match Instant::now().checked_add(duration) {
        Some(deadline) => deadline,
        None => Instant::now(),
    }
}

pub(crate) fn runtime_backoff_ready(deadline: Option<Instant>) -> bool {
    match deadline {
        Some(deadline) => Instant::now() >= deadline,
        None => true,
    }
}

pub(crate) fn runtime_backoff_remaining(deadline: Option<Instant>) -> Duration {
    match deadline {
        Some(deadline) => deadline.saturating_duration_since(Instant::now()),
        None => Duration::ZERO,
    }
}
