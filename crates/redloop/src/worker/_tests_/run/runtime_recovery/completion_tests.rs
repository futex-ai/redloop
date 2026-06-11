//! Recoverable completion mutation tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use unimock::{Clause, MockFn as _, Unimock, matching};

use crate::config::WorkerConfig;
use crate::error::InvalidConfigKind;
use crate::store::{QueueStore, QueueStoreMock};
use crate::types::JobOutcome;
use crate::worker::handler::RuntimeHandler;

use super::super::support::worker_config;
use super::support::{
    CompleteImmediately, assert_invalid_config, command_timeout, reap_ok, reserve_job,
    reserve_sentinel_error, run_with_timeout, worker,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completion_ack_timeout_is_recoverable_and_keeps_lease_active() {
    let completion_timed_out = Arc::new(AtomicBool::new(false));
    let completion_timed_out_store = Arc::clone(&completion_timed_out);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        heartbeat_tracks_completed_lease(Arc::clone(&completion_timed_out)),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                completion_timed_out_store.store(true, Ordering::SeqCst);
                Err(command_timeout("ack"))
            })),
        reap_ok(),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .returns(Ok(())),
        reserve_sentinel_error(),
    )));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(CompleteImmediately);

    assert_completion_timeout_recovery(store, handler, completion_timed_out).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completion_reschedule_timeout_is_recoverable_and_keeps_lease_active() {
    let completion_timed_out = Arc::new(AtomicBool::new(false));
    let completion_timed_out_store = Arc::clone(&completion_timed_out);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        heartbeat_tracks_completed_lease(Arc::clone(&completion_timed_out)),
        QueueStoreMock::complete_and_reschedule
            .next_call(matching!(
                "workers",
                "worker-test",
                "job-1",
                "lease-1",
                _,
                _
            ))
            .answers_arc(Arc::new(move |_, _, _, _, _, _, _| {
                completion_timed_out_store.store(true, Ordering::SeqCst);
                Err(command_timeout("complete_and_reschedule"))
            })),
        reap_ok(),
        QueueStoreMock::complete_and_reschedule
            .next_call(matching!(
                "workers",
                "worker-test",
                "job-1",
                "lease-1",
                _,
                _
            ))
            .returns(Ok(())),
        reserve_sentinel_error(),
    )));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(OutcomeImmediately {
        outcome: JobOutcome::Reschedule { schedule_at: None },
    });

    assert_completion_timeout_recovery(store, handler, completion_timed_out).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completion_fail_timeout_is_recoverable_and_keeps_lease_active() {
    let completion_timed_out = Arc::new(AtomicBool::new(false));
    let completion_timed_out_store = Arc::clone(&completion_timed_out);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        heartbeat_tracks_completed_lease(Arc::clone(&completion_timed_out)),
        QueueStoreMock::fail_or_retry
            .next_call(matching!("workers", _))
            .answers_arc(Arc::new(move |_, _, _| {
                completion_timed_out_store.store(true, Ordering::SeqCst);
                Err(command_timeout("fail_or_retry"))
            })),
        reap_ok(),
        QueueStoreMock::fail_or_retry
            .next_call(matching!("workers", _))
            .returns(Ok(1)),
        reserve_sentinel_error(),
    )));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(OutcomeImmediately {
        outcome: JobOutcome::Fail {
            message: "stop".to_owned(),
        },
    });

    assert_completion_timeout_recovery(store, handler, completion_timed_out).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completion_retryable_error_timeout_is_recoverable_and_keeps_lease_active() {
    let completion_timed_out = Arc::new(AtomicBool::new(false));
    let completion_timed_out_store = Arc::clone(&completion_timed_out);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        heartbeat_tracks_completed_lease(Arc::clone(&completion_timed_out)),
        QueueStoreMock::fail_or_retry
            .next_call(matching!("workers", _))
            .answers_arc(Arc::new(move |_, _, _| {
                completion_timed_out_store.store(true, Ordering::SeqCst);
                Err(command_timeout("fail_or_retry"))
            })),
        reap_ok(),
        QueueStoreMock::fail_or_retry
            .next_call(matching!("workers", _))
            .returns(Ok(1)),
        reserve_sentinel_error(),
    )));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(RetryableErrorImmediately);

    assert_completion_timeout_recovery(store, handler, completion_timed_out).await;
}

fn completion_config() -> WorkerConfig {
    let mut config = worker_config();
    config.heartbeat_interval = Duration::from_millis(5);
    config.poll_interval_min = Duration::from_millis(50);
    config.poll_interval_max = Duration::from_millis(50);
    config
}

fn heartbeat_tracks_completed_lease(completion_timed_out: Arc<AtomicBool>) -> impl Clause {
    QueueStoreMock::heartbeat
        .each_call(matching!(
            "workers",
            "worker-test",
            "job-1",
            "lease-1",
            _,
            _
        ))
        .answers_arc(Arc::new(move |_, _, _, _, _, _, _| {
            if completion_timed_out.load(Ordering::SeqCst) {
                completion_timed_out.store(false, Ordering::SeqCst);
            }
            Ok(())
        }))
}

async fn assert_completion_timeout_recovery(
    store: Arc<dyn QueueStore>,
    handler: Arc<dyn RuntimeHandler>,
    completion_timed_out: Arc<AtomicBool>,
) {
    let worker = worker(store, completion_config());

    let result = run_with_timeout(worker, handler).await;

    assert!(!completion_timed_out.load(Ordering::SeqCst));
    assert_invalid_config(result, InvalidConfigKind::EmptyRedisNodes);
}

struct OutcomeImmediately {
    outcome: JobOutcome,
}

#[async_trait]
impl RuntimeHandler for OutcomeImmediately {
    async fn handle(&self, _job_id: String) -> std::result::Result<JobOutcome, String> {
        Ok(self.outcome.clone())
    }
}

struct RetryableErrorImmediately;

#[async_trait]
impl RuntimeHandler for RetryableErrorImmediately {
    async fn handle(&self, _job_id: String) -> std::result::Result<JobOutcome, String> {
        Err("retryable".to_owned())
    }
}
