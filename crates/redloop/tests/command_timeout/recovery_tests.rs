//! A reservation reply can be lost after Redis has already granted the lease.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use redloop::{
    ConnectConfig, HandlerResult, JobHandler, JobOutcome, JobState, RedisDeployment,
    RedisRedloopClient, RetryPolicy, WorkerConfig,
};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

use super::proxy::ResponseProxy;
use super::support::{RedisTestContext, wait_until};

struct RecordingHandler(mpsc::UnboundedSender<String>);

#[async_trait]
impl JobHandler for RecordingHandler {
    async fn handle(&self, job_id: String) -> HandlerResult {
        self.0.send(job_id).unwrap();
        Ok(JobOutcome::Complete)
    }
}

#[tokio::test]
async fn lost_reservation_reply_is_reaped_and_processed_without_worker_restart() {
    let redis = RedisTestContext::new().await.unwrap();
    let proxy = ResponseProxy::new(&redis.redis_url).await;
    let client = RedisRedloopClient::connect(ConnectConfig {
        deployment: RedisDeployment::Standalone {
            url: proxy.url.clone(),
        },
        key_prefix: redis.key_prefix.clone(),
        command_timeout: Duration::from_millis(200),
    })
    .await
    .unwrap();
    let name = redis.namespace_name("lost-reservation");
    let namespace = client.namespace(name.clone());
    let observer = redis.redloop.namespace(name);
    namespace.job("reserved-wake").execute().await.unwrap();

    let received = proxy.delay_reservation("reserved-wake", Duration::from_millis(800));
    let (processed, mut deliveries) = mpsc::unbounded_channel();
    let worker = namespace.worker(WorkerConfig {
        worker_id: "recovering-worker".to_owned(),
        concurrency: 1,
        retry_policy: RetryPolicy::Never,
        lease_duration: Duration::from_secs(2),
        heartbeat_interval: Duration::from_millis(500),
        reap_interval: Duration::from_millis(250),
        poll_interval_min: Duration::from_millis(10),
        poll_interval_max: Duration::from_millis(50),
    });
    let task = tokio::spawn(async move { worker.run(Arc::new(RecordingHandler(processed))).await });

    timeout(Duration::from_secs(5), received)
        .await
        .unwrap()
        .unwrap();
    sleep(Duration::from_millis(300)).await;
    assert_eq!(
        observer
            .get_job("reserved-wake")
            .await
            .unwrap()
            .unwrap()
            .state,
        JobState::Leased
    );
    assert!(
        deliveries.try_recv().is_err(),
        "handler must not run for an unreceived lease"
    );
    assert!(
        !task.is_finished(),
        "a reserve timeout must not stop the worker"
    );

    let delivery = timeout(Duration::from_secs(5), deliveries.recv()).await;
    if delivery.is_err() && task.is_finished() {
        panic!("worker exited before recovery: {:?}", task.await);
    }
    assert!(
        delivery.is_ok(),
        "recovery timed out: {:?}",
        observer.get_job("reserved-wake").await
    );
    assert_eq!(delivery.unwrap().as_deref(), Some("reserved-wake"));
    wait_until(Duration::from_secs(5), || async {
        observer.get_job("reserved-wake").await.unwrap().is_none()
    })
    .await;
    assert!(
        deliveries.try_recv().is_err(),
        "only the recovered lease should run"
    );
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}
