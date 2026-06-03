//! Enqueue and scheduling integration tests.

use chrono::{Duration as ChronoDuration, Utc};
use redloop::{EnqueueStatus, JobState};

use super::support::{RedisTestContext, persisted_timestamp};

#[tokio::test]
async fn enqueue_and_reschedule_moves_between_ready_and_scheduled() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    let queue = ctx.redloop.namespace(ctx.namespace_name("storage"));

    let created = queue.job("job-1").execute().await?;
    assert_eq!(created.status, EnqueueStatus::Created);
    assert_eq!(created.state, JobState::Ready);

    let scheduled_at = persisted_timestamp(Utc::now() + ChronoDuration::hours(1));
    let scheduled = queue
        .job("job-1")
        .schedule_at(scheduled_at)
        .execute()
        .await?;
    assert_eq!(scheduled.status, EnqueueStatus::Updated);
    assert_eq!(scheduled.state, JobState::Scheduled);

    let record = queue.get_job("job-1").await?.expect("job should exist");
    assert_eq!(record.state, JobState::Scheduled);
    assert_eq!(record.schedule_at, Some(scheduled_at));

    let moved_back = queue.reschedule("job-1", None).await?;
    assert_eq!(moved_back.state, JobState::Ready);
    assert!(moved_back.schedule_at.is_none());

    let ready_record = queue.get_job("job-1").await?.expect("job should exist");
    assert_eq!(ready_record.state, JobState::Ready);
    assert!(ready_record.ready_at.is_some());

    Ok(())
}

#[tokio::test]
async fn due_duplicate_scheduled_entry_is_dropped_when_job_is_already_ready() -> redloop::Result<()>
{
    let ctx = RedisTestContext::new().await?;
    let namespace = ctx.namespace_name("duplicates");
    let queue = ctx.redloop.namespace(namespace.clone());
    assert!(ctx.redis_url.starts_with("redis://"));
    assert!(!ctx.key_prefix.is_empty());

    queue.job("job-dup").execute().await?;

    let slot = namespace
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let scheduled_key = format!("{}{{{slot}}}:scheduled", ctx.key_prefix);
    let client =
        redis::Client::open(ctx.redis_url.clone()).map_err(|source| redloop::Error::Redis {
            operation: "test_client_open",
            source,
        })?;
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|source| redloop::Error::Redis {
            operation: "test_async_connection",
            source,
        })?;
    let _: () = redis::cmd("ZADD")
        .arg(&scheduled_key)
        .arg(Utc::now().timestamp_micros())
        .arg("job-dup")
        .query_async(&mut connection)
        .await
        .map_err(|source| redloop::Error::Redis {
            operation: "test_zadd_duplicate",
            source,
        })?;

    let processed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let worker = queue.worker(redloop::WorkerConfig {
        worker_id: "dup-worker".into(),
        concurrency: 1,
        retry_policy: redloop::RetryPolicy::Count {
            max_retries: 1,
            backoff: redloop::Backoff::None,
        },
        lease_duration: std::time::Duration::from_millis(500),
        heartbeat_interval: std::time::Duration::from_millis(100),
        reap_interval: std::time::Duration::from_millis(100),
        poll_interval_min: std::time::Duration::from_millis(10),
        poll_interval_max: std::time::Duration::from_millis(50),
    });

    let handle = tokio::spawn({
        let processed = std::sync::Arc::clone(&processed);
        async move {
            worker
                .run(std::sync::Arc::new(super::support::CompleteOnceHandler {
                    processed,
                }))
                .await
        }
    });

    super::support::wait_until(std::time::Duration::from_secs(5), || {
        let processed = std::sync::Arc::clone(&processed);
        async move { processed.load(std::sync::atomic::Ordering::SeqCst) >= 1 }
    })
    .await;

    super::support::wait_until(std::time::Duration::from_secs(2), || {
        let queue = queue.clone();
        let scheduled_key = scheduled_key.clone();
        let redis_url = ctx.redis_url.clone();
        async move {
            let job_gone = queue.get_job("job-dup").await.ok().flatten().is_none();
            let client = redis::Client::open(redis_url.clone()).ok();
            let Some(client) = client else {
                return false;
            };
            let mut connection = match client.get_multiplexed_async_connection().await {
                Ok(connection) => connection,
                Err(_) => return false,
            };
            let scheduled_score: Option<String> = match redis::cmd("ZSCORE")
                .arg(&scheduled_key)
                .arg("job-dup")
                .query_async(&mut connection)
                .await
            {
                Ok(score) => score,
                Err(_) => return false,
            };
            job_gone && scheduled_score.is_none()
        }
    })
    .await;

    assert_eq!(processed.load(std::sync::atomic::Ordering::SeqCst), 1);

    handle.abort();
    let _ = handle.await;
    Ok(())
}
