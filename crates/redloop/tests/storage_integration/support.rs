//! Shared helpers for storage integration tests.

use async_trait::async_trait;
use chrono::{Timelike, Utc};
use redloop::{JobHandler, JobOutcome};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(crate) use super::test_support::{RedisTestContext, wait_until};

pub(crate) fn persisted_timestamp(value: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    value
        .with_nanosecond(value.timestamp_subsec_micros() * 1_000)
        .expect("timestamp should remain valid after precision truncation")
}

pub(crate) struct CompleteOnceHandler {
    pub(crate) processed: Arc<AtomicUsize>,
}

#[async_trait]
impl JobHandler for CompleteOnceHandler {
    async fn handle(&self, _job_id: String) -> redloop::HandlerResult {
        self.processed.fetch_add(1, Ordering::SeqCst);
        Ok(JobOutcome::Complete)
    }
}
