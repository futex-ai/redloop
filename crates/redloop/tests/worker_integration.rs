mod support;

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use redloop::{
    Backoff, DynRedloopNamespace, EnqueueResult, EnqueueStatus, JobError, JobHandler, JobOutcome,
    JobState, RetryPolicy, WorkerConfig,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use support::{RedisTestContext, wait_until};

#[derive(Debug)]
struct RetryableError;

impl std::fmt::Display for RetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("retryable")
    }
}

impl std::error::Error for RetryableError {}

struct RetryThenCompleteHandler {
    attempts: Arc<AtomicUsize>,
}

#[async_trait]
impl JobHandler for RetryThenCompleteHandler {
    async fn handle(&self, _job_id: String) -> redloop::HandlerResult {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        match attempt {
            0 => Err(JobError::Retryable {
                message: RetryableError.to_string(),
            }),
            1 => Ok(JobOutcome::Reschedule {
                schedule_at: Some(Utc::now() + ChronoDuration::milliseconds(50)),
            }),
            _ => Ok(JobOutcome::Complete),
        }
    }
}

struct TerminalFailHandler;

#[async_trait]
impl JobHandler for TerminalFailHandler {
    async fn handle(&self, _job_id: String) -> redloop::HandlerResult {
        Ok(JobOutcome::Fail {
            message: "stop".into(),
        })
    }
}

struct EnqueueDuringLeaseHandler {
    queue: DynRedloopNamespace,
    attempts: Arc<AtomicUsize>,
    enqueue_results: Arc<std::sync::Mutex<Vec<EnqueueResult>>>,
}

#[async_trait]
impl JobHandler for EnqueueDuringLeaseHandler {
    async fn handle(&self, job_id: String) -> redloop::HandlerResult {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            let result =
                self.queue
                    .job(&job_id)
                    .execute()
                    .await
                    .map_err(|error| JobError::Retryable {
                        message: error.to_string(),
                    })?;
            self.enqueue_results
                .lock()
                .expect("result lock should not be poisoned")
                .push(result);
        }
        Ok(JobOutcome::Complete)
    }
}

#[tokio::test]
async fn worker_retries_then_completes_rescheduled_job() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    assert!(ctx.redis_url.starts_with("redis://"));
    assert!(!ctx.key_prefix.is_empty());
    let queue = ctx.redloop.namespace(ctx.namespace_name("worker"));
    queue.job("job-1").execute().await?;

    let attempts = Arc::new(AtomicUsize::new(0));
    let worker = queue.worker(WorkerConfig {
        worker_id: "worker-a".into(),
        concurrency: 1,
        retry_policy: RetryPolicy::Count {
            max_retries: 2,
            backoff: Backoff::None,
        },
        lease_duration: Duration::from_millis(500),
        heartbeat_interval: Duration::from_millis(100),
        reap_interval: Duration::from_millis(100),
        poll_interval_min: Duration::from_millis(10),
        poll_interval_max: Duration::from_millis(50),
    });

    let mut handle = Some(tokio::spawn({
        let attempts = Arc::clone(&attempts);
        async move {
            worker
                .run(Arc::new(RetryThenCompleteHandler { attempts }))
                .await
        }
    }));

    tokio::time::sleep(Duration::from_millis(200)).await;
    if handle.as_ref().expect("worker handle").is_finished() {
        let result = handle
            .take()
            .expect("worker handle")
            .await
            .expect("worker task join");
        panic!("worker exited before processing any jobs: {result:?}");
    }

    wait_until(Duration::from_secs(5), || {
        let attempts = Arc::clone(&attempts);
        async move { attempts.load(Ordering::SeqCst) >= 3 }
    })
    .await;

    wait_until(Duration::from_secs(5), || {
        let queue = queue.clone();
        async move { queue.get_job("job-1").await.ok().flatten().is_none() }
    })
    .await;

    let handle = handle.expect("worker handle");
    handle.abort();
    let _ = handle.await;

    Ok(())
}

#[tokio::test]
async fn immediate_enqueue_while_leased_runs_job_again_after_ack() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    let queue = ctx.redloop.namespace(ctx.namespace_name("leased-rerun"));
    queue.job("job-rerun").execute().await?;

    let attempts = Arc::new(AtomicUsize::new(0));
    let enqueue_results = Arc::new(std::sync::Mutex::new(Vec::new()));
    let worker = queue.worker(WorkerConfig {
        worker_id: "worker-rerun".into(),
        concurrency: 1,
        retry_policy: RetryPolicy::Count {
            max_retries: 1,
            backoff: Backoff::None,
        },
        lease_duration: Duration::from_millis(500),
        heartbeat_interval: Duration::from_millis(100),
        reap_interval: Duration::from_millis(100),
        poll_interval_min: Duration::from_millis(10),
        poll_interval_max: Duration::from_millis(50),
    });

    let handle = tokio::spawn({
        let queue = queue.clone();
        let attempts = Arc::clone(&attempts);
        let enqueue_results = Arc::clone(&enqueue_results);
        async move {
            worker
                .run(Arc::new(EnqueueDuringLeaseHandler {
                    queue,
                    attempts,
                    enqueue_results,
                }))
                .await
        }
    });

    wait_until(Duration::from_secs(5), || {
        let attempts = Arc::clone(&attempts);
        async move { attempts.load(Ordering::SeqCst) >= 2 }
    })
    .await;

    {
        let results = enqueue_results
            .lock()
            .expect("result lock should not be poisoned");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].state, JobState::Leased);
        assert_eq!(results[0].status, EnqueueStatus::Updated);
    }

    wait_until(Duration::from_secs(5), || {
        let queue = queue.clone();
        async move { queue.get_job("job-rerun").await.ok().flatten().is_none() }
    })
    .await;

    handle.abort();
    let _ = handle.await;

    Ok(())
}

#[tokio::test]
async fn failed_job_can_be_reactivated_by_normal_queue_command() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    assert!(ctx.redis_url.starts_with("redis://"));
    assert!(!ctx.key_prefix.is_empty());
    let queue = ctx.redloop.namespace(ctx.namespace_name("failed"));
    queue.job("job-2").execute().await?;

    let worker = queue.worker(WorkerConfig {
        worker_id: "worker-b".into(),
        concurrency: 1,
        retry_policy: RetryPolicy::Never,
        lease_duration: Duration::from_millis(500),
        heartbeat_interval: Duration::from_millis(100),
        reap_interval: Duration::from_millis(100),
        poll_interval_min: Duration::from_millis(10),
        poll_interval_max: Duration::from_millis(50),
    });

    let handle = tokio::spawn(async move { worker.run(Arc::new(TerminalFailHandler)).await });

    wait_until(Duration::from_secs(5), || {
        let queue = queue.clone();
        async move {
            matches!(
                queue
                    .get_job("job-2")
                    .await
                    .ok()
                    .flatten()
                    .map(|job| job.state),
                Some(redloop::JobState::Failed)
            )
        }
    })
    .await;

    handle.abort();
    let _ = handle.await;

    let reactivated = queue.job("job-2").execute().await?;
    assert_eq!(reactivated.status, redloop::EnqueueStatus::Updated);
    assert_eq!(reactivated.state, redloop::JobState::Ready);

    Ok(())
}
