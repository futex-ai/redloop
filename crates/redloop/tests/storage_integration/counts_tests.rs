//! Count and namespace integration tests.

use chrono::{Duration as ChronoDuration, Utc};

use super::support::RedisTestContext;

#[tokio::test]
async fn counts_and_namespaces_reflect_mutations() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    let namespace = ctx.namespace_name("counts");
    let queue = ctx.redloop.namespace(namespace.clone());

    queue.job("ready-job").execute().await?;
    queue
        .job("scheduled-job")
        .schedule_at(Utc::now() + ChronoDuration::minutes(10))
        .execute()
        .await?;

    let counts = queue.counts().await?;
    assert_eq!(counts.ready_count, 1);
    assert_eq!(counts.scheduled_future_count, 1);
    assert_eq!(counts.failed_count, 0);

    let namespaces = ctx.redloop.list_namespaces().await?;
    assert!(namespaces.contains(&namespace));
    Ok(())
}
