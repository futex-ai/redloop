//! Runtime recovery test helpers.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::TimeZone;
use chrono::Utc;
use unimock::{Clause, MockFn as _, Unimock, matching};

use crate::clock::{Clock, ClockMock, TokioWorkerCoordinator};
use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, InvalidConfigKind};
use crate::store::{QueueStore, QueueStoreMock, ReserveBatch, ReservedJob};
use crate::types::JobOutcome;
use crate::worker::handler::RuntimeHandler;

use super::super::super::Worker;

pub(super) fn worker(store: Arc<dyn QueueStore>, config: WorkerConfig) -> Worker {
    Worker::new(
        "workers".to_owned(),
        store,
        clock(),
        Arc::new(TokioWorkerCoordinator),
        config,
    )
}

pub(super) fn worker_without_clock_calls(
    store: Arc<dyn QueueStore>,
    config: WorkerConfig,
) -> Worker {
    Worker::new(
        "workers".to_owned(),
        store,
        Arc::new(Unimock::new(())),
        Arc::new(TokioWorkerCoordinator),
        config,
    )
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(Unimock::new(
        ClockMock::now.each_call(matching!()).returns(fixed_now()),
    ))
}

fn fixed_now() -> Timestamp {
    Utc.timestamp_millis_opt(1_764_000_000_000)
        .single()
        .expect("valid timestamp")
}

pub(super) fn reserve_job(job_id: &'static str, lease_token: &'static str) -> impl Clause {
    QueueStoreMock::reserve
        .next_call(matching!("workers", _, 1, _, _))
        .returns(Ok(ReserveBatch {
            jobs: vec![ReservedJob {
                job_id: job_id.to_owned(),
                lease_token: lease_token.to_owned(),
            }],
            next_schedule_at: None,
        }))
}

pub(super) fn reserve_sentinel_error() -> impl Clause {
    QueueStoreMock::reserve
        .next_call(matching!("workers", _, 1, _, _))
        .returns(Err(Error::InvalidConfig {
            kind: InvalidConfigKind::EmptyRedisNodes,
        }))
}

pub(super) fn reserve_empty_repeatedly() -> impl Clause {
    QueueStoreMock::reserve
        .each_call(matching!("workers", _, 1, _, _))
        .answers_arc(Arc::new(|_, _, _, _, _, _| {
            Ok(ReserveBatch {
                jobs: Vec::new(),
                next_schedule_at: None,
            })
        }))
}

pub(super) fn reap_ok() -> impl Clause {
    QueueStoreMock::reap_expired
        .each_call(matching!("workers", _, 1))
        .answers_arc(Arc::new(|_, _, _, _| Ok(0)))
}

pub(super) fn command_timeout(operation: &'static str) -> Error {
    Error::CommandTimedOut {
        operation,
        timeout_ms: 50,
    }
}

pub(super) async fn run_with_timeout(
    worker: Worker,
    handler: Arc<dyn RuntimeHandler>,
) -> crate::error::Result<()> {
    tokio::time::timeout(Duration::from_secs(1), worker.run_internal(handler))
        .await
        .expect("worker should return before test timeout")
}

pub(super) fn assert_invalid_config(result: crate::error::Result<()>, expected: InvalidConfigKind) {
    match result {
        Err(Error::InvalidConfig { kind }) => assert_eq!(kind, expected),
        other => panic!("expected invalid config error, got {other:?}"),
    }
}

pub(super) struct WaitForFlagComplete {
    pub(super) flag: Arc<AtomicBool>,
}

#[async_trait]
impl RuntimeHandler for WaitForFlagComplete {
    async fn handle(&self, _job_id: String) -> std::result::Result<JobOutcome, String> {
        while !self.flag.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(JobOutcome::Complete)
    }
}

pub(super) struct CompleteImmediately;

#[async_trait]
impl RuntimeHandler for CompleteImmediately {
    async fn handle(&self, _job_id: String) -> std::result::Result<JobOutcome, String> {
        Ok(JobOutcome::Complete)
    }
}

pub(super) struct PanicHandler {
    pub(super) flag: Arc<AtomicBool>,
}

#[async_trait]
impl RuntimeHandler for PanicHandler {
    async fn handle(&self, _job_id: String) -> std::result::Result<JobOutcome, String> {
        while !self.flag.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        panic!("handler task failed")
    }
}
