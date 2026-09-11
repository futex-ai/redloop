//! Delayed replies exercise the connection manager and both Redis query paths.

use std::time::Duration;

use redloop::{ConnectConfig, Error, JobState, RedisDeployment, RedisRedloopClient};
use tokio::time::{Instant, timeout};

use super::proxy::ResponseProxy;
use super::support::{RedisTestContext, wait_until};

const RESPONSE_DELAY: Duration = Duration::from_millis(800);

#[tokio::test]
async fn commands_and_pipelines_honor_the_configured_response_timeout() {
    let redis = RedisTestContext::new().await.unwrap();
    let proxy = ResponseProxy::new(&redis.redis_url).await;
    let client = connect(&proxy, &redis.key_prefix, Duration::from_secs(5)).await;
    let namespace = client.namespace(redis.namespace_name("delayed"));

    let received = proxy.delay_next(RESPONSE_DELAY);
    let started = Instant::now();
    namespace.job("delayed-wake").execute().await.unwrap();
    received.await.unwrap();
    assert!(started.elapsed() >= RESPONSE_DELAY);

    let received = proxy.delay_next(RESPONSE_DELAY);
    let started = Instant::now();
    assert_eq!(namespace.counts().await.unwrap().ready_count, 1);
    received.await.unwrap();
    assert!(started.elapsed() >= RESPONSE_DELAY);
}

#[tokio::test]
async fn response_timeout_is_preserved_after_automatic_reconnection() {
    let redis = RedisTestContext::new().await.unwrap();
    let proxy = ResponseProxy::new(&redis.redis_url).await;
    let client = connect(&proxy, &redis.key_prefix, Duration::from_secs(5)).await;
    let namespace = client.namespace(redis.namespace_name("reconnected"));
    namespace.job("before-reconnect").execute().await.unwrap();
    proxy.disconnect().await;
    wait_until(Duration::from_secs(5), || async {
        namespace.counts().await.is_ok()
    })
    .await;

    let received = proxy.delay_next(RESPONSE_DELAY);
    namespace.job("after-reconnect").execute().await.unwrap();
    received.await.unwrap();
    assert_eq!(namespace.counts().await.unwrap().ready_count, 2);
}

#[tokio::test]
async fn a_timed_out_mutation_can_have_executed_and_retry_stays_idempotent() {
    let redis = RedisTestContext::new().await.unwrap();
    let proxy = ResponseProxy::new(&redis.redis_url).await;
    let deadline = Duration::from_millis(200);
    let client = connect(&proxy, &redis.key_prefix, deadline).await;
    let name = redis.namespace_name("timed-out-enqueue");
    let namespace = client.namespace(name.clone());
    let observer = redis.redloop.namespace(name);
    let received = proxy.delay_next(RESPONSE_DELAY);

    let started = Instant::now();
    let error = timeout(Duration::from_secs(2), namespace.job("wake").execute())
        .await
        .expect("Redis operation must remain bounded")
        .expect_err("reply must exceed the configured deadline");
    assert_timeout(&error);
    assert!(started.elapsed() >= deadline);
    assert!(started.elapsed() < RESPONSE_DELAY);
    received.await.unwrap();
    assert_eq!(
        observer.get_job("wake").await.unwrap().unwrap().state,
        JobState::Ready
    );

    wait_until(Duration::from_secs(5), || async {
        namespace.job("wake").execute().await.is_ok()
    })
    .await;
    assert_eq!(observer.counts().await.unwrap().ready_count, 1);
}

#[tokio::test]
async fn pipelines_remain_bounded_and_usable_after_timeout() {
    let redis = RedisTestContext::new().await.unwrap();
    let proxy = ResponseProxy::new(&redis.redis_url).await;
    let client = connect(&proxy, &redis.key_prefix, Duration::from_millis(200)).await;
    let namespace = client.namespace(redis.namespace_name("timed-out-pipeline"));
    namespace.job("wake").execute().await.unwrap();
    let received = proxy.delay_next(RESPONSE_DELAY);
    let error = timeout(Duration::from_secs(2), namespace.counts())
        .await
        .expect("pipeline must remain bounded")
        .expect_err("pipeline reply must exceed the configured deadline");
    assert_timeout(&error);
    received.await.unwrap();
    wait_until(Duration::from_secs(5), || async {
        namespace
            .counts()
            .await
            .is_ok_and(|counts| counts.ready_count == 1)
    })
    .await;
}

async fn connect(
    proxy: &ResponseProxy,
    key_prefix: &str,
    command_timeout: Duration,
) -> RedisRedloopClient {
    RedisRedloopClient::connect(ConnectConfig {
        deployment: RedisDeployment::Standalone {
            url: proxy.url.clone(),
        },
        key_prefix: key_prefix.to_owned(),
        command_timeout,
    })
    .await
    .unwrap()
}

fn assert_timeout(error: &Error) {
    assert!(
        matches!(error, Error::CommandTimedOut { .. })
            || matches!(error, Error::Redis { source, .. } if source.is_timeout()),
        "{error:?}"
    );
    assert!(error.is_recoverable_worker_runtime());
}
