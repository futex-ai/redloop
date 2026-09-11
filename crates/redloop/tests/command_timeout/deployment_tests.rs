//! Real topology connections use the same response deadline as standalone Redis.

use std::time::Duration;

use redloop::{ConnectConfig, Error, RedisDeployment, RedisRedloopClient};
use tokio::time::{Instant, sleep, timeout};

use super::deployments::RedisProcess;

#[tokio::test]
async fn sentinel_discovered_connection_honors_response_timeout() {
    let master = RedisProcess::standalone().await;
    let sentinel = RedisProcess::sentinel(master.port).await;
    let client = connect(
        RedisDeployment::Sentinel {
            service_name: "test-master".to_owned(),
            nodes: vec![sentinel.url()],
        },
        Duration::from_secs(5),
    )
    .await;
    let namespace = client.namespace("sentinel-timeout");

    master.pause(Duration::from_millis(800)).await;
    namespace.job("wake").execute().await.unwrap();
    master.pause(Duration::from_millis(800)).await;
    assert_eq!(namespace.counts().await.unwrap().ready_count, 1);
}

#[tokio::test]
async fn cluster_commands_and_pipelines_honor_response_timeout() {
    let cluster = RedisProcess::cluster().await;
    let client = connect(
        RedisDeployment::Cluster {
            nodes: vec![cluster.url()],
        },
        Duration::from_secs(5),
    )
    .await;
    let namespace = client.namespace("cluster-timeout");

    cluster.pause(Duration::from_millis(800)).await;
    namespace.job("wake").execute().await.unwrap();
    cluster.pause(Duration::from_millis(800)).await;
    assert_eq!(namespace.counts().await.unwrap().ready_count, 1);
}

#[tokio::test]
async fn cluster_deadline_includes_waiting_for_the_shared_connection() {
    let cluster = RedisProcess::cluster().await;
    let deadline = Duration::from_millis(400);
    let client = connect(
        RedisDeployment::Cluster {
            nodes: vec![cluster.url()],
        },
        deadline,
    )
    .await;
    let namespace = client.namespace("cluster-lock-timeout");
    namespace.job("wake").execute().await.unwrap();
    cluster.pause(Duration::from_secs(2)).await;

    let first_namespace = namespace.clone();
    let first = tokio::spawn(async move { first_namespace.counts().await });
    sleep(Duration::from_millis(100)).await;
    let started = Instant::now();
    let error = timeout(Duration::from_secs(2), namespace.counts())
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, Error::CommandTimedOut { .. }), "{error:?}");
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "waiting for the cluster lock must not add a second deadline"
    );
    assert!(first.await.unwrap().is_err());
}

async fn connect(deployment: RedisDeployment, command_timeout: Duration) -> RedisRedloopClient {
    RedisRedloopClient::connect(ConnectConfig {
        deployment,
        key_prefix: "redloop-deployment-timeout".to_owned(),
        command_timeout,
    })
    .await
    .unwrap()
}
