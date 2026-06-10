//! Recoverable transient runtime error tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use unimock::{MockFn as _, Unimock, matching};

use crate::error::InvalidConfigKind;
use crate::store::{QueueStore, QueueStoreMock};
use crate::worker::handler::RuntimeHandler;

use super::super::support::worker_config;
use super::support::{
    WaitForFlagComplete, assert_invalid_config, command_timeout, lease_mismatch, reap_ok,
    reserve_empty_repeatedly, reserve_job, reserve_sentinel_error, run_with_timeout, worker,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_timeout_does_not_exit_worker() {
    let config = worker_config();
    let heartbeat_seen = Arc::new(AtomicBool::new(false));
    let heartbeat_seen_store = Arc::clone(&heartbeat_seen);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        QueueStoreMock::heartbeat
            .next_call(matching!(
                "workers",
                "worker-test",
                "job-1",
                "lease-1",
                _,
                _
            ))
            .answers_arc(Arc::new(move |_, _, _, _, _, _, _| {
                heartbeat_seen_store.store(true, Ordering::SeqCst);
                Err(command_timeout("heartbeat"))
            })),
        reap_ok(),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .returns(Ok(())),
        reserve_sentinel_error(),
    )));
    let worker = worker(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(WaitForFlagComplete {
        flag: heartbeat_seen,
    });

    let result = run_with_timeout(worker, handler).await;

    assert_invalid_config(result, InvalidConfigKind::EmptyRedisNodes);
}

#[tokio::test]
async fn reap_timeout_does_not_exit_worker() {
    let mut config = worker_config();
    config.reap_interval = Duration::from_millis(1);
    config.poll_interval_min = Duration::from_millis(1);
    config.poll_interval_max = Duration::from_millis(1);
    let reap_called = Arc::new(AtomicBool::new(false));
    let reap_called_store = Arc::clone(&reap_called);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_empty_repeatedly(),
        QueueStoreMock::reap_expired
            .each_call(matching!("workers", _, 1))
            .answers_arc(Arc::new(move |_, _, _, _| {
                reap_called_store.store(true, Ordering::SeqCst);
                Err(command_timeout("reap_expired"))
            })),
    )));
    let worker = worker(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(Unimock::new(()));

    let result =
        tokio::time::timeout(Duration::from_millis(50), worker.run_internal(handler)).await;

    assert!(result.is_err());
    assert!(reap_called.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_lease_mismatch_marks_attempt_lost_and_keeps_worker_running() {
    let mut config = worker_config();
    config.heartbeat_interval = Duration::from_millis(1);
    config.poll_interval_min = Duration::from_millis(1);
    config.poll_interval_max = Duration::from_millis(1);
    let heartbeat_seen = Arc::new(AtomicBool::new(false));
    let heartbeat_seen_store = Arc::clone(&heartbeat_seen);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        QueueStoreMock::heartbeat
            .next_call(matching!(
                "workers",
                "worker-test",
                "job-1",
                "lease-1",
                _,
                _
            ))
            .answers_arc(Arc::new(move |_, _, _, _, _, _, _| {
                heartbeat_seen_store.store(true, Ordering::SeqCst);
                Err(lease_mismatch("job-1"))
            })),
        reap_ok(),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .returns(Err(lease_mismatch("job-1"))),
        reserve_sentinel_error(),
    )));
    let worker = worker(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(WaitForFlagComplete {
        flag: Arc::clone(&heartbeat_seen),
    });

    let result = run_with_timeout(worker, handler).await;

    assert!(heartbeat_seen.load(Ordering::SeqCst));
    assert_invalid_config(result, InvalidConfigKind::EmptyRedisNodes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completion_lease_mismatch_drops_completion_and_keeps_worker_running() {
    let heartbeat_seen = Arc::new(AtomicBool::new(false));
    let heartbeat_seen_store = Arc::clone(&heartbeat_seen);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        reserve_job("job-1", "lease-1"),
        QueueStoreMock::heartbeat
            .next_call(matching!(
                "workers",
                "worker-test",
                "job-1",
                "lease-1",
                _,
                _
            ))
            .answers_arc(Arc::new(move |_, _, _, _, _, _, _| {
                heartbeat_seen_store.store(true, Ordering::SeqCst);
                Ok(())
            })),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .returns(Err(lease_mismatch("job-1"))),
        reap_ok(),
        reserve_sentinel_error(),
    )));
    let worker = worker(store, worker_config());
    let handler: Arc<dyn RuntimeHandler> = Arc::new(WaitForFlagComplete {
        flag: heartbeat_seen,
    });

    let result = run_with_timeout(worker, handler).await;

    assert_invalid_config(result, InvalidConfigKind::EmptyRedisNodes);
}
