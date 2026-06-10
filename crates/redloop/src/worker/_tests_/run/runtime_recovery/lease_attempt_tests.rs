//! Lease-attempt bookkeeping recovery tests.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use unimock::{MockFn as _, Unimock, matching};

use crate::error::{Error, InvalidConfigKind};
use crate::store::{QueueStore, QueueStoreMock};
use crate::types::JobOutcome;
use crate::worker::handler::RuntimeHandler;
use crate::worker::leases::{ActiveLease, CompletedLease};

use super::super::support::worker_config;
use super::support::{
    WaitForReleaseComplete, assert_invalid_config, command_timeout, lease_mismatch, reap_ok,
    reserve_job, run_with_timeout, worker, worker_without_clock_calls,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_lease_mismatch_counts_capacity_until_handler_joins() {
    let mut config = worker_config();
    config.heartbeat_interval = Duration::from_millis(1);
    config.poll_interval_min = Duration::from_millis(1);
    config.poll_interval_max = Duration::from_millis(1);
    let heartbeat_seen = Arc::new(AtomicBool::new(false));
    let heartbeat_seen_store = Arc::clone(&heartbeat_seen);
    let handler_release = Arc::new(AtomicBool::new(false));
    let handler_finished = Arc::new(AtomicBool::new(false));
    let handler_finished_store = Arc::clone(&handler_finished);
    let reserve_before_finish = Arc::new(AtomicBool::new(false));
    let reserve_before_finish_store = Arc::clone(&reserve_before_finish);
    let store_unimock = Arc::new(Unimock::new((
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
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 1, _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _, _| {
                if !handler_finished_store.load(Ordering::SeqCst) {
                    reserve_before_finish_store.store(true, Ordering::SeqCst);
                }
                Err(Error::InvalidConfig {
                    kind: InvalidConfigKind::EmptyRedisNodes,
                })
            })),
    )));
    let store: Arc<dyn QueueStore> = store_unimock.clone();
    let worker = worker(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(WaitForReleaseComplete {
        release: Arc::clone(&handler_release),
        finished: Arc::clone(&handler_finished),
    });

    let worker_future = run_with_timeout(worker, handler);
    tokio::pin!(worker_future);
    while !heartbeat_seen.load(Ordering::SeqCst) {
        tokio::select! {
            result = &mut worker_future => {
                panic!("worker exited before heartbeat lease loss: {result:?}");
            }
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
        }
    }
    let settle = tokio::time::sleep(Duration::from_millis(20));
    tokio::pin!(settle);
    tokio::select! {
        result = &mut worker_future => {
            panic!("worker exited before handler release: {result:?}");
        }
        _ = &mut settle => {
        }
    }

    assert!(!reserve_before_finish.load(Ordering::SeqCst));
    handler_release.store(true, Ordering::SeqCst);
    let result = worker_future.await;

    assert_invalid_config(result, InvalidConfigKind::EmptyRedisNodes);
}

#[tokio::test]
async fn recoverable_completion_failure_yields_before_next_pending_completion() {
    let ack_calls = Arc::new(AtomicUsize::new(0));
    let ack_calls_store = Arc::clone(&ack_calls);
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new(
        QueueStoreMock::ack
            .each_call(matching!("workers", "worker-test", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                let previous = ack_calls_store.fetch_add(1, Ordering::SeqCst);
                assert_eq!(previous, 0, "worker should yield after first timeout");
                Err(command_timeout("ack"))
            })),
    ));
    let mut active = HashMap::new();
    let lease_1 = ActiveLease::new("job-1".to_owned(), "lease-1".to_owned());
    active.insert(lease_1.attempt_key(), lease_1);
    let lease_2 = ActiveLease::new("job-2".to_owned(), "lease-2".to_owned());
    active.insert(lease_2.attempt_key(), lease_2);
    let mut pending_completed = HashMap::new();
    let completed_1 = CompletedLease {
        job_id: "job-1".to_owned(),
        lease_token: "lease-1".to_owned(),
        result: Ok(JobOutcome::Complete),
    };
    pending_completed.insert(completed_1.attempt_key(), completed_1);
    let completed_2 = CompletedLease {
        job_id: "job-2".to_owned(),
        lease_token: "lease-2".to_owned(),
        result: Ok(JobOutcome::Complete),
    };
    pending_completed.insert(completed_2.attempt_key(), completed_2);
    let worker = worker_without_clock_calls(store, worker_config());

    let should_back_off = worker
        .resolve_pending_completed(&mut active, &mut pending_completed)
        .await
        .expect("recoverable completion error should not exit worker");

    assert!(should_back_off);
    assert_eq!(ack_calls.load(Ordering::SeqCst), 1);
    assert_eq!(active.len(), 2);
    assert_eq!(pending_completed.len(), 2);
}
