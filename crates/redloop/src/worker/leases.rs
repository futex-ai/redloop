//! Worker lease bookkeeping helpers.

use std::time::Duration;

use uuid::Uuid;

use crate::config::Timestamp;
use crate::types::JobOutcome;

pub(crate) struct ActiveLease {
    pub(crate) job_id: String,
    pub(crate) lease_token: String,
}

pub(crate) struct CompletedLease {
    pub(crate) job_id: String,
    pub(crate) lease_token: String,
    pub(crate) result: std::result::Result<JobOutcome, String>,
}

pub(crate) fn new_lease_tokens(count: usize) -> Vec<String> {
    (0..count).map(|_| Uuid::now_v7().to_string()).collect()
}

pub(crate) fn next_poll_delay(current: Duration, max: Duration) -> Duration {
    (current.saturating_mul(2)).min(max)
}

pub(crate) fn sleep_duration(
    now: Timestamp,
    poll_delay: Duration,
    next_schedule_at: Option<Timestamp>,
) -> Duration {
    match next_schedule_at {
        Some(schedule_at) if schedule_at > now => {
            let due_in = (schedule_at - now).to_std().unwrap_or(Duration::ZERO);
            due_in.min(poll_delay)
        }
        _ => poll_delay,
    }
}
