//! Fatal runtime error tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use unimock::{MockFn as _, Unimock, matching};

use crate::error::{Error, InvalidConfigKind};
use crate::store::{QueueStore, QueueStoreMock};
use crate::worker::handler::RuntimeHandler;

use super::super::support::worker_config;
use super::support::{
    PanicHandler, WaitForFlagComplete, assert_invalid_config, reap_ok, reserve_job,
    run_with_timeout, worker, worker_without_clock_calls,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_data_stays_fatal() {
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
                Err(Error::InvalidData { field: "heartbeat" })
            })),
    )));
    let worker = worker(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(WaitForFlagComplete {
        flag: heartbeat_seen,
    });

    let result = run_with_timeout(worker, handler).await;

    match result {
        Err(Error::InvalidData { field }) => assert_eq!(field, "heartbeat"),
        other => panic!("expected invalid data error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_task_join_stays_fatal() {
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
        reap_ok(),
    )));
    let worker = worker(store, worker_config());
    let handler: Arc<dyn RuntimeHandler> = Arc::new(PanicHandler {
        flag: heartbeat_seen,
    });

    let result = run_with_timeout(worker, handler).await;

    match result {
        Err(Error::WorkerTaskJoin { .. }) => {}
        other => panic!("expected worker task join error, got {other:?}"),
    }
}

#[tokio::test]
async fn invalid_config_stays_fatal() {
    let mut config = worker_config();
    config.concurrency = 0;
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new(()));
    let worker = worker_without_clock_calls(store, config);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(Unimock::new(()));

    let result = run_with_timeout(worker, handler).await;

    assert_invalid_config(result, InvalidConfigKind::ConcurrencyZero);
}
