//! Pending reserve fairness tests.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use async_trait::async_trait;
use chrono::TimeZone;
use chrono::Utc;
use unimock::{MockFn as _, Unimock, matching};

use crate::clock::{Clock, ClockMock, TokioWorkerCoordinator, WorkerCoordinator};
use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, InvalidConfigKind, Result};
use crate::store::{EnqueueRequest, FailureRequest, QueueStore, ReserveBatch, ReservedJob};
use crate::types::{
    EnqueueResult, FailedJobsPage, FailedJobsQuery, FailedSelector, ForceFailRequest, JobOutcome,
    JobRecord, QueueCounts, RescheduleResult, WorkerInfo,
};
use crate::worker::handler::{RuntimeHandler, RuntimeHandlerMock};

use super::super::Worker;
use super::support::{unused_store_error, worker_config_with_concurrency};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_reserve_does_not_delay_active_job_ack() {
    let config = worker_config_with_concurrency(2);
    let now = Utc
        .timestamp_millis_opt(1_764_000_000_000)
        .single()
        .expect("valid timestamp");
    let (reserve_started_tx, reserve_started_rx) = mpsc::channel();
    let store = Arc::new(PendingReserveStore {
        reserve_calls: AtomicUsize::new(0),
        reserve_started: Mutex::new(reserve_started_tx),
        ack_called: AtomicBool::new(false),
    });
    let store_trait: Arc<dyn QueueStore> = store.clone();
    let clock: Arc<dyn Clock> = Arc::new(Unimock::new(
        ClockMock::now.each_call(matching!()).returns(now),
    ));
    let coordinator: Arc<dyn WorkerCoordinator> = Arc::new(TokioWorkerCoordinator);
    let reserve_started_rx = Arc::new(Mutex::new(reserve_started_rx));
    let handler: Arc<dyn RuntimeHandler> = Arc::new(Unimock::new(
        RuntimeHandlerMock::handle
            .next_call(matching!("job-1"))
            .answers_arc(Arc::new(move |_, _| {
                reserve_started_rx
                    .lock()
                    .expect("reserve started receiver lock")
                    .recv_timeout(Duration::from_secs(1))
                    .expect("second reserve should start before completion");
                Ok(JobOutcome::Complete)
            })),
    ));
    let worker = Worker::new(
        "workers".to_owned(),
        store_trait,
        clock,
        coordinator,
        config,
    );

    let result = tokio::time::timeout(Duration::from_secs(1), worker.run_internal(handler)).await;

    match result {
        Ok(Err(Error::InvalidConfig { kind })) => {
            assert_eq!(kind, InvalidConfigKind::EmptyRedisNodes);
        }
        other => panic!("expected ack sentinel error before reserve completed, got {other:?}"),
    }
    assert!(store.ack_called.load(Ordering::SeqCst));
}

struct PendingReserveStore {
    reserve_calls: AtomicUsize,
    reserve_started: Mutex<mpsc::Sender<()>>,
    ack_called: AtomicBool,
}

#[async_trait]
impl QueueStore for PendingReserveStore {
    async fn reserve(
        &self,
        _namespace: &str,
        _config: &WorkerConfig,
        _available_capacity: usize,
        _now: Timestamp,
        _lease_tokens: Vec<String>,
    ) -> Result<ReserveBatch> {
        match self.reserve_calls.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(ReserveBatch {
                jobs: vec![ReservedJob {
                    job_id: "job-1".to_owned(),
                    lease_token: "lease-1".to_owned(),
                }],
                next_schedule_at: None,
            }),
            _ => {
                {
                    let sender = self
                        .reserve_started
                        .lock()
                        .expect("reserve started sender lock");
                    let _ = sender.send(());
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                Err(unused_store_error())
            }
        }
    }

    async fn ack(
        &self,
        _namespace: &str,
        _worker_id: &str,
        _job_id: &str,
        _lease_token: &str,
    ) -> Result<()> {
        self.ack_called.store(true, Ordering::SeqCst);
        Err(unused_store_error())
    }

    async fn remember_namespace(&self, _namespace: &str) -> Result<()> {
        Err(unused_store_error())
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        Err(unused_store_error())
    }

    async fn enqueue(&self, _namespace: &str, _request: EnqueueRequest) -> Result<EnqueueResult> {
        Err(unused_store_error())
    }

    async fn reschedule(
        &self,
        _namespace: &str,
        _job_id: &str,
        _schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult> {
        Err(unused_store_error())
    }

    async fn get_job(&self, _namespace: &str, _job_id: &str) -> Result<Option<JobRecord>> {
        Err(unused_store_error())
    }

    async fn counts(&self, _namespace: &str, _now: Timestamp) -> Result<QueueCounts> {
        Err(unused_store_error())
    }

    async fn list_failed(
        &self,
        _namespace: &str,
        _query: FailedJobsQuery,
    ) -> Result<FailedJobsPage> {
        Err(unused_store_error())
    }

    async fn list_workers(&self, _namespace: &str, _now: Timestamp) -> Result<Vec<WorkerInfo>> {
        Err(unused_store_error())
    }

    async fn cancel(&self, _namespace: &str, _job_id: &str) -> Result<()> {
        Err(unused_store_error())
    }

    async fn force_ack(&self, _namespace: &str, _job_id: &str) -> Result<()> {
        Err(unused_store_error())
    }

    async fn force_fail(
        &self,
        _namespace: &str,
        _request: ForceFailRequest,
        _now: Timestamp,
    ) -> Result<()> {
        Err(unused_store_error())
    }

    async fn requeue(&self, _namespace: &str, _job_id: &str, _now: Timestamp) -> Result<()> {
        Err(unused_store_error())
    }

    async fn retry_now(&self, _namespace: &str, _job_id: &str, _now: Timestamp) -> Result<()> {
        Err(unused_store_error())
    }

    async fn force_retry(&self, _namespace: &str, _job_id: &str, _now: Timestamp) -> Result<()> {
        Err(unused_store_error())
    }

    async fn purge_failed(&self, _namespace: &str, _selector: FailedSelector) -> Result<u64> {
        Err(unused_store_error())
    }

    async fn heartbeat(
        &self,
        _namespace: &str,
        _worker_id: &str,
        _job_id: &str,
        _lease_token: &str,
        _now: Timestamp,
        _lease_duration: Duration,
    ) -> Result<()> {
        Ok(())
    }

    async fn complete_and_reschedule(
        &self,
        _namespace: &str,
        _worker_id: &str,
        _job_id: &str,
        _lease_token: &str,
        _schedule_at: Option<Timestamp>,
        _now: Timestamp,
    ) -> Result<()> {
        Err(unused_store_error())
    }

    async fn fail_or_retry(&self, _namespace: &str, _request: FailureRequest) -> Result<u32> {
        Err(unused_store_error())
    }

    async fn reap_expired(&self, _namespace: &str, _now: Timestamp, _limit: usize) -> Result<u64> {
        Ok(0)
    }
}
