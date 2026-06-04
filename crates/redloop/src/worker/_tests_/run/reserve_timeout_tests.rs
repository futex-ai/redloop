//! Reserve timeout behavior tests.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::TimeZone;
use chrono::Utc;
use unimock::{MockFn as _, Unimock, matching};

use crate::clock::{
    Clock, ClockMock, TokioWorkerCoordinator, WorkerCoordinator, WorkerCoordinatorMock,
};
use crate::error::{Error, InvalidConfigKind};
use crate::store::{QueueStore, QueueStoreMock, ReserveBatch, ReservedJob};
use crate::types::JobOutcome;
use crate::worker::handler::{RuntimeHandler, RuntimeHandlerMock};

use super::super::Worker;
use super::support::{redis_timeout_error, worker_config, worker_config_with_concurrency};

#[tokio::test]
async fn reserve_timeout_backs_off_and_continues_loop() {
    let config = worker_config();
    let now = Utc
        .timestamp_millis_opt(1_764_000_000_000)
        .single()
        .expect("valid timestamp");
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 1, _, _))
            .answers_arc(Arc::new(|_, _, _, _, _, _| {
                Err(Error::Redis {
                    operation: "reserve",
                    source: redis_timeout_error(),
                })
            })),
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 1, _, _))
            .answers_arc(Arc::new(|_, _, _, _, _, _| {
                Err(Error::InvalidConfig {
                    kind: InvalidConfigKind::EmptyRedisNodes,
                })
            })),
    )));
    let clock: Arc<dyn Clock> = Arc::new(Unimock::new(
        ClockMock::now.each_call(matching!()).returns(now),
    ));
    let coordinator: Arc<dyn WorkerCoordinator> = Arc::new(Unimock::new(
        WorkerCoordinatorMock::sleep
            .next_call(matching!(_))
            .returns(()),
    ));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(Unimock::new(()));
    let worker = Worker::new("workers".to_owned(), store, clock, coordinator, config);

    let result = worker.run_internal(handler).await;

    match result {
        Err(Error::InvalidConfig { kind }) => {
            assert_eq!(kind, InvalidConfigKind::EmptyRedisNodes);
        }
        other => panic!("expected second reserve error after retry, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reserve_timeout_does_not_abort_active_job_completion() {
    let config = worker_config_with_concurrency(2);
    let now = Utc
        .timestamp_millis_opt(1_764_000_000_000)
        .single()
        .expect("valid timestamp");
    let (completion_tx, completion_rx) = mpsc::channel();
    let completion_rx = Arc::new(Mutex::new(completion_rx));
    let (release_completion_tx, release_completion_rx) = mpsc::channel();
    let release_completion_rx = Arc::new(Mutex::new(release_completion_rx));
    let reserve_waits_for_completion_rx = Arc::clone(&completion_rx);
    let reserve_releases_completion_tx = release_completion_tx.clone();
    let store: Arc<dyn QueueStore> = Arc::new(Unimock::new((
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 2, _, _))
            .returns(Ok(ReserveBatch {
                jobs: vec![ReservedJob {
                    job_id: "job-1".to_owned(),
                    lease_token: "lease-1".to_owned(),
                }],
                next_schedule_at: None,
            })),
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 1, _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _, _| {
                reserve_releases_completion_tx
                    .send(())
                    .expect("completion release signal");
                reserve_waits_for_completion_rx
                    .lock()
                    .expect("completion receiver lock")
                    .recv_timeout(Duration::from_secs(1))
                    .expect("active job should complete before reserve timeout");
                Err(Error::Redis {
                    operation: "reserve",
                    source: redis_timeout_error(),
                })
            })),
        QueueStoreMock::ack
            .next_call(matching!("workers", "worker-test", "job-1", "lease-1"))
            .returns(Ok(())),
        QueueStoreMock::reap_expired
            .each_call(matching!("workers", _, 2))
            .answers_arc(Arc::new(|_, _, _, _| Ok(0))),
        QueueStoreMock::reserve
            .next_call(matching!("workers", _, 2, _, _))
            .answers_arc(Arc::new(|_, _, _, _, _, _| {
                Err(Error::InvalidConfig {
                    kind: InvalidConfigKind::EmptyRedisNodes,
                })
            })),
    )));
    let clock: Arc<dyn Clock> = Arc::new(Unimock::new(
        ClockMock::now.each_call(matching!()).returns(now),
    ));
    let coordinator: Arc<dyn WorkerCoordinator> = Arc::new(TokioWorkerCoordinator);
    let handler_completion_tx = completion_tx.clone();
    let handler_release_completion_rx = Arc::clone(&release_completion_rx);
    let handler: Arc<dyn RuntimeHandler> = Arc::new(Unimock::new(
        RuntimeHandlerMock::handle
            .next_call(matching!("job-1"))
            .answers_arc(Arc::new(move |_, _| {
                handler_release_completion_rx
                    .lock()
                    .expect("completion release receiver lock")
                    .recv_timeout(Duration::from_secs(1))
                    .expect("second reserve should start before completion");
                handler_completion_tx
                    .send(())
                    .expect("active completion signal");
                Ok(JobOutcome::Complete)
            })),
    ));
    let worker = Worker::new("workers".to_owned(), store, clock, coordinator, config);

    let result = worker.run_internal(handler).await;

    match result {
        Err(Error::InvalidConfig { kind }) => {
            assert_eq!(kind, InvalidConfigKind::EmptyRedisNodes);
        }
        other => panic!("expected sentinel reserve error after active ack, got {other:?}"),
    }
}
