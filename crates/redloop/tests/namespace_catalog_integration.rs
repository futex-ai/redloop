mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use redloop::{Backoff, JobHandler, JobOutcome, RetryPolicy, WorkerConfig};

use support::{RedisTestContext, wait_until};

struct CountingCompleteHandler {
    processed: Arc<AtomicUsize>,
}

#[async_trait]
impl JobHandler for CountingCompleteHandler {
    async fn handle(&self, _job_id: String) -> redloop::HandlerResult {
        self.processed.fetch_add(1, Ordering::SeqCst);
        Ok(JobOutcome::Complete)
    }
}

#[tokio::test]
async fn worker_processes_job_when_namespace_catalog_write_fails() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    let queue = ctx
        .redloop
        .namespace(ctx.namespace_name("catalog-write-fail"));
    queue.job("job-catalog").execute().await?;
    corrupt_namespace_catalog_key(&ctx).await?;

    let processed = Arc::new(AtomicUsize::new(0));
    let worker = queue.worker(WorkerConfig {
        worker_id: "catalog-worker".to_owned(),
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
        let processed = Arc::clone(&processed);
        async move {
            worker
                .run(Arc::new(CountingCompleteHandler { processed }))
                .await
        }
    });

    wait_until(Duration::from_secs(5), || {
        let processed = Arc::clone(&processed);
        async move { processed.load(Ordering::SeqCst) >= 1 }
    })
    .await;
    wait_until(Duration::from_secs(5), || {
        let queue = queue.clone();
        async move { queue.get_job("job-catalog").await.ok().flatten().is_none() }
    })
    .await;

    handle.abort();
    let _ = handle.await;
    Ok(())
}

async fn corrupt_namespace_catalog_key(ctx: &RedisTestContext) -> redloop::Result<()> {
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
    let key = if ctx.key_prefix.is_empty() {
        "namespaces".to_owned()
    } else {
        format!("{}:namespaces", ctx.key_prefix)
    };

    let _: u64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .map_err(|source| redloop::Error::Redis {
            operation: "test_delete_namespaces",
            source,
        })?;
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg("wrong-type")
        .query_async(&mut connection)
        .await
        .map_err(|source| redloop::Error::Redis {
            operation: "test_corrupt_namespaces",
            source,
        })?;
    Ok(())
}
